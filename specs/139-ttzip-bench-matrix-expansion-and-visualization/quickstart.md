# Quickstart: ttzip-bench Multi-Format Matrix Expansion & Interactive Visualizations

**Feature Directory**: `specs/139-ttzip-bench-matrix-expansion-and-visualization`  
**Status**: Ready  

---

## Scenario 1: Execute Full Multi-Engine Matrix

### Command
```bash
swift run ttzip-bench matrix
```

### Expected Output
```text
==========================================================================================================================
⚡️ TTZip Deflate-Bench Unified In-Memory Matrix (Total Points: 74)
==========================================================================================================================
[Idx] Engine     | Corpus        | Size  | Lvl | Comp Time  | Comp Rate   | Decomp Time| Decomp Rate | Ratio  | Status
--------------------------------------------------------------------------------------------------------------------------
[ 1] libdeflate | text          | 128KB | L1  |    60.4 µs | 2070.4 MB/s |    25.0 µs | 5000.0 MB/s |   9.0% | OK
...
[74] bzip2      | text          | 128KB | L9  |   1.45 ms  |   86.2 MB/s |   380.0 µs |  328.9 MB/s |   8.5% | OK
--------------------------------------------------------------------------------------------------------------------------
Summary: 74/74 Points PASSED | Total Matrix Time: 1.420s | Median CV: 0.95%
==========================================================================================================================
```

### Failure Diagnostic
- If exit code is non-zero, check `Status` column for any row failing roundtrip decompression byte verification.
- If duration exceeds 2.5s, check system background load and CPU throttling.

---

## Scenario 2: Generate Interactive Vector SVG & Standalone HTML Dashboard

### Command
```bash
swift run ttzip-bench plot --svg-out docs/benchmarks/pareto.svg --html-out docs/benchmarks/dashboard.html
```

### Expected Output
```text
📈 Interactive SVG Pareto chart exported: docs/benchmarks/pareto.svg
🌐 Self-contained Zen UI HTML Dashboard exported: docs/benchmarks/dashboard.html
```

### Failure Diagnostic
- If file writing fails, ensure target directory `docs/benchmarks/` exists.
- Open `docs/benchmarks/dashboard.html` in Safari/Chrome to verify interactive tooltips render without console errors.

---

## Scenario 3: Point-to-Point Benchmark Diff & Automated CI Regression Gate

### Command
```bash
swift run ttzip-bench matrix --json-out baseline.json
swift run ttzip-bench diff baseline.json baseline.json --fail-pct 5.0
```

### Expected Output
```text
==========================================================================================================================
📊 TTZip Codec Benchmark Regression Differential (Baseline vs. Candidate)
==========================================================================================================================
[Idx] Engine     | Corpus        | Size  | Lvl | Base Speed  | Cand Speed  | Delta %   | Status
--------------------------------------------------------------------------------------------------------------------------
[ 1] libdeflate | text          | 128KB | L1  | 2070.4 MB/s | 2070.4 MB/s |   +0.00%  | 🟢 FLAT
...
==========================================================================================================================
Summary: 74/74 Points Analyzed | 0 Regressions Detected | Overall Verdict: PASS
==========================================================================================================================
```

### Failure Diagnostic
- If exit code is 70 (`EX_SOFTWARE`), locate lines marked `🔴 REG` with $> 5.0\%$ drop and check compiler optimization flags.
