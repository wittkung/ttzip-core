# Feature Specification: TurboBench & lzbench In-Memory Benchmarking & High-Precision Timer Calibration Suite

**Feature Branch**: `052-turbobench-inmemory-alignment`  
**Created**: 2026-08-17  
**Status**: Draft  
**Input**: User description: "20. powturbo/TurboBench & inikep/lzbench 项目地址：github.com/powturbo/TurboBench 开源许可证：用于开发调试与测试对比 双平台支持度：支持 macOS (Clang) 与 Windows (MSVC) 原生编译运行。对我们的核心赋能：【评测基准对齐】纯内存运行（In-Memory Benchmarking），彻底排除磁盘 I/O、杀毒软件（Windows Defender）或文件系统缓存带来的抖动误差；校准 TTZip 内部 ttzip-cli bench 的计时器精度（mach_absolute_time on macOS / QueryPerformanceCounter on Windows）与 MB/s 计算公式，确保对外发布的性能数据具有工业级公信力。"

## Clarifications

### Session 2026-08-17

- Q: What exact in-memory benchmarking methodology should be aligned with TurboBench / lzbench standards?
  → A: Adopt pre-allocated contiguous memory buffers (source buffer, destination compressed buffer, roundtrip verification buffer), execute configurable warmup iterations (default 2 passes) to prime CPU caches / TLB / branch predictors, and perform multi-iteration timed loops until accumulated wall-clock time reaches a statistically stable threshold (>= 500 ms per test slice).
- Q: How should high-resolution platform timers and time-base conversion be standardized across macOS and Windows?
  → A: On macOS / Apple Silicon, encapsulate hardware nanosecond precision via `mach_absolute_time()` scaled by `mach_timebase_info`; on Windows, encapsulate `QueryPerformanceCounter` scaled by `QueryPerformanceFrequency`; on POSIX/Linux, encapsulate `clock_gettime(CLOCK_MONOTONIC_RAW)`. Eliminate high-level UI framework dependencies (e.g. `CACurrentMediaTime`).
- Q: What unit definition and rounding methodology should be enforced for throughput calculation?
  → A: Align with the industry standard decimal MB/s metric ($1\text{ MB/s} = 1,000,000\text{ Bytes/second}$) with configurable toggle for binary MiB/s ($1\text{ MiB/s} = 1,048,576\text{ Bytes/second}$), calculating throughput as $\text{Raw Uncompressed Bytes} / \text{Elapsed Time in Seconds}$ across both compression and decompression phases.
- Q: How to handle CPU frequency scaling and thermal throttling jitter during sustained benchmark runs?
  → A: Support statistical aggregation modes (fastest run / min elapsed time as used in TurboBench to measure peak CPU potential, alongside median and interquartile mean), with optional CPU cache eviction passes between trials when evaluating cold-start performance.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Pure In-Memory Benchmark Engine with Zero Disk I/O Jitter (Priority: P1)

As a compression engine developer, I want to execute pure in-memory benchmarks on raw buffers across all supported algorithms (Deflate, LZMA2, ZSTD, LZ4, Brotli, Snappy, etc.), so that disk I/O, VFS inode locking, filesystem cache writebacks, and OS security hooks (macOS Gatekeeper / Windows Defender) are completely bypassed and engine algorithmic throughput is measured with absolute repeatability.

**Why this priority**: Disk I/O introduces 30% to 50% measurement variance and caps high-throughput algorithms (e.g., LZ4/ZSTD at >10 GB/s). Pure in-memory execution is the baseline requirement for valid algorithm benchmarking.

**Independent Test**: Execute `swift run ttzip-cli bench --in-memory --format zip,7z,zstd,lz4` and verify all allocations, compression, decompression, and integrity verifications occur exclusively within pre-allocated RAM without file descriptor or disk syscall activity.

**Acceptance Scenarios**:
1. **Given** an in-memory input buffer populated with standard benchmark corpora (e.g., Silesia, Enwik, Synthetic XML), **When** running in-memory benchmark mode, **Then** zero disk read/write syscalls are emitted during the measurement loop, and compression/decompression throughput is computed solely over CPU processing time.
2. **Given** 10 consecutive runs of the in-memory benchmark under identical system load, **When** calculating standard deviation of measured throughput, **Then** coefficient of variation ($CV = \sigma / \mu$) is <= 2.5%.

