# Feature Specification: Systemic Architecture, Resource Invariants & Testing Governance Hardening

**Feature ID**: `224-systemic-architecture-testing-governance`  
**Classification**: `[Full SDD]`  
**Created**: 2026-08-24  
**Status**: Draft  
**Input**: Comprehensive research, systemic root-cause audit, and end-to-end architectural governance hardening for TTZip across Swift, C-ABI, Rust, and local CI.

---

## 1. Executive Summary & Problem Definition

### 1.1 Background & Root-Cause Synthesis
In recent architectural evaluations, multiple systemic flaws were discovered across TTZip's subsystem boundaries:
1. **Engine Routing Bypass**: High-performance Rust Rayon parallel ZIP engines were implemented but bypassed in Swift, which defaulted to legacy single-threaded libarchive wrappers.
2. **Implicit Resource Bloat**: Single-entry extraction, in-place mutation, and split-volume readers relied on whole-file heap loading (`fs::read`) and temporary disk copying (`/tmp`), risking OOM and SSD wear on multi-gigabyte archives.
3. **Context Severance & Dropped Metadata**: Character set detection (`chardetng`) and detailed diagnostic errors were computed in Rust but dropped at the C-ABI boundary (`let _ = ...`), forcing Swift to reimplement fragile heuristics.
4. **Allocation Storms & Dead FFI Accumulation**: VFS fuzzy search allocated hundreds of thousands of heap objects per keystroke (`target.chars().collect::<Vec<char>>()`), while dozens of orphaned C-ABI symbols (`worker_pool`, `mpmc_ring_buffer`) remained in headers without Swift callers.
5. **Testing Invariant Blind Spots**: Existing test suites only validated functional output (`result == expected`), without asserting resource invariants ($O(1)$ memory bounds, zero heap allocations, zero temporary disk write I/O).

### 1.2 Target Objectives
This specification defines the complete architectural governance and verification framework:
- **Pillar I**: Resource Invariant Testing Harness (50GB+ APFS Sparse Fixtures with Mach RSS $<16\text{MB}$ bounding, Scoped `GlobalAlloc` zero-allocation tracking, and `FSEvents` + `proc_pid_rusage` zero-disk-IO leakage monitoring).
- **Pillar II**: Bidirectional C-ABI & Struct Context Linter (`scripts/lint_cabi_context.py` using Clang AST JSON to detect orphaned exports, undefined imports, and dropped struct fields).
- **Pillar III**: End-to-End Tracer & Telemetry Harness (`TTZipExecutionProvenance` and `EngineDispatchProvenance` ensuring concrete Rust engine execution without silent fallback).
- **Pillar IV**: End-to-End Pipeline Benchmarking (`ttzip-bench pipeline` measuring full-pipeline APFS I/O throughput and quantifying FFI Tax percentages).

---

## 2. User Scenarios & Testing *(Prioritized)*

### User Story 1 - Resource-Invariant Hard Assertions (Priority: P1)
As a systems engineer or CI gate runner, I need the test suite to fail immediately if any streaming operation loads whole archives into memory, leaks temporary files to `/tmp`, or performs heap allocations during interactive search.

**Why this priority**: Directly eliminates OOM vulnerabilities, disk write amplification, and UI stuttering by enforcing resource bounds as hard CI failures.

**Independent Test**:
- Run `cargo test --test sparse_fixture_rss_test` on a 50GB virtual APFS sparse file -> Peak RSS must strictly stay $<16\text{MB}$.
- Run `cargo test --test zero_alloc_vfs_search_test` on 100,000 nodes -> Heap allocations in critical section must be strictly $0$.
- Run `swift test --filter ZeroDiskIOLeakHarnessTests` -> Captured `/tmp` disk events and $\Delta \text{DiskIO}$ bytes must be strictly $0$.

