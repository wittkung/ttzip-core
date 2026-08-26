# Feature Specification: Silesia Corpus Standard Benchmark Fixtures & Regression Gates

**Feature Branch**: `049-silesia-corpus-benchmark`

**Created**: 2026-08-17

**Status**: Draft

**Input**: User description: "Silesia Corpus (标准真实数据语料集)。语料内容：包含 12 种典型真实场景文件（共 211 MB）：可执行程序 (mozilla)、医学影像 (mr)、纯文本 (dickens)、数据库 (samba)、PDF 文档 (reymont)、预编译库 (x-ray) 等。Mac 平台价值：作为 Apple Silicon 统一内存上测试全格式 16 种压缩比与吞吐波动的黄金基准。Windows 平台价值：在 MSVC 与 Windows 文件系统（NTFS）上作为消除缓存抖动的标准化测试集。落地方式：【P0 引入】 完整纳入 Tests/TTZipTests/Fixtures/Silesia/，作为 CI/CD 性能回归门禁输入。"

## Clarifications

### Session 2026-08-17

- Q: How should the Silesia corpus files be stored and accessed in TTZipTests fixtures to avoid Git LFS/network latency during CI while maintaining clean repository structure? → A: Directly stored under `Tests/TTZipTests/Fixtures/Silesia/` as bundled static resources within SPM `Package.swift` resource target (`.copy("Fixtures")`), with an integrity manifest (`silesia_manifest.json`) recording exact file sizes and SHA-256 hashes.
- Q: How should the performance regression threshold be enforced during benchmark test runs? → A: Calibrated against historical golden baseline throughput (MB/s); any drop > 3.0% on any of the 12 files triggers an XCTest failure with detailed diagnostic diff table.
- Q: How should warm-up and cache jitter be handled for reliable throughput measurement? → A: Minimum 1 warm-up iteration followed by 3 measurement iterations, computing median throughput and variance ($\le \pm 2.5\%$).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Standardized Real-World Performance Regression Gating (Priority: P1)

As a core engine developer and release maintainer, I want CI/CD and local test suites to automatically run compression and decompression benchmarks against the 12 standardized Silesia corpus files across all supported archive formats, so that any code modifications causing throughput drop or compression ratio degradation on real-world payload distributions are caught immediately before merging.

**Why this priority**: Synthetic or random test data fails to reflect real CPU cache hierarchy behavior, SIMD vectorization efficiency, and entropy distribution in practical scenarios. Establishing a 211 MB gold-standard corpus test gate prevents real-world performance regressions from reaching production.

**Independent Test**: Can be verified by running the automated benchmark suite against the local Silesia fixture directory and confirming that per-file and total-corpus compression/decompression throughput, compression ratio, and checksum integrity are reported and checked against historical performance baselines.

**Acceptance Scenarios**:

1. **Given** the 12 Silesia standard corpus files are present in the test fixture directory, **When** the performance regression gate test is executed, **Then** all 12 files are processed through all primary compression/decompression pipelines without failure, and each run produces valid data integrity assertions (byte-level parity).
2. **Given** an engine change that introduces an unintended performance slowdown (> 3.0% regression), **When** the Silesia benchmark suite runs in the CI environment, **Then** the regression gate triggers a failure status with clear diagnostic reporting highlighting the regressed file type and format.

---

### User Story 2 - Cross-Platform Baseline & Cache-Jitter Immunity (Priority: P2)

As a cross-platform engineer targeting macOS (Apple Silicon UMA) and Windows (MSVC/NTFS), I want standardized benchmark executions to evaluate performance metrics under consistent physical payload conditions, so that memory bus saturation, filesystem cache trashing, and OS-specific I/O latency anomalies can be objectively separated from compression algorithm efficiency.

**Why this priority**: Apple Silicon unified memory architectures and Windows NTFS caching behave drastically differently under high-throughput streaming. Using the identical 211 MB physical payload ensures true cross-platform parity and reproducible benchmarks.

**Independent Test**: Can be tested on both macOS and Windows build agents by executing identical corpus benchmarking suites and comparing normalized throughput and compression ratios across systems.

**Acceptance Scenarios**:

1. **Given** identical Silesia corpus files on macOS and Windows test environments, **When** benchmark runs are executed, **Then** both environments output standardized JSON/console metric records with identical file hashes, byte counts, and comparable ratio breakdowns.
2. **Given** repeated benchmark runs on the same environment, **When** memory caching effects occur, **Then** the suite provides warm-up and multi-iteration aggregation to filter out filesystem cache jitter.

---

### User Story 3 - Granular Corpus Data-Type Profiling & Anomaly Diagnostics (Priority: P3)

As an optimization researcher and algorithm developer, I want to view detailed per-file performance breakdowns across all 12 Silesia file types (executable, medical image, literature text, database table, PDF document, raw binary), so that algorithm tuning and SIMD optimizations can target specific entropy domains (e.g., LZMA2 match-finder depth on text vs. zstd compression on binary executables).

