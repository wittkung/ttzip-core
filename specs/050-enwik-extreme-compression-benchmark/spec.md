# Feature Specification: enwik8 / enwik9 Extreme Compression Ratio & Memory Ceiling Benchmark

**Feature Branch**: `050-enwik-extreme-compression-benchmark`

**Created**: 2026-08-17

**Status**: Draft

**Input**: User description: "18. enwik8 / enwik9 (极端高压缩比文本语料)。语料内容：100MB / 1GB 维基百科纯 XML 语料，具备海量长距离重复模式。双平台价值：用于高压力压测双平台下 LZMA2、BZIP2、ZSTD 高级别（Level 19~22）的字典命中率与多核内存占用上限。"

## Clarifications

### Session 2026-08-17

- Q: How should enwik8 (100 MB) vs. enwik9 (1 GB) be staged across local development, standard CI, and stress-test pipelines to balance repository size, runner memory limits, and test execution time? → A: enwik8 (100 MB uncompressed) is integrated as a standard fixture for benchmark test passes (`TTZIP_RUN_BENCHMARKS=1`); enwik9 (1 GB uncompressed) is staged as an on-demand / nightly high-stress target (`TTZIP_RUN_STRESS_BENCHMARKS=1`) with streaming verification to prevent low-memory runner exhaustion.
- Q: What are the primary algorithms and compression levels evaluated under this extreme-ratio corpus? → A: High-dictionary match finders including LZMA2 (Levels 5~9, dict size 16MB~64MB), ZSTD (Levels 19~22, Ultra window log 22~27), and BZIP2 (Level 9, block size 900k).
- Q: What specific memory metrics must be captured during high-pressure multi-core compression runs? → A: Resident Set Size (RSS) peak memory footprint, memory allocation per thread/worker, streaming buffer residency (guaranteeing adherence to Stream-First micro-buffering), and allocation stability across Apple Silicon UMA vs. standard x86 memory layouts.
- Q: How are GB-scale payloads (enwik9) stored and delivered without bloating the Git repository? → A: Stored out-of-tree in localized user cache (`~/Library/Caches/com.ttzip.tests/fixtures/` or `~/.cache/ttzip/fixtures/`). Tests automatically fetch compressed seeds (`enwik9.zst` ~150MB) on demand via multi-mirror fallback (TTZip GitHub Release CDN primary, Matt Mahoney official origin fallback), decompress in-process with TTZip engine, and lock with inter-process file locks.
- Q: How is offline execution handled when network is unavailable? → A: Automatic seamless fallback to an in-memory `SyntheticXmlCorpusGenerator` that deterministically produces 1GB XML text with high-repetition patterns at > 2000 MB/s, guaranteeing zero CI blocking.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Extreme Long-Distance Pattern Compression & Ratio Validation (Priority: P1)

As a compression engine architect and performance researcher, I want the benchmark suite to test enwik8 and enwik9 XML corpora against high-level compression pipelines (LZMA2, ZSTD Ultra, BZIP2), so that I can evaluate dictionary match-finder depth, long-distance repetition exploitation, and compression ratio efficiency under highly redundant structured text.

**Why this priority**: Highly repetitive structured text (XML dumps with duplicated tags, templates, and revision histories) represents the upper bound of dictionary compression algorithms. Evaluating compression ratio on enwik8/enwik9 verifies whether multi-threaded chunking strategies maintain global match efficiency without degrading compression density compared to single-threaded golden baselines.

**Independent Test**: Can be verified by executing compression runs on enwik8 across target algorithms (LZMA2, ZSTD, BZIP2) and validating that compressed byte size meets or beats target ratio thresholds while maintaining exact byte-for-byte decompression parity.

**Acceptance Scenarios**:

1. **Given** the 100 MB enwik8 uncompressed XML payload, **When** compressed with LZMA2 (Level 9) or ZSTD (Level 19+), **Then** the compressed output achieves the expected extreme ratio (< 30% of original uncompressed size for LZMA2, < 35% for ZSTD), and decompressing the output yields an exact cryptographic hash match with the original payload.
2. **Given** multi-threaded parallel compression enabled, **When** processing enwik8/enwik9 chunks across multiple CPU cores, **Then** the global compression ratio does not degrade by more than 1.5% compared to single-threaded sequential execution.

