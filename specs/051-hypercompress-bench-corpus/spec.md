# Feature Specification: HyperCompressBench (Micro-Files & Data Center Fragments) Benchmark Suite

**Feature Branch**: `051-hypercompress-bench-corpus`  
**Created**: 2026-08-17  
**Status**: Draft  
**Input**: User description: "19. HyperCompressBench (Google 微文件与数据中心碎片语料)。语料内容：数万个 1KB~64KB 微型 JSON、日志片段与高熵伪随机文件。Mac / Windows 平台价值：压测 macOS (APFS) 与 Windows (NTFS) 在遍历、读取海量小文件时的目录扫描性能与多线程 I/O 调度能力；检验 TTZip 的小文件批处理 Fast-Path 性能门禁（>= 500 文件）。"

## Clarifications

### Session 2026-08-17

- Q: How should tens of thousands of micro-files be generated and stored without inflating the Git repository with millions of inodes?
  → A: Implement a deterministic `HyperCompressCorpusGenerator` that programmatically produces reproducible file topologies (depth 3~5, fanout 10~50, pseudo-random payload mix) in temporary directory / RAM disk, paired with an optional cached `.tar.zst` seed package for cross-language validation.
- Q: What is the exact payload distribution across the micro-file categories?
  → A: 40% Micro-JSON (1KB~8KB, structured repeated keys), 40% Log Fragments (8KB~32KB, timestamps/stacktraces), 20% High-Entropy Random Blobs (16KB~64KB, uncompressible binary).
- Q: What are the tiered workload scales for different test environments?
  → A: Tier 1 (CI Gate): 500~2,000 files (~20MB total) for instant regression gating (< 2s). Tier 2 (Stress Bench): 20,000~50,000 files (~500MB total) for deep I/O and APFS/NTFS directory scanner benchmarking (`TTZIP_RUN_STRESS_BENCHMARKS=1`).
- Q: How to prevent OS File Descriptor (FD) exhaustion on macOS (default ulimit -n 256/1024)?
  → A: The batch scanner and compressor must use streaming batch workers with strict per-thread FD quotas (<= 64 concurrent open handles), closing descriptors immediately upon mmap/buffer load and reusing directory search handles.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Micro-File Batch Compression Fast-Path & Floor Gating (Priority: P1)

As a core engine developer, I want the automated test harness to compress 500 to 2,000 micro-files (1KB~64KB) in batch mode across all archive formats, so that TTZip's small-file Fast-Path performance floor (>= 70 MB/s in Release, >= 50 MB/s in Debug) is strictly enforced and regressions in per-file allocation overhead are detected immediately.

**Why this priority**: Micro-files reveal per-file fixed overheads (heap allocations, lock acquisitions, file header initializations). A regression here cripples real-world performance on software projects (e.g., node_modules, Xcode derived data).

**Independent Test**: Execute `swift test --filter HyperCompressBatchGateTests` and assert that batch compression throughput exceeds 70 MB/s without per-file heap thrashing.

**Acceptance Scenarios**:
1. **Given** 500+ micro-files across JSON and log categories, **When** compressed into ZIP / TAR.ZST / 7Z, **Then** total throughput meets or exceeds 70 MB/s in Release (50 MB/s in Debug).
2. **Given** an engine change that introduces dynamic memory allocation per file in the inner loop, **When** the HyperCompress benchmark runs, **Then** the regression gate triggers a failure alert.

---

### User Story 2 - Cross-Platform High-Concurrency Directory Traversal & VFS Immunity (Priority: P2)

As a systems performance engineer targeting macOS (APFS) and Windows (NTFS), I want to benchmark directory scanning and metadata collection throughput on deeply nested file trees (up to 50,000 nodes), so that filesystem lock contention, syscall overheads, and thread pool starvation can be isolated and optimized.

**Why this priority**: Traversal of tens of thousands of files triggers APFS B-tree lock contention and NTFS MFT seeks. The directory scanner must achieve sub-millisecond per-thousand-node throughput without thread deadlock.

**Independent Test**: Run directory scanner benchmarks over a 50,000-node synthetic tree and assert traversal completes in <= 250 ms (>= 250,000 nodes/s).

**Acceptance Scenarios**:
1. **Given** a 50,000-node directory tree, **When** scanned via `ZipDirectoryScanner` / native VFS bridge, **Then** elapsed time is <= 250 ms on Apple Silicon and <= 350 ms on Windows NTFS.
2. **Given** highly concurrent reader threads, **When** scanning adjacent subdirectories, **Then** thread CPU utilization scales linearly without kernel lock starvation.

---

### User Story 3 - Mixed-Entropy Handling & Match-Finder Early-Exit Validation (Priority: P3)

