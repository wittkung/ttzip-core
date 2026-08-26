# Data Model: AArch64 Pareto-Optimal Zero-Regression compare256 Engine

## Entities

### `MatchResult`

Represents the output of comparing two memory blocks up to 256 bytes.

| Field | Type | Required | Constraints | Description |
| :--- | :--- | :---: | :--- | :--- |
| `matched_bytes` | `integer` (uint32) | Yes | `0 <= matched_bytes <= 256` | Number of identical contiguous bytes before first mismatch or 256 |
| `is_full_match` | `boolean` | Yes | `matched_bytes == 256` | True if all 256 bytes matched identically |

---

### `BenchmarkDataPoint`

Represents a single latency and throughput measurement in the microbenchmark matrix.

| Field | Type | Required | Constraints | Description |
| :--- | :--- | :---: | :--- | :--- |
| `input_length` | `integer` | Yes | `0 <= input_length <= 256` | Target match length in bytes |
| `baseline_ns` | `number` (float) | Yes | `> 0.0` | Median latency of `develop` baseline in nanoseconds |
| `candidate_ns` | `number` (float) | Yes | `> 0.0` | Median latency of Candidate engine in nanoseconds |
| `speedup_pct` | `number` (float) | Yes | Any real number | Performance difference percentage: `(baseline - candidate) / baseline * 100` |
| `status` | `string` | Yes | Enum: `["SPEEDUP", "PARITY", "REGRESSION"]` | Status classification based on $\pm 3.0\%$ threshold |

---

## Invariants

1. **Deterministic Identity**: For any two identical buffers of length $\ge 256$, `compare256(s0, s1)` MUST return exactly `256`.
2. **First Mismatch Precision**: If `s0[k] != s1[k]` and for all $0 \le i < k$, `s0[i] == s1[i]`, `compare256(s0, s1)` MUST return exactly `k`.
3. **Zero Buffer Overread**: Neither pointer shall be dereferenced past `src0 + 256` or `src1 + 256` in any branch.