---

### User Story 2 - High-Pressure Peak Memory Footprint & Ceiling Gating (Priority: P2)

As a core systems engineer, I want automated memory tracking during extreme-level compression of 100 MB and 1 GB workloads, so that peak Resident Set Size (RSS) memory consumption is strictly bounded per thread and per engine instance, preventing Out-Of-Memory (OOM) aborts on resource-constrained runners and mobile/desktop clients.

**Why this priority**: Maximum-level compression algorithms (e.g., ZSTD Level 22 with 1 GB window size or LZMA2 Level 9 with large dictionary match finders) can allocate hundreds of megabytes or gigabytes of memory per thread if unbounded. Establishing deterministic memory ceiling gates ensures engine safety and prevents memory leaks or catastrophic paging.

**Independent Test**: Can be tested by instrumenting memory measurement hooks during enwik8/enwik9 compression passes and asserting that peak memory consumption remains within predefined hardware memory budgets (e.g., $\le 512$ MB for 100 MB enwik8, $\le 2.0$ GB for 1 GB enwik9 streaming).

**Acceptance Scenarios**:

1. **Given** a multi-core compression pass on enwik8 with maximum compression levels, **When** memory usage is monitored across all worker threads, **Then** peak resident memory does not exceed the designated memory budget threshold.
2. **Given** the 1 GB enwik9 stress workload, **When** processed through streaming compression pipelines, **Then** memory usage remains stable in micro-buffering ranges without unbounded heap expansion.

---

### User Story 3 - Cross-Platform Multi-Core Scaling & Decompression Burst Throughput (Priority: P3)

As a release engineer supporting macOS (Apple Silicon UMA) and cross-platform environments, I want to benchmark compression throughput, multi-core scaling efficiency, and decompression burst speed under extreme-ratio archives, so that platform-specific I/O latency, memory bandwidth bottlenecks, and CPU cache saturations can be identified and optimized.

**Why this priority**: Decompressing highly compressed XML text is computationally intensive on bitstream decoding and dictionary lookahead, stressing the L1/L2/L3 cache hierarchies and unified memory bus differently across architectures.

**Independent Test**: Can be tested by measuring wall-clock duration and calculating throughput (MB/s) for both compression and decompression phases across single-core vs. multi-core configurations on target hardware.

**Acceptance Scenarios**:

1. **Given** enwik8 compressed archives, **When** decompressed across supported algorithms, **Then** decompression throughput meets or exceeds established speed floors (e.g., $\ge 1500$ MB/s for ZSTD, $\ge 350$ MB/s for LZMA2) with zero corrupted bytes.
2. **Given** multi-core scaling enabled on multi-core systems, **When** core counts scale from 1 to $N$, **Then** compression throughput exhibits linear or near-linear scaling up to memory bus saturation limits.

---

### Edge Cases

