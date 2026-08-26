# Research Report: TurboBench & lzbench In-Memory Benchmarking & High-Precision Timer Calibration Suite

## Research Item R001: In-Memory Benchmarking Buffer Lifecycle, Warmup Passes, and Multi-Iteration Loop Clamping

### 1. Decision
Adopt a **Unified 3-Buffer Pre-Allocated Harness with 1-Pass Warmup & 500ms Adaptive Time-Clamping** for TTZip's in-memory benchmarking engine:
1. **Buffer Lifecycle**: Pre-allocate `srcBuffer`, `compBuffer` (sized to `compressBound(N)`), and `decompBuffer` using `NativeCoreArchitecture.allocateAlignedPageBuffer(capacity:)` (16KB Apple Silicon page alignment).
2. **Warmup Protocol**: Run exactly 1 unmeasured full compress + decompress pass to prime L1/L2 caches, pre-fault memory pages, train branch predictors, ramp P-core clock rates, and assert round-trip bit-exactness.
3. **Adaptive Timing Clamping**: Clamp benchmark duration to a minimum timing window of $T_{\min} = 500\text{ ms}$ using dynamic batch scaling (`batch_size` geometric scaling when inner loop $< 10\text{ ms}$), measuring elapsed time via hardware monotonic timers.
4. **Zero-Allocation Hot Path**: Enforce zero `malloc`/`free`, zero `Data` copying/zeroing, zero ARC reference counting, and persistent codec context reuse inside the timing loop.

### 2. Rationale
* **Precision & Low Jitter**: Apple Silicon M-series chips have unified memory and high memory bandwidth (>100–800 GB/s). Ultra-fast codecs like LZ4, Snappy, and libdeflate Level 1 process small buffers in sub-millisecond windows. Clamping to $\ge 500\text{ ms}$ with adaptive batching eliminates timer quantization and context-switch noise.
* **Hardware Alignment**: 16KB alignment matches macOS Apple Silicon physical page size, preventing TLB thrashing, unaligned NEON 128-bit vector loads, and memory bus line-splits.
* **Warmup Fairness**: Eliminating cold-start page faults and DVFS ramp-up lag ensures fair, reproducible speed comparisons against competitor baselines (`7zz`, `keka`, `lzbench`, `TurboBench`).

### 3. Alternatives Considered
1. **Fixed Iteration Count ($N=10$)**: Rejected because for fast codecs (LZ4 at 15 GB/s on 10MB), 10 iterations take only ~6.6 ms (dominated by timer jitter and DVFS fluctuations); for slow codecs (LZMA2 Level 9 on 100MB), 10 iterations take >120 seconds, causing severe benchmark timeouts.
2. **Single-Pass Cold Benchmark**: Rejected because it measures OS demand paging faults, instruction cache cold misses, and CPU dynamic clock scaling latency rather than raw algorithmic compression throughput, introducing up to 35% random variance between consecutive runs.
3. **`Data(count:)` Swift Heap Buffers per Iteration**: Rejected because it forces kernel zero-fill page faults on every iteration and ARC retain/release overhead, artificially throttling measured throughput by 40–60%.