**Acceptance Scenarios**:
1. **Given** a 50GB sparse ZIP64 archive, **When** `ZipArchive::open_slice` inspects entries or `read_at` streams bytes, **Then** Mach `task_info` peak RSS remains under 16MB.
2. **Given** a 100,000-node VFS tree, **When** `search_vfs_tree_zero_alloc` runs fuzzy matching against preallocated buffer slots, **Then** thread-local `TrackingAllocator` records exactly 0 allocations.
3. **Given** 100 streaming in-memory single-entry extractions, **When** `FSEventTempWatcher` and `proc_pid_rusage` monitor system paths, **Then** 0 bytes are written to disk outside the target directory.

---

### User Story 2 - Bidirectional C-ABI & Struct Context Linter (Priority: P1)
As an architect, I need a deterministic static analysis tool that guarantees 100% mutual consumption between C-ABI headers and Swift callers, preventing dead exports and dropped metadata fields.

**Why this priority**: Prevents architectural drift, dead code accumulation, and dropped diagnostic/charset context at the FFI boundary.

**Independent Test**:
- Run `python3 scripts/lint_cabi_context.py --strict` -> Passes cleanly when all C-ABI functions and struct fields are consumed in Swift (or explicitly exempted).
- Inject an unused C-ABI function or comment out Swift access to `TTZipEntryMetadata.detected_encoding` -> Linter fails with `CABI_001` or `CABI_003` error.

**Acceptance Scenarios**:
1. **Given** `ttzip_rust_glue.h` exporting C-ABI functions, **When** `lint_cabi_context.py` runs, **Then** any symbol with 0 Swift callers outside `cabi_exemptions.json` raises `CABI_001_DEAD_CABI_EXPORT`.
2. **Given** C structs exported to Swift, **When** fields like `detected_encoding`, `crc32`, or `mtime_epoch_secs` are never accessed, **Then** linter raises `CABI_003_STRUCT_FIELD_DROPPED`.
3. **Given** local Git hooks and CI, **When** `./scripts/verify_cabi_symbols.sh` executes, **Then** it verifies Mach-O symbol parity, dead code parity, and struct field context integrity.

---

### User Story 3 - End-to-End Engine Dispatch Provenance & Anti-Fallback Assertions (Priority: P2)
As a developer, I need Swift facade operations (`ArchiveWriter`, `ArchiveReader`, `InPlaceArchiveMutationEngine`) to provide non-forgeable provenance telemetry confirming that operations executed via pure Rust engines and never silently fell back to legacy wrappers.

**Why this priority**: Eliminates engine bypass regressions where developers assume high-performance Rust runs while code silently executes single-threaded legacy C fallbacks.

**Independent Test**:
- Run `swift test --filter E2EEnginePathTracerTests` -> Verifies `report.provenance.engineTag == .rustRayonParallelZip` and `report.provenance.isFallback == false`.

**Acceptance Scenarios**:
1. **Given** an `ArchiveWriter.createArchiveWithReport` call for ZIP, **When** the operation finishes, **Then** `provenance.engineTag` is `.rustRayonParallelZip` and `isFallback` is `false`.
2. **Given** an in-place mutation call for 7z, **When** the operation commits, **Then** `provenance.engineTag` is `.rustInPlaceSevenZip`.
3. **Given** an unexpected internal error causing libarchive fallback, **When** `provenance.isFallback` is `true`, **Then** `fallbackReason` contains diagnostic details and tests fail via `assertNoFallback`.

---

### User Story 4 - Full-Pipeline APFS Benchmarking & FFI Tax Quantification (Priority: P3)
As a performance engineer, I need `ttzip-bench` to measure end-to-end pipeline throughput (`Swift -> C-ABI -> Rust -> APFS I/O`) alongside isolated in-memory codec speed, computing the exact FFI bridge tax and I/O overhead.

**Why this priority**: Bridges the gap between synthetic algorithmic benchmarks and real-world user perceived desktop performance.

**Independent Test**:
- Run `swift run ttzip-bench pipeline --json-out docs/benchmarks/latest_pipeline_telemetry.json` -> Outputs isolated speed, E2E speed, FFI tax %, and degradation %.

**Acceptance Scenarios**:
1. **Given** Silesia / Calgary test datasets, **When** `ttzip-bench pipeline` runs, **Then** it measures isolated in-memory codec MB/s, full E2E APFS MB/s, and calculates `FFI Tax %`.
2. **Given** `--json-out` parameter, **When** executed, **Then** structured JSON telemetry is emitted for regression monitoring.