- **Out-Of-Memory (OOM) on Restricted CI Nodes**: If running on a CI environment with strict RAM limits (e.g. 2 GB runner), high-thread LZMA2/ZSTD Ultra jobs must throttle worker concurrency or fall back to bounded dictionary windows to prevent process termination.
- **Corpus Integrity & Checksum Failure**: If the enwik8 or enwik9 fixture file is corrupted or truncated on disk, the harness must fail fast prior to running benchmarks, logging an explicit hash mismatch diagnostic.
- **Extreme Compression Timeouts**: High-level compression on 1 GB payloads can take significant CPU time. The harness must support configurable per-pass timeouts and allow enwik8 for fast CI gating while isolating enwik9 to extended stress runs.
- **Zero-Byte Expansion on Non-Compressible Chunks**: While XML is highly compressible, specific dense segments or pre-compressed inclusions within XML must not cause buffer overflow or decoding failures.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The test framework MUST provide a unified fixture loader for the enwik8 standard corpus (100,000,000 bytes uncompressed XML) and optional enwik9 corpus (1,000,000,000 bytes uncompressed XML) with SHA-256 integrity verification.
- **FR-002**: The benchmark suite MUST support execution across high-compression algorithms: LZMA2 (Levels 5~9), ZSTD (Levels 1~22, including Ultra mode), and BZIP2 (Levels 1~9).
- **FR-003**: The benchmark framework MUST capture and report quantitative metrics per pass: Uncompressed Size, Compressed Size, Space Saving Ratio (%), Compression Throughput (MB/s), Decompression Throughput (MB/s), and Peak Resident Set Size (RSS in MB).
- **FR-004**: The benchmark suite MUST enforce a deterministic memory budget gate, asserting that peak RSS memory consumption during enwik8 execution does not exceed the algorithm's configured upper bound.
- **FR-005**: The benchmark framework MUST verify 100% byte-for-byte decompression fidelity (MD5/SHA-256 identical to original corpus input) for all generated archives.
- **FR-006**: The test runner MUST provide decoupled execution controls (`TTZIP_RUN_BENCHMARKS=1` for enwik8 standard pass, `TTZIP_RUN_STRESS_BENCHMARKS=1` for enwik9 extended pass) to prevent lengthening standard unit test execution.
- **FR-007**: The streaming compression pipeline MUST maintain bounded micro-buffering residency, avoiding full-file unconstrained memory loading in accordance with Stream-First architectural invariants.
- **FR-008**: The benchmark harness MUST output structured JSON performance summaries alongside human-readable comparison tables for CI regression tracking.
- **FR-009**: The fixture system MUST implement an out-of-tree localized cache manager (`EnwikFixtureCacheManager`) supporting multi-mirror resilient download (GitHub CDN primary, Matt Mahoney origin fallback), in-process decompression, and POSIX file locking (`flock`).
- **FR-010**: The test harness MUST provide a zero-network deterministic generator (`SyntheticXmlCorpusGenerator`) capable of generating 1GB repetitive structured XML at $\ge 2000$ MB/s as an offline fallback.

### Key Entities

- **EnwikCorpusItem**: Encapsulates an enwik corpus specification (e.g., `enwik8`, `enwik9`), including file path, raw uncompressed byte length ($10^8$ / $10^9$), expected SHA-256 hash, and payload classification (`structured-xml-text`).
- **MemoryCeilingSnapshot**: Captures memory telemetry during benchmark execution, including initial RSS, peak RSS, total allocated virtual memory, and per-worker memory distribution.
- **ExtremeRatioBenchmarkResult**: Stores the complete benchmark evaluation record for a given corpus, algorithm, compression level, and thread count, containing duration, throughput, compressed size, compression ratio, peak RSS, and verification status.
- **ExtremeRatioBenchmarkSuite**: Orchestrates test matrix execution across configured algorithms and levels, performing memory gating assertions, ratio evaluations, and baseline diff generation.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of tested compression archives generated from enwik8/enwik9 decompress to byte-exact parity with the original corpus (0 bit discrepancies).
- **SC-002**: Compression ratio on enwik8 meets target benchmarks for structured text: LZMA2 Level 9 achieves $\ge 70\%$ space reduction (compressed size $\le 30$ MB), ZSTD Level 19+ achieves $\ge 65\%$ space reduction (compressed size $\le 35$ MB).
- **SC-003**: Peak memory consumption during enwik8 compression under default concurrency does not exceed $512$ MB RSS, and streaming enwik9 processing stays strictly under $2.0$ GB RSS.
- **SC-004**: Multi-threaded compression on enwik8 scales throughput by at least $3.0\times$ on 4+ core systems compared to single-threaded baseline without degrading compression ratio by more than $1.5\%$.
- **SC-005**: Zero execution overhead on standard `swift test` runs when benchmark environment flags are disabled.

## Assumptions

- **A-001**: enwik8 payload is stored or fetched deterministically as the canonical first 100,000,000 bytes of the March 3, 2006 English Wikipedia dump.
- **A-002**: Hardware running benchmark tests possesses at least 4 GB of available physical RAM for enwik8 tests and at least 8 GB for enwik9 stress tests.
- **A-003**: Memory tracking utilizes platform-native APIs (e.g. `mach_task_basic_info` / `task_info` on macOS, `/proc/self/statm` on Linux) to obtain reliable RSS measurements.
