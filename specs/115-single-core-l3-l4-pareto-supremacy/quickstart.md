# Quickstart: Single-Core L3/L4 Intermediate Pareto Dominance

## Verification Scenarios

### Scenario 1: Single-Core Pareto Frontier Benchmark Validation

Execute the full 1-thread single-core Pareto frontier PK benchmark against `libdeflate`, `7-Zip`, `Apple Native (zip/ditto)`, and `Apple libcompression`.

```bash
TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter ZipSingleCoreParetoFrontierPkTests
```

**Expected Output**:
```text
[PERF] [Single-Core Benchmark] Starting 100MB enwik8 pure 1-thread PK...
  [  2/ 22] [PERF] [TTZip 1-Core] Tier 1 (Fast (1))     (~26 ms) | >= 3.40 GB/s | ~4.16 MB
  [  3/ 22] [PERF] [TTZip 1-Core] Tier 2 (Fast+ (2))    (~25 ms) | >= 3.60 GB/s | ~4.16 MB
  [  4/ 22] [PERF] [TTZip 1-Core] Tier 3 (Normal (3))   (~75 ms) | >= 1.20 GB/s | ~3.35 MB
  [  5/ 22] [PERF] [TTZip 1-Core] Tier 4 (Maximum (4))  (~115 ms)| >= 850 MB/s  | ~3.20 MB
  [  9/ 22] [PERF] [libdeflate  ] Level 1               (~61 ms) | ~1.53 GB/s   | 4.01 MB
  [ 10/ 22] [PERF] [libdeflate  ] Level 3               (~87 ms) | ~1.07 GB/s   | 3.34 MB
  [ 11/ 22] [PERF] [libdeflate  ] Level 6               (~127 ms)| ~749 MB/s    | 3.21 MB
```

**Failure Diagnostic**:
- If Tier 3 throughput is $< 1.20\text{ GB/s}$, verify that `ttzip_deflate_fast_lazy` uses 128KB 2-way inline table and tail-only match skip.
- If Tier 4 space savings $< 66.5\%$ or throughput $< 850\text{ MB/s}$, verify that 2-step lookahead (`lazy2`) with distance bit weighting is active and state size is $\le 192\text{KB}$.
- If Tier 3 and Tier 4 throughputs are identical, check `ttzip_deflate_engine.c` dispatch options to ensure distinct profile mappings.

---

### Scenario 2: Intermediate Level Micro-Benchmark Verification

Validate throughput floors and ratio bounds for Level 3 and Level 4 compression directly:

```bash
swift test -c release --filter SingleCoreDeflatePkTests
```

**Expected Output**:
```text
Test Suite 'SingleCoreDeflatePkTests' passed with 0 failures (0 unexpected)
```

**Failure Diagnostic**:
- If test fails, check `SingleCoreDeflatePkTests.swift` assertions against threshold parameters.

---

### Scenario 3: Cross-Ecosystem Bit-Exact Oracle Verification

Validate 100% round-trip fidelity against system `/usr/bin/unzip` and standard `zlib`:

```bash
swift test -c release --filter SingleCoreDeflateOracleTests
```

**Expected Output**:
```text
Test Suite 'SingleCoreDeflateOracleTests' passed with 0 failures
```

**Failure Diagnostic**:
- If SHA-256 mismatch occurs, verify RFC 1951 bitstream alignment and dynamic tree header encoding in `ttzip_deflate_huffman.c`.