---

## 3. Clarifications & Resolved Decisions

### Decision 1: Linter Enforcement Scope & Hook Integration
- **Resolution**: `scripts/lint_cabi_context.py` will be integrated as Stage 2 in `./scripts/verify_cabi_symbols.sh` and `./scripts/run_local_ci_gate.sh`. It enforces bidirectional parity (`CABI_001` through `CABI_006`) on all C-ABI functions and C structs, with explicit exemptions declared in `scripts/cabi_exemptions.json`.

### Decision 2: Provenance Telemetry Propagation in Swift
- **Resolution**: High-level Swift APIs (`ArchiveWriter`, `ArchiveReader`, `InPlaceArchiveMutationEngine`) will expose `createArchiveWithReport` and `extractWithReport` returning `(result, EngineDispatchProvenance)` using `EngineProvenanceCollector.capture { ... }`. The base void/bool methods will continue to work for backwards compatibility.

### Decision 3: Sparse File Fixture Size & Speed Bounds
- **Resolution**: The APFS sparse test fixture will use logical size 50GB, generated via POSIX hole seeks in $< 5\text{ms}$ with $< 16\text{KB}$ physical disk allocation. Peak RSS during inspection and block streaming is bounded to $< 16\text{MB}$.

### Decision 4: Zero-Allocation Scope
- **Resolution**: Zero-allocation assertion (`assert_zero_alloc`) applies to the inner search loop of `search_vfs_tree_zero_alloc` where results are written into pre-allocated memory slices (`&mut [Option<VfsMatchRef>]`), guaranteeing 0 heap allocations during typing/search events.

---

## 4. Requirements & Invariants

### 3.1 Functional Requirements
- **FR-1**: APFS sparse file generator must create 50GB+ virtual archives in $< 10\text{ms}$ using $< 32\text{KB}$ physical disk blocks.
- **FR-2**: Mach Task Basic Info RSS monitor must sample at $\ge 1\text{kHz}$ ($500\mu\text{s}$ intervals) and assert peak RSS $< 16\text{MB}$ during 50GB archive inspection.
- **FR-3**: Thread-local `TrackingAllocator` must track `alloc`, `realloc`, and `dealloc` in designated test scopes without impacting parallel test worker threads.
- **FR-4**: VFS search algorithm must provide a zero-allocation mode (`search_vfs_tree_zero_alloc`) populating preallocated result slices.
- **FR-5**: `FSEventTempWatcher` must capture any filesystem mutations in `/tmp`, `/private/tmp`, `$TMPDIR`, and assert zero leakage during streaming operations.
- **FR-6**: `lint_cabi_context.py` must use `/usr/bin/clang` AST JSON to extract function and record declarations without third-party Python dependencies.
- **FR-7**: `ttzip-engine` must export `TTZipExecutionProvenance` thread-local context with exact `TTZipEngineTag` and kernel duration nanoseconds.
- **FR-8**: `TTZipCore` must expose `EngineDispatchProvenance` and `EngineProvenanceCollector` to wrap Swift operations.

### 3.2 Non-Functional Invariants (NFR)
- **NFR-1 (Single-File LOC Limit)**: All new and modified Swift, Rust, and Python files must remain strictly $\le 800$ LOC.
- **NFR-2 (Strict Concurrency & Memory Safety)**: Swift code must comply with Swift 6 Strict Concurrency (`Sendable` structs, `OSAllocatedUnfairLock`). Rust code must have zero unsafe concurrency blocks (`Send`/`!Sync` soundness).
- **NFR-3 (Zero Cloud CI Reliance)**: All test fixtures, linters, and verification scripts must run completely offline on macOS Apple Silicon.
- **NFR-4 (Local Hook Speed)**: Local `pre-push` gate must complete within $< 30\text{s}$ total runtime.

---

## 4. Architecture & Interface Specifications