### 4. Source
* **powturbo/TurboBench**: [powturbo/TurboBench GitHub Repository](https://github.com/powturbo/TurboBench)
* **inikep/lzbench**: [inikep/lzbench GitHub Repository](https://github.com/inikep/lzbench)
* **Facebook Zstandard Benchmarking Harness**: `facebook/zstd/programs/benchfn.c` & `timefn.c`
* **TTZip Native Architecture**: `Sources/TTZipCore/NativeCoreArchitecture.swift`, `Sources/TTZipCore/Adapters/CUnsafeBufferAdapter.swift`, `GEMINI.md` (§4 Performance Invariants).

---

## Research Item R002: Cross-Platform Nanosecond Monotonic Hardware Timers

### 1. Decision
Implement a unified, header-only / C-bridge high-resolution hardware monotonic timer abstraction (`ttzip_monotonic_nanos()`) in `Sources/CTTZipBridge/` and bridge it to Swift `PlatformMonotonicTimer`:
1. **macOS / Apple Silicon**: Use `mach_absolute_time()` with cached `mach_timebase_info` using `__int128` multiplication (or `clock_gettime_nsec_np(CLOCK_UPTIME_RAW)` on Darwin).
2. **Windows (x64 & ARM64)**: Use `QueryPerformanceCounter()` with cached `QueryPerformanceFrequency()` using `__int128` / decomposed 64-bit integer math.
3. **Linux / POSIX**: Use `clock_gettime(CLOCK_MONOTONIC_RAW, &ts)` converted to `uint64_t` nanoseconds.
4. **Deprecation**: Remove all `CACurrentMediaTime()` invocations across `Sources/TTZipCLI/` and `Sources/TTZipCore/Benchmark/`, replacing them with `PlatformMonotonicTimer.nowNanoseconds()`.

### 2. Rationale
* Eliminates the heavy `QuartzCore.framework` dependency from CLI benchmark runners and core compression engines, enabling headless execution on Linux/Windows.
* Guarantees nanosecond precision without floating-point truncation or system uptime overflow.
* Directly aligns TTZip's micro-benchmarking precision with industry-standard memory benchmark suites (powturbo/TurboBench and inikep/lzbench).

### 3. Alternatives Considered
1. **Using `CACurrentMediaTime()`**: Rejected because it requires linking Apple UI frameworks (`QuartzCore`), cannot compile on Windows or Linux for cross-platform `ttzip-cli`, and introduces floating-point rounding errors on extended runs.
2. **Using POSIX `gettimeofday()`**: Rejected because it only offers microsecond resolution ($1\,\mu\text{s}$) and is non-monotonic, causing potential negative elapsed times during NTP step adjustments.
3. **Using Standard `ContinuousClock` in Swift**: Considered, but rejected for C-level slice profiler integration (`Sources/CTTZipBridge/CTTZipSliceProfiler.c`), where zero-overhead C calling convention is required across C and Swift boundaries without Swift runtime allocation.

### 4. Source
* Apple Developer Documentation: `mach_absolute_time`, `mach_timebase_info`, `clock_gettime_nsec_np(3)`
* Microsoft Learn: *Acquiring high-resolution time stamps* (`QueryPerformanceCounter`, `QueryPerformanceFrequency`)
* Linux Programmer's Manual: `clock_gettime(2)`, `CLOCK_MONOTONIC_RAW`, vDSO kernel documentation
* Existing Codebase Reference: `Sources/CTTZipBridge/CTTZipSliceProfiler.c` and `Sources/TTZipCLI/CLIBenchmarkRunner.swift`

---

## Research Item R003: Standard Throughput Calculation, Statistical Aggregations, and TurboBench / lzbench Parity Output

### 1. Decision
1. **Throughput Formula**: Adopt the industry-standard throughput formula based on uncompressed bytes for both compression and decompression, supporting explicit unit metadata:
   $$\text{Compression Throughput (MB/s)} = \frac{\text{Uncompressed Bytes}}{\text{Elapsed Time (Seconds)} \times 1,000,000}$$
   $$\text{Decompression Throughput (MB/s)} = \frac{\text{Uncompressed Bytes}}{\text{Elapsed Time (Seconds)} \times 1,000,000}$$
   *(Optional `--binary-units` flag for $\text{MiB/s} = \text{Bytes} / (\text{Seconds} \times 1,048,576)$).*
2. **Peak Aggregation**: Maintain multi-pass benchmarking with a mandatory warm-up pass and $\min(\text{duration})$ aggregation for peak throughput determination (TurboBench standard).
3. **Dual-Tier Verification**: Implement `memcmp` for in-memory buffer roundtrips, and hardware NEON CRC32 (`ttzip_compute_buffer_crc32_neon`) for multi-file/streaming datasets.
4. **Parity Reports**: Generate TurboBench/lzbench-compatible JSON and Markdown reports containing explicit `ratio` ($U/C$), `space_savings` ($1 - C/U$), and separate compression/decompression durations.

### 2. Rationale
* Guarantees 1:1 mathematical equivalence with TurboBench, lzbench, and Zstandard benchmarks, allowing TTZip benchmark numbers to be directly compared against published academic and open-source benchmarks without unit confusion.
* Using $\min(\text{duration})$ eliminates macOS Darwin thread scheduling variance and background task latency, reflecting true SIMD and pipeline efficiency.
* SIMD NEON CRC32 and `memcmp` prevent integrity verification from introducing I/O or memory bandwidth bottlenecks into the benchmarking loop.

### 3. Alternatives Considered
1. **Arithmetic Mean / Trimmed Mean**: Rejected because CPU throttling and OS interrupts skew the average upwards, making benchmark runs non-reproducible across different background system loads.
2. **Decompression throughput calculated on Compressed Input Size**: Rejected because it violates compression benchmarking standards (compressors with worse compression ratios would falsely appear faster due to larger input buffers).
3. **Full SHA-256 for all in-memory verification**: Rejected because SHA-256 (even hardware-accelerated) tops out around 3–4 GB/s on single cores, which would throttle verification when benchmarking 15,000+ MB/s compressors (like LZ4 or direct ZSTD).

### 4. Source
* `https://github.com/powturbo/TurboBench` (powturbo TurboBench architecture, metrics, and multi-format outputs)
* `https://github.com/inikep/lzbench` (`lzbench.cpp`, documentation, `-t` timing loops, and ratio definitions)
* `Sources/TTZipCore/Benchmark/CompetitorBenchmarkModels.swift`
* `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift`
* `Sources/TTZipCore/Benchmark/CompetitorReportWriter.swift`
* `Sources/CTTZipBridge/CTTZipUtils.c` (`ttzip_compute_buffer_crc32_neon`, `libdeflate_crc32`)
