# Quickstart: libdeflate-Aligned Single-Core DEFLATE Engine with Apple Silicon Optimization

## Verification Scenarios

### Scenario 1: Pareto Dominance Benchmark against libdeflate

Execute the 1-thread single-core Pareto frontier PK benchmark on 100MB `enwik8`:

```bash
TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter ZipSingleCoreParetoFrontierPkTests
```

**Expected Output**:
```text
[PERF] [Single-Core Benchmark] Starting 100MB enwik8 pure 1-thread PK...
  [  1/ 22] [PERF] [TTZip 1-Core] Tier 0 (Store (0))                         (5.90 ms) | >= 15.00 GB/s | 95.37 MB
  [  2/ 22] [PERF] [TTZip 1-Core] Tier 1 (Fast (1))                          (53.00 ms)| >= 1.70 GB/s  | ~4.11 MB
  [  3/ 22] [PERF] [TTZip 1-Core] Tier 2 (Fast+ (2))                         (52.00 ms)| >= 1.75 GB/s  | ~4.11 MB
  [  4/ 22] [PERF] [TTZip 1-Core] Tier 3 (Normal (3))                        (75.00 ms)| >= 1.20 GB/s  | <= 3.34 MB
  [  5/ 22] [PERF] [TTZip 1-Core] Tier 4 (Maximum (4))                       (115.0 ms)| >= 800 MB/s   | <= 3.21 MB
  [  9/ 22] [PERF] [libdeflate  ] Level 1                                    (62.49 ms)| ~1.49 GB/s    | 4.01 MB
  [ 10/ 22] [PERF] [libdeflate  ] Level 3                                    (87.36 ms)| ~1.07 GB/s    | 3.34 MB
  [ 11/ 22] [PERF] [libdeflate  ] Level 6                                    (132.1 ms)| ~722 MB/s     | 3.21 MB
```

**Failure Diagnostic**:
- If Tier 3 size $> 3.34\text{ MB}$, check `block_splitting` soft max block length ($\ge 300\text{ KB}$) and 16-bit `hc_matchfinder` hash3/hash4 table wiring.
- If Tier 4 throughput $< 800\text{ MB/s}$, verify that `lz_extend_neon` Tier-0 GPR SWAR exit and `hc_matchfinder` multi-candidate load unrolling are active.

---

### Scenario 2: Intermediate Level Micro-Benchmark Verification

```bash
swift test -c release --filter SingleCoreDeflatePkTests
```

**Expected Output**:
```text
Test Suite 'SingleCoreDeflatePkTests' passed with 0 failures
```

---

### Scenario 3: Bit-Exact Oracle Verification

```bash
swift test -c release --filter SingleCoreDeflateOracleTests
```

**Expected Output**:
```text
Test Suite 'SingleCoreDeflateOracleTests' passed with 0 failures
```
