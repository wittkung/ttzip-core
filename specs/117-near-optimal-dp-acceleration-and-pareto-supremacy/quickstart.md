# Quickstart: Near-Optimal DP Acceleration and Full-Spectrum Pareto Supremacy

## Verification Scenarios

### Scenario 1: 8-Tier Single-Core Pareto PK Benchmark

Run the full Pareto frontier PK suite on 100MB `enwik8`:

```bash
TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter ZipSingleCoreParetoFrontierPkTests
```

**Expected Output**:
```text
[PERF] [Single-Core Benchmark] Starting 100MB enwik8 pure 1-thread PK...
  [  1/ 22] [PERF] [TTZip 1-Core] Tier 0 (Store (0))                         | >= 10.0 GB/s | 95.37 MB
  [  2/ 22] [PERF] [TTZip 1-Core] Tier 1 (Fast (1))                          | >= 1.60 GB/s | 4.11 MB
  [  3/ 22] [PERF] [TTZip 1-Core] Tier 2 (Normal (2))                        | >= 1.20 GB/s | <= 3.34 MB
  [  4/ 22] [PERF] [TTZip 1-Core] Tier 3 (Maximum (3))                       | >= 880 MB/s  | <= 3.21 MB
  [  5/ 22] [PERF] [TTZip 1-Core] Tier 4 (High (4))                          | >= 35 MB/s   | <= 3.03 MB
  [  6/ 22] [PERF] [TTZip 1-Core] Tier 5 (Graph Fast (5))                    | ~1.5 MB/s    | 2.86 MB
  [  7/ 22] [PERF] [TTZip 1-Core] Tier 6 (Ultra Zopfli (6))                  | ~0.9 MB/s    | 2.85 MB
  [  8/ 22] [PERF] [TTZip 1-Core] Tier 7 (Extreme Peak (7))                  | ~0.4 MB/s    | 2.85 MB
```

**Failure Diagnostic**:
- If Tier 4 throughput $< 35\text{ MB/s}$, verify that slot boundary pruning is enabled in `deflate_find_min_cost_path` and `max_optim_passes <= 4`.
- If Tier 4 compressed size $> 3.05\text{ MB}$, check that Pareto edge pruning preserves slot endpoints and maximum match lengths.

---

### Scenario 2: Near-Optimal DP Fast Convergence Micro-Tests

```bash
swift test -c release --filter SingleCoreDeflatePkTests
```

**Expected Output**:
```text
Test Suite 'SingleCoreDeflatePkTests' passed with 0 failures
```
