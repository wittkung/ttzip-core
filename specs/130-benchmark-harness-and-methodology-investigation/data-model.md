# Data Model: 130-benchmark-harness-and-methodology-investigation

## Entities

### 1. `MacroBenchmarkPoint`
Represents a single end-to-end macro compression measurement point on a defined corpus and compression level.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `corpus` | `string` (enum) | Yes | Corpus identifier: `text`, `striped_rgb`, `random`, `dna`, `literals`, `short_match`, `mixed`, `realistic_rgb` |
| `level` | `integer` (1..9) | Yes | Deflate compression level (1, 3, 6, 9) |
| `buffer_size` | `integer` | Yes | In-memory buffer size in bytes (e.g. `1048576` for 1MB) |
| `baseline_time_ms` | `number` | Yes | Baseline execution time in milliseconds |
| `candidate_time_ms` | `number` | Yes | Candidate build execution time in milliseconds |
| `delta_percent` | `number` | Yes | Relative speedup: $(T_{\text{base}} - T_{\text{cand}}) / T_{\text{base}} \times 100\%$ |
| `status` | `string` (enum) | Yes | Rating: `GAIN`, `FLAT`, `WARNING`, `CRITICAL_REGRESSION` |

---

### 2. `MicroBenchmarkPoint`
Represents a nanosecond-precision microarchitectural match comparison latency measurement for a given match length.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `match_length` | `integer` (0..256) | Yes | Target identical byte length (0 to 256) |
| `description` | `string` | Yes | Microarchitectural stage description |
| `baseline_latency_ns` | `number` | Yes | Baseline latency in nanoseconds |
| `candidate_latency_ns` | `number` | Yes | Candidate build latency in nanoseconds |
| `delta_percent` | `number` | Yes | Relative speedup in latency |
| `execution_stage` | `string` (enum) | Yes | Architecture tier: `SCALAR_SUBREGISTER`, `DISCRETE_STEPPING`, `UNROLLED_HIGHWAY`, `TAIL_BOUNDARY` |

---

### 3. `BenchmarkSuiteReport`
Represents the complete structured report containing execution metadata, macro matrix, micro spectrum, and regression verdict.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `timestamp` | `string` (ISO-8601) | Yes | Run timestamp |
| `platform` | `string` | Yes | Hardware CPU and memory configuration string |
| `os_version` | `string` | Yes | Operating system kernel and build version |
| `compiler` | `string` | Yes | Compiler version and optimization flags (`-O3 -DNDEBUG`) |
| `macro_points` | `array<MacroBenchmarkPoint>` | Yes | List of all 25 macro measurement points |
| `micro_points` | `array<MicroBenchmarkPoint>` | Yes | List of microarchitectural spectrum points |
| `total_points` | `integer` | Yes | Total measured points count (e.g. 25) |
| `improved_points` | `integer` | Yes | Count of improved points ($\Delta\% > 0$) |
| `regressed_points` | `integer` | Yes | Count of regressed points ($\Delta\% < -3.0\%$) |
| `verdict` | `string` (enum) | Yes | Final status: `PASS_ALL_GREEN`, `PASS_WITH_WARNINGS`, `BLOCKED_REGRESSION` |