---

### User Story 2 - Cross-Platform Hardware High-Resolution Timer Calibration (Priority: P2)

As a cross-platform performance architect, I want `ttzip-cli bench` and core performance suites to utilize calibrated native hardware timers (`mach_absolute_time` on macOS/Apple Silicon, `QueryPerformanceCounter` on Windows, and `clock_gettime(CLOCK_MONOTONIC_RAW)` on POSIX), so that sub-microsecond timing precision is guaranteed and published benchmark data has industrial-grade credibility matching TurboBench and lzbench.

**Why this priority**: High-level timers (such as `Date()`, `gettimeofday()`, or UI frame timers) suffer from timer quantization, wall-clock drift, NTP adjustments, and millisecond-level jitter that invalidate micro-benchmarks.

**Independent Test**: Run the timer calibration diagnostic suite and assert monotonic progress, sub-100ns tick resolution, and zero backward drift under heavy multithreaded context switching.

**Acceptance Scenarios**:
1. **Given** Apple Silicon macOS hardware, **When** querying elapsed benchmark time, **Then** measurements utilize `mach_absolute_time()` with cached `mach_timebase_info` conversions without UI framework dependencies.
2. **Given** Windows x64 / ARM64 environment, **When** running benchmarks, **Then** timing derives directly from `QueryPerformanceCounter` with `QueryPerformanceFrequency` conversion.
3. **Given** very fast decompression passes (< 50 microseconds), **When** measured over multiple accumulated iterations, **Then** timer overhead contributes < 0.1% to total recorded duration.

---

### User Story 3 - TurboBench & lzbench Metric Formula Standardization & Output Parity (Priority: P3)

As a benchmarking analyst and open-source contributor, I want `ttzip-cli bench` to output standardized benchmark metrics (Throughput in MB/s, Compression Ratio %, Inverse Ratio / CSize, and Speedup Multiplier) formatted identically to TurboBench / lzbench tables, so that direct apples-to-apples performance comparisons with upstream compression libraries can be generated seamlessly.

**Why this priority**: Divergent metric formulas ($10^6$ vs $1024^2$, compressed size basis vs raw size basis) create confusion and undermine external trust. Conforming to TurboBench / lzbench standard representations ensures seamless validation by external maintainers.

**Independent Test**: Compare output metrics of `ttzip-cli bench --in-memory --compat-turbobench` against native `turbobench` execution on the same corpus, verifying exact formula alignment.

**Acceptance Scenarios**:
1. **Given** raw and compressed byte counts and elapsed time, **When** calculating throughput, **Then** decimal $10^6$ MB/s is computed as $\text{Uncompressed Bytes} / (\text{Time in Seconds} \times 1,000,000)$, and compression ratio is computed as $(\text{Compressed Bytes} / \text{Uncompressed Bytes}) \times 100.0\%$.
2. **Given** benchmark output in JSON or Markdown table format, **When** exported, **Then** fields include algorithm name, compression level, compressed size, compression MB/s, decompression MB/s, and roundtrip MD5/CRC32 verification status.

---

### Edge Cases