As an algorithm optimization engineer, I want the benchmark suite to evaluate compression behavior on mixed-entropy micro-files (high-repetition JSON vs. uncompressible binary blobs), so that match-finder early-exit mechanisms skip uncompressible blocks without payload expansion or wasted CPU cycles.

**Why this priority**: Forcing high-level dictionary searches on high-entropy data wastes power and throughput. Early-exit detection ensures uncompressible fragments are stored or compressed with minimal effort.

**Independent Test**: Measure CPU time and compressed size on the 20% high-entropy corpus slice and verify execution time is <= 50% of full match-search time.

**Acceptance Scenarios**:
1. **Given** 16KB~64KB high-entropy random files, **When** processed by the compression pipeline, **Then** the compressed output does not expand beyond 100.5% of raw size, and early-exit triggers within the first 4KB evaluation.

---

### Edge Cases

- **File Descriptor Exhaustion**: Handling `EMFILE`/`ENFILE` when directory traversal opens more files than OS soft limit. Must bound active open handles to <= 64 per worker.
- **Deep Directory Nesting / Path Length Limits**: Handling directory depth >= 32 and path lengths exceeding `MAXPATHLEN` (1024 on macOS) or `MAX_PATH` (260 on Windows without `\\?\` prefix).
- **Unicode Normalization (NFC vs. NFD)**: macOS APFS uses normalized UTF-8 (NFD-like decomposed preview), while NTFS preserves verbatim UTF-16 (NFC). The corpus must test multi-byte UTF-8 filenames without collation mismatch.
- **Zero-Byte & Sub-Kilobyte Edge Fragments**: Including 0-byte and 1-byte files alongside 64KB files to test edge boundary handling in batch writers.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The test framework MUST provide a deterministic, zero-network generator (`HyperCompressCorpusGenerator`) capable of synthesizing 500 to 50,000 structured micro-files with fixed PRNG seeds at >= 1500 MB/s.
- **FR-002**: The corpus generator MUST support 3 distinct payload profiles: Micro-JSON (1KB~8KB), Server Logs (8KB~32KB), and High-Entropy Binary (16KB~64KB).
- **FR-003**: The test harness MUST enforce the TTZip small-file batch compression gate (>= 50 MB/s Debug, >= 70 MB/s Release on 500+ files).
- **FR-004**: The directory scanner benchmark MUST measure and assert tree construction speed against standard limits (1,000 nodes <= 10 ms, 10,000 nodes <= 60 ms, 50,000 nodes <= 250 ms).
- **FR-005**: All batch compression operations MUST verify 100% byte-for-byte extraction fidelity via cryptographic checksums across all generated files.
- **FR-006**: The batch compressor MUST operate under strict bounded thread allocations, recycling compression context buffers via thread-local pools without per-file `malloc`/`free`.
- **FR-007**: The benchmark framework MUST capture per-phase breakdown metrics: Directory Scan Time, File Read Time, Compression Time, Archive Write Time, and Peak Memory/FD Count.
- **FR-008**: The harness MUST support isolated temporary directory sandboxing and automatic teardown to guarantee zero disk residue after test execution.
- **FR-009**: The test suite MUST provide decoupled execution flags (`TTZIP_RUN_BENCHMARKS=1` for 2,000 files, `TTZIP_RUN_STRESS_BENCHMARKS=1` for 50,000 files).
- **FR-010**: Path normalization and file metadata extraction MUST be verified across both macOS APFS and Windows NTFS semantics.

### Key Entities

- **MicroCorpusProfile**: Defines the distribution parameters (file count, size range, entropy model, directory depth, fanout).
- **HyperCompressBatchResult**: Records batch execution telemetry including node count, total bytes, scan duration, compression throughput, extraction throughput, peak memory, and verification status.
- **DirectoryScanMetric**: Captures directory traversal statistics (total entries, elapsed wall time, nodes per second, syscall count).
- **HyperCompressBenchmarkSuite**: Coordinates tiered execution across archive formats, asserting performance floor compliance.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Batch compression of 500 micro-files achieves >= 70 MB/s throughput in Release mode (>= 50 MB/s in Debug) across ZIP, TAR.ZST, and 7Z formats.
- **SC-002**: In-memory / APFS directory traversal of 50,000 generated nodes finishes in <= 250 ms (>= 200,000 items/s).
- **SC-003**: 100% of extracted files match original CRC32/SHA-256 hashes with zero corruption across all categories.
- **SC-004**: Peak file descriptor usage during 50,000-file traversal never exceeds 128 open handles concurrently.
- **SC-005**: Zero execution overhead on default `swift test` runs when benchmark flags are omitted.

## Assumptions

- **A-001**: Micro-files can be generated on-the-fly into RAM disk (`/tmp` / `NSTemporaryDirectory`) to isolate raw engine performance from physical disk thermal throttling.
- **A-002**: Test machines provide at least 2 GB of free RAM and standard POSIX file permissions.