### 4.1 Rust Invariant & Provenance Interfaces
```rust
// Rust Engine Tag & Provenance Models (types.rs)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TTZipEngineTag {
    Unknown = 0,
    RustRayonParallelZip = 1,
    RustStreamingParallelZip = 2,
    RustZeroCopy7zDecoder = 3,
    RustPure7zEncoder = 4,
    RustTarStreamEngine = 5,
    RustInPlaceZip = 6,
    RustInPlaceSevenZip = 7,
    RustVfsParallelScanner = 8,
    LibarchiveLegacy = 100,
    Cli7zFallback = 101,
    SystemTarFallback = 102,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TTZipExecutionProvenance {
    pub engine_tag: TTZipEngineTag,
    pub thread_count: u32,
    pub uncompressed_bytes: u64,
    pub compressed_bytes: u64,
    pub kernel_duration_nanos: u64,
    pub is_fallback: bool,
    pub fallback_reason: [libc::c_char; 128],
}
```

### 4.2 C-ABI Declarations (`ttzip_rust_glue.h`)
```c
// Execution Provenance & Telemetry
bool ttzip_rust_get_last_execution_provenance(TTZipExecutionProvenance *out_provenance);
const char *ttzip_rust_engine_tag_name(TTZipEngineTag tag);

// Zero-Allocation VFS Search
int32_t ttzip_rust_vfs_search_zero_alloc(
    const void *session_handle,
    const char *query,
    TTZipVfsMatchDto *out_matches,
    int32_t capacity
);
```

### 4.3 Swift Provenance Models (`EngineDispatchProvenance.swift`)
```swift
public enum EngineExecutionTag: String, Sendable, Equatable, CaseIterable {
    case rustRayonParallelZip = "RustRayonParallelZip"
    case rustStreamingParallelZip = "RustStreamingParallelZip"
    case rustZeroCopy7zDecoder = "RustZeroCopy7zDecoder"
    case rustPure7zEncoder = "RustPure7zEncoder"
    case rustTarStreamEngine = "RustTarStreamEngine"
    case rustInPlaceZip = "RustInPlaceZip"
    case rustInPlaceSevenZip = "RustInPlaceSevenZip"
    case rustVfsParallelScanner = "RustVfsParallelScanner"
    case libarchiveLegacy = "LibarchiveLegacy"
    case cli7zFallback = "Cli7zFallback"
    case systemTarFallback = "SystemTarFallback"
    case unknown = "Unknown"

    public var isPureRust: Bool { ... }
}

public struct EngineDispatchProvenance: Sendable, Equatable {
    public let engineTag: EngineExecutionTag
    public let threadCount: Int
    public let uncompressedBytes: Int64
    public let compressedBytes: Int64
    public let kernelDurationNanos: UInt64
    public let isFallback: Bool
    public let fallbackReason: String?
    public let ffiBridgeOverheadNanos: UInt64
    public let totalE2EDurationNanos: UInt64
    public var throughputMBs: Double { ... }
}
```

---

## 5. Verification & Acceptance Criteria

```
===================================================================================================
                           TTZip 质量与测试治理验收标准
===================================================================================================
验收门禁                   | 校验手段                               | 判定通过阈值
---------------------------------------------------------------------------------------------------
1. 50GB 稀疏大包内存边界   | cargo test --test sparse_fixture_rss   | 峰值物理内存 RSS < 16.0 MB
2. 100k 节点 VFS 树搜索    | cargo test --test zero_alloc_vfs_search| 临界区堆分配数 == 0 次
3. 分卷与流式临时磁盘 I/O  | swift test --filter ZeroDiskIOLeak     | 临时目录写事件 == 0, ΔDiskIO == 0
4. 双向 C-ABI 死代码/字段  | python3 scripts/lint_cabi_context.py   | 0 Dead Exports, 0 Dropped Fields
5. 端到端引擎穿透防回退    | swift test --filter E2EEnginePathTracer| 100% Pure Rust, isFallback == false
6. 全链路管道基准与 FFI 税 | swift run ttzip-bench pipeline         | 输出 FFI Tax % 与 E2E 真实吞吐
===================================================================================================
```
