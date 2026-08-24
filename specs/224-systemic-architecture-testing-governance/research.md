# Research Notes: Systemic Architecture & Testing Governance

**Feature ID**: `224-systemic-architecture-testing-governance`  
**Classification**: `[Full SDD]`  
**Date**: 2026-08-24  

---

## 1. Research Item 1: APFS Sparse File & Peak RSS Invariant

### 1.1 Findings
- **POSIX Seek Hole**: On Apple File System (APFS), writing a Local File Header at byte 0, calling `lseek(fd, 50 * 1024 * 1024 * 1024, SEEK_CUR)`, and appending the Central Directory + Zip64 EOCD creates a 50GB logical file in `< 5ms`.
- **Physical Block Consumption**: `stat.st_blocks` is $\le 32$ (approx 16KB), meaning 0 disk allocation pressure.
- **Mach Task Info vs getrusage**: Darwin's `task_info(mach_task_self(), MACH_TASK_BASIC_INFO, ...)` provides exact real-time `resident_size` (current RSS) and `resident_size_max`. On macOS, `getrusage.ru_maxrss` is in bytes (unlike Linux where it is in kilobytes).
- **Sampling Guard**: A background thread sampling RSS at $500\mu\text{s}$ intervals captures any transient heap spike and asserts `peak_rss < 16MB` during 50GB archive inspection.

### 1.2 Verification Source
- Darwin `mach/task_info.h` & `sys/resource.h`
- Subagent `Resource-Invariant-Harness-Architect` live execution & validation.

---

## 2. Research Item 2: Zero-Heap Allocation VFS Search

### 2.1 Findings
- **Allocation Vectors**: Standard fuzzy matching uses `target.chars().collect::<Vec<char>>()`, generating $>200,000$ heap allocations for 100k nodes.
- **Zero-Alloc Solution**:
  1. `fuzzy_match_zero_alloc` uses `target.char_indices()` and advances pattern characters inline without heap collections.
  2. Result sinks are pre-allocated slices `&mut [Option<VfsMatchRef<'a>>]` allocated on the stack or reused across UI typing keystrokes.
- **Allocation Auditor**: A custom `GlobalAlloc` (`TrackingAllocator`) equipped with `thread_local!` state asserts `alloc_count == 0` during the search critical section.

### 2.2 Verification Source
- Rust `core::alloc::GlobalAlloc` & `std::cell::Cell`
- Subagent `Resource-Invariant-Harness-Architect` benchmark fixture.

---

## 3. Research Item 3: Zero-Disk-IO Leakage Tracking

### 3.1 Findings
- **FSEvents Integration**: CoreServices `FSEventStreamCreate` with `kFSEventStreamCreateFlagFileEvents | kFSEventStreamCreateFlagNoDefer` monitors `/tmp`, `/private/tmp`, and `$TMPDIR`.
- **Kernel IO Rusage**: `proc_pid_rusage(getpid(), PROC_PIDRUSAGE_VI, &info)` returns `ri_diskio_byteswritten`.
- **Assertion**: For 100 in-memory streaming extractions, `leaked_tmp_events == 0` and $\Delta \text{DiskBytes} == 0$.

### 3.2 Verification Source
- macOS CoreServices `FSEvents.h` & `libproc.h`
- Subagent `Resource-Invariant-Harness-Architect`.

---

## 4. Research Item 4: Bidirectional Clang AST C-ABI Linter

### 4.1 Findings
- **Zero Python Dependency AST**: `clang -fsyntax-only -Xclang -ast-dump=json Sources/CTTZipBridge/include/ttzip_rust_glue.h` extracts 100% of C-ABI functions and C structs.
- **Static Dead Code Scanner**: Matches all Swift calls against exported C-ABI functions, flagging orphaned functions (`CABI_001`) and dropped struct fields (`CABI_003`).
- **Exemptions File**: `scripts/cabi_exemptions.json` permits declaring explicit tool/fuzzing exemptions with rationale.

### 4.2 Verification Source
- Clang 16+ AST JSON specification
- Subagent `Bidirectional-CABI-Linter-Architect`.

---

## 5. Research Item 5: End-to-End Dispatch Provenance & FFI Tax

### 5.1 Findings
- **Immutable Provenance**: Rust engine writes `TTZipExecutionProvenance` into a thread-local cell at the exact moment of completion.
- **Swift Provenance Collector**: `EngineProvenanceCollector.capture` retrieves the provenance and computes `ffiBridgeOverheadNanos = totalNanos - kernelNanos`.
- **FFI Tax % Formula**:
  $$\text{FFI Tax \%} = \frac{T_{\text{E2E}} - T_{\text{Isolated}} - T_{\text{APFS\_IO}}}{T_{\text{E2E}}} \times 100\%$$
- **Anti-Fallback Assertions**: `TTZipAssertions.assertNoFallback` verifies that `isFallback == false` and `engineTag.isPureRust == true`.

### 5.2 Verification Source
- Swift 6 Swift Package Manager & `TTZipCore`
- Subagent `End-to-End-Tracer-Architect`.