- **Timer Integer Overflow & Timebase Multiplier Arithmetic**: Ensuring 64-bit unsigned hardware ticks scaled by numerator/denominator never overflow `UInt64.max` during extended stress runs (using `__builtin_mul_overflow` or 128-bit intermediate arithmetic).
- **Sub-Millisecond Execution / Zero-Elapsed Protection**: Clamping minimum measured duration to non-zero positive epsilon (>= 1 nanosecond) to prevent division by zero in ultra-fast passes.
- **Warmup Cache Pollution vs. Working Set Size**: Handling buffers exceeding CPU L3 cache size (> 32MB / 64MB) where cache warming behaves differently from resident micro-blocks.
- **Roundtrip Buffer Overrun**: Ensuring pre-allocated destination buffers account for worst-case uncompressible data expansion (e.g., raw size + 64KB + 1% overhead per format specification).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide an isolated in-memory benchmark engine (`InMemoryBenchmarkEngine`) that executes compression and decompression directly on pre-allocated contiguous memory buffers without touching physical disk storage.
- **FR-002**: The benchmark engine MUST support configurable warmup iterations (minimum 1, default 2) prior to timing collection to ensure CPU instruction cache, data cache (L1/L2/L3), and branch target predictors are fully primed.
- **FR-003**: The benchmark engine MUST execute multi-pass measurement loops until accumulated elapsed time exceeds a target duration threshold (default: >= 500 ms per algorithm/level test point) to eliminate timer quantization error on ultra-fast algorithms.
- **FR-004**: The system MUST implement a platform-agnostic high-resolution monotonic timer abstraction (`PlatformMonotonicTimer`) backed by `mach_absolute_time()` on macOS, `QueryPerformanceCounter` on Windows, and `clock_gettime(CLOCK_MONOTONIC_RAW)` on Linux/POSIX.
- **FR-005**: All timer timebase conversions MUST cache hardware frequency parameters at system initialization to ensure zero syscall overhead during the active timing measurement window.
- **FR-006**: The system MUST support standard TurboBench throughput calculations defined as decimal $\text{MB/s} = \text{Raw Bytes} / (\text{Seconds} \times 10^6)$, with an optional `--binary-units` flag for binary $\text{MiB/s} = \text{Raw Bytes} / (\text{Seconds} \times 1048576)$.
- **FR-007**: The system MUST calculate and report compression ratio as $\text{Ratio} = (\text{Compressed Bytes} / \text{Uncompressed Bytes}) \times 100.0\%$ and space savings percentage as $(1.0 - \text{Compressed Bytes} / \text{Uncompressed Bytes}) \times 100.0\%$.
- **FR-008**: Every in-memory decompression benchmark pass MUST perform 100% byte-for-byte roundtrip integrity verification (`memcmp` or SIMD hardware CRC32/SHA-256) comparing the restored buffer with the original source buffer.
- **FR-009**: The benchmark runner MUST support statistical result aggregation strategies: Peak/Max Speed (minimum elapsed time, TurboBench standard), Median Speed, and Mean $\pm$ Standard Deviation.
- **FR-010**: The CLI MUST expose dedicated flags (`--in-memory`, `--iterations <N>`, `--min-duration <ms>`, `--compat-turbobench`, `--json-report <path>`) for automated CI and external benchmark alignment.

### Key Entities

- **InMemoryBenchmarkConfig**: Encapsulates benchmark parameters including target algorithms, compression levels, memory buffer size, warmup pass count, minimum trial duration, and unit mode.
- **PlatformMonotonicTimer**: Low-level high-resolution monotonic time provider offering sub-microsecond precision across macOS, Windows, and POSIX platforms.
- **AlgorithmBenchmarkResult**: Encapsulates raw metrics for an individual test point (algorithm name, level, source bytes, compressed bytes, compression time ns, decompression time ns, compression MB/s, decompression MB/s, ratio %, verification status).
- **BenchmarkSuiteReport**: Aggregates matrix results, platform hardware metadata (CPU model, core count, frequency, OS version), and statistical distributions for export.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In-memory benchmark runs demonstrate a coefficient of variation ($CV \le 2.5\%$) across 10 identical consecutive trials on idle hardware.
- **SC-002**: High-resolution timer calibration achieves $< 100\text{ ns}$ tick resolution with zero drift and $< 0.1\%$ measurement overhead on operations lasting $\ge 1\text{ ms}$.
- **SC-003**: 100% of benchmarked compression/decompression operations undergo roundtrip byte-level verification with zero silent corruption.
- **SC-004**: Benchmark throughput and compression ratio metrics agree within $0.01\%$ with theoretical mathematical bounds and official TurboBench/lzbench formula outputs on identical memory buffers.
- **SC-005**: Full compatibility with both macOS (Clang/Apple Silicon & Intel) and Windows (MSVC) build targets with zero platform-specific `#if` leaks into public API signatures.

## Assumptions

- **A-001**: Sufficient host physical RAM is available to allocate working buffers (up to $500\text{ MB} \sim 1\text{ GB}$ total for high-capacity benchmark corpora) without triggering OS swap/paging.
- **A-002**: The benchmarking environment provides consistent CPU clock scaling when running in performance evaluation mode (users or CI scripts configure performance power profiles on test hosts).