**Why this priority**: Real-world payloads have vastly different entropy distributions. A global average metric hides domain-specific bottlenecks (e.g., a regression in PDF text parsing offset by gains in executable compression). Granular inspection ensures targeted optimization.

**Independent Test**: Can be tested by invoking the benchmark runner with a granular reporting flag, verifying that individual metrics for all 12 corpus components are logged with throughput, compression ratio, and duration.

**Acceptance Scenarios**:

1. **Given** a benchmark execution over the Silesia corpus, **When** the detailed diagnostic mode is requested, **Then** the test reporter generates individual rows for each of the 12 files (`dickens`, `mozilla`, `mr`, `nci`, `ooffice`, `osdb`, `reymont`, `samba`, `sao`, `webster`, `xml`, `x-ray`).

---

### Edge Cases

- **Missing Fixture Files**: What happens when the Silesia fixture directory is incomplete or partially corrupted on a test node? The system must detect missing or invalid corpus files via cryptographic hashes before benchmarking starts and fail fast with an explicit descriptive error rather than producing skewed benchmark results.
- **Extreme Memory Pressure / Low-Resource Runners**: How does the benchmark suite handle CI nodes with limited RAM (< 2 GB)? The benchmark loader must support streaming or sequential file-by-file testing to prevent Out-Of-Memory (OOM) aborts while preserving throughput fidelity.
- **Read-Only Test Environments**: What happens when the fixture files reside on a read-only filesystem (e.g., SPM resource bundles or container mount points)? The suite must operate purely in-memory or in isolated temporary output sandboxes without attempting in-place writes to fixture paths.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The test framework MUST provide a centralized, immutable fixture loader for the complete Silesia standard dataset (12 files, totaling ~211 MB uncompressed, loaded via `TestFixtureLoader.silesiaCorpusURL`).
- **FR-002**: The fixture loader MUST verify the integrity (SHA-256 and byte length) of each corpus file against `silesia_manifest.json` prior to benchmarking to guarantee reproducible gold-standard baselines.
- **FR-003**: The regression testing suite MUST execute compression and decompression rounds across all supported archive formats (ZIP, 7Z, TAR, TAR.ZST, TAR.GZ, TAR.BZ2, TAR.XZ, etc.) using the Silesia corpus as input.
- **FR-004**: The benchmark harness MUST compute and report quantitative metrics per file and aggregate: Uncompressed Size, Compressed Size, Compression Ratio (%), Compression Throughput (MB/s), Decompression Throughput (MB/s), and CPU/Wall Time.
- **FR-005**: The benchmark framework MUST support baseline comparison against historical golden baselines, emitting automated failure alerts if throughput falls below established floor thresholds (> 3.0% regression).
- **FR-006**: The fixture repository MUST be integrated cleanly into project build manifests without breaking developer checkout ergonomics or existing fast unit test execution.
- **FR-007**: The test suite MUST provide decoupled execution tags, allowing developers to run lightweight unit tests without requiring the full 211 MB benchmark pass, while ensuring CI performance jobs execute the full matrix (`TTZIP_RUN_BENCHMARKS=1`).
- **FR-008**: The corpus runner MUST support isolated temporary directory sandboxing for compression/decompression targets, guaranteeing zero side-effects across test iterations.

### Key Entities

- **SilesiaCorpusItem**: Represents a single standard corpus component, containing file name, data category (e.g., text, executable, database, image), exact uncompressed byte length, and expected checksum.
- **CorpusBenchmarkResult**: Encapsulates the performance metrics for a specific file and format pairing, including compression/decompression duration, throughput in MB/s, compressed payload size, and byte-exact verification status.
- **CorpusBenchmarkSuite**: Orchestrates execution across all 12 corpus items for one or more archive formats, aggregating summary statistics, variance calculations, and regression threshold assertions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the 12 Silesia standard corpus files (totaling 211,945,550 bytes) are accessible and validated with byte-level integrity in local and CI test environments.
- **SC-002**: The Silesia benchmark suite completes a full multi-format compression and decompression matrix validation pass within standard CI timeout limits (< 120 seconds in Release mode).
- **SC-003**: Benchmark result variability across 3 consecutive warm runs on the same hardware is contained within $\pm 2.5\%$, providing stable regression detection.
- **SC-004**: Any simulated performance regression greater than 3.0% on any of the 12 files reliably triggers a test failure in CI gating.
- **SC-005**: Zero impact on standard unit test suite execution time when benchmark mode is not explicitly enabled.

## Assumptions

- The 12 standard Silesia corpus files (`dickens`, `mozilla`, `mr`, `nci`, `ooffice`, `osdb`, `reymont`, `samba`, `sao`, `webster`, `xml`, `x-ray`) are public domain / open benchmark data and can be bundled within the test fixtures.
- Test runners meet minimum hardware requirements (>= 4 GB RAM, 64-bit architecture) capable of buffering or streaming up to 50 MB single-file payloads without paging swaps.
- Developer local workflows can choose between fast unit tests and full Silesia benchmark sweeps via environment flags or dedicated filter arguments (e.g., `TTZIP_RUN_BENCHMARKS=1`).
