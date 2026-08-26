# Data Model: TurboBench & lzbench In-Memory Benchmarking & High-Precision Timer Calibration Suite

## 1. Entities & Value Objects

### 1.1 `InMemoryBenchmarkConfig`
Encapsulates runtime execution options for in-memory algorithm benchmarking:

| Field Name | Type | Required | Description / Constraints |
| :--- | :--- | :--- | :--- |
| `selectedFormats` | `[String]` | Yes | Target format algorithm tokens (e.g., `["zip", "7z", "zstd", "lz4", "brotli"]`). |
| `selectedLevels` | `[Int]` | Yes | Target compression levels (e.g., `[1, 3, 6, 9]`). |
| `bufferSizeBytes` | `Int64` | Yes | Size of the in-memory test payload in bytes ($1\text{ MB} \le N \le 1\text{ GB}$). |
| `warmupPasses` | `Int` | Yes | Number of untimed warmup passes prior to measurement ($\ge 1$, default 2). |
| `minDurationMs` | `Int` | Yes | Target minimum accumulated timing window per test slice in milliseconds ($\ge 100$, default 500). |
| `useBinaryUnits` | `Bool` | Yes | If `true`, outputs binary $\text{MiB/s}$ ($2^{20}\text{ B/s}$); if `false`, outputs decimal $\text{MB/s}$ ($10^6\text{ B/s}$). |
| `turboBenchOutput` | `Bool` | Yes | If `true`, outputs Markdown table conforming strictly to TurboBench layout. |

### 1.2 `PlatformTimerCalibrationInfo`
Captures low-level hardware timer diagnostic metadata:

| Field Name | Type | Required | Description / Constraints |
| :--- | :--- | :--- | :--- |
| `platformOS` | `String` | Yes | Operating system identifier (`"macOS"`, `"Windows"`, `"Linux"`). |
| `architecture` | `String` | Yes | CPU architecture (`"arm64"`, `"x86_64"`). |
| `timerBackend` | `String` | Yes | Monotonic clock primitive name (`"mach_absolute_time"`, `"QueryPerformanceCounter"`, `"CLOCK_MONOTONIC_RAW"`). |
| `frequencyHz` | `UInt64` | Yes | Base hardware clock frequency in Hertz (e.g., $24,000,000$ on Apple Silicon, $10,000,000$ on Windows QPC). |
| `timebaseNumer` | `UInt32` | Yes | Timebase conversion numerator multiplier. |
| `timebaseDenom` | `UInt32` | Yes | Timebase conversion denominator divisor. |
| `resolutionNanos` | `Double` | Yes | Measured single-tick resolution in nanoseconds. |
| `overheadNanos` | `Double` | Yes | Average timer invocation latency overhead in nanoseconds. |

### 1.3 `AlgorithmBenchmarkResult`
Records execution metrics for an individual algorithm/level combination:

| Field Name | Type | Required | Description / Constraints |
| :--- | :--- | :--- | :--- |
| `algorithm` | `String` | Yes | Full algorithm / format identifier (e.g., `"ZIP-Deflate"`, `"7Z-LZMA2"`, `"ZSTD"`, `"LZ4"`). |
| `level` | `Int` | Yes | Compression level parameter. |
| `uncompressedBytes`| `Int64` | Yes | Raw uncompressed buffer size in bytes. |
| `compressedBytes` | `Int64` | Yes | Compressed output buffer size in bytes. |
| `ratio` | `Double` | Yes | Standard compression factor ($\text{Uncompressed} / \text{Compressed}$, e.g. $2.50$). |
| `spaceSavingsPct` | `Double` | Yes | Space savings percentage ($[1 - \text{Compressed}/\text{Uncompressed}] \times 100.0\%$). |
| `compressionTimeNs` | `UInt64` | Yes | Best / minimum elapsed compression duration in nanoseconds. |
| `decompressionTimeNs`| `UInt64` | Yes | Best / minimum elapsed decompression duration in nanoseconds. |
| `compressionSpeedMBs` | `Double` | Yes | Compression throughput in decimal MB/s (or binary MiB/s if configured). |
| `decompressionSpeedMBs` | `Double` | Yes | Decompression throughput in decimal MB/s (or binary MiB/s if configured). |
| `iterationsCompleted` | `Int` | Yes | Total measured iterations executed within the time clamping window. |
| `integrityVerified` | `Bool` | Yes | `true` if roundtrip `memcmp` / CRC32 matched 100% byte-for-byte. |

### 1.4 `BenchmarkSuiteReport`
Aggregates all execution results and environmental telemetry:

| Field Name | Type | Required | Description / Constraints |
| :--- | :--- | :--- | :--- |
| `reportId` | `String` | Yes | Unique UUID string identifying this benchmark execution. |
| `timestamp` | `String` | Yes | ISO 8601 UTC timestamp string. |
| `timerCalibration` | `PlatformTimerCalibrationInfo` | Yes | Hardware timer calibration diagnostics. |
| `totalInputBytes` | `Int64` | Yes | Cumulative raw byte volume processed. |
| `totalWallDurationMs` | `Double` | Yes | Overall wall-clock execution duration in milliseconds. |
| `results` | `[AlgorithmBenchmarkResult]` | Yes | Collection of per-algorithm measurement entries. |
| `allPassed` | `Bool` | Yes | `true` if all tests completed and verified with zero errors. |
