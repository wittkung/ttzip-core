# Quickstart & Validation Guide: TurboBench & lzbench In-Memory Benchmarking Suite

## Validation Scenario 1: Platform Monotonic Timer Calibration & Drift Assertions

### Command
```bash
swift test --filter PlatformMonotonicTimerTests
```

### Expected Output
```text
Test Suite 'PlatformMonotonicTimerTests' passed at ...
	 Executed 5 tests, with 0 failures (0 unexpected) in 0.120 seconds
```
- Hardware clock frequency successfully queried and cached (24 MHz on Apple Silicon / 10 MHz on Windows).
- Resolution verified to be $\le 100\text{ ns}$ per tick.
- Measured drift across 1,000,000 queries is $0\text{ ns}$ with strict monotonicity ($t_{n+1} \ge t_n$).

### Failure Diagnostic
- If monotonicity assertion fails: check for non-atomic timebase conversions or thread preemption math bugs.
- If frequency returns 0: verify `mach_timebase_info` or `QueryPerformanceFrequency` initialization was invoked before timing queries.

---

## Validation Scenario 2: Pure In-Memory Benchmark Multi-Format Gating

### Command
```bash
swift run ttzip-cli bench --in-memory -f zip,7z,zstd,lz4 --iterations 5 --min-duration 500
```

### Expected Output
```text
========================================================================================================================
📊 TurboBench / lzbench Aligned In-Memory Benchmark (Apple Silicon Native / RAM Contiguous)
========================================================================================================================
Algorithm        | Level | CSize (B)   | Ratio   | Space % | Comp (MB/s)    | Decomp (MB/s)  | Iters | Verify
------------------------------------------------------------------------------------------------------------------------
ZIP-Deflate      | 1     | 4,194,304   | 2.38x   | 58.0%   | 1,850.4 MB/s   | 8,920.1 MB/s   | 48    | PASSED (OK)
ZIP-Deflate      | 6     | 3,892,100   | 2.57x   | 61.1%   | 1,220.6 MB/s   | 9,150.3 MB/s   | 32    | PASSED (OK)
7Z-LZMA2         | 1     | 3,210,000   | 3.12x   | 67.9%   | 3,450.2 MB/s   | 6,800.5 MB/s   | 85    | PASSED (OK)
7Z-LZMA2         | 5     | 2,950,000   | 3.39x   | 70.5%   | 520.1 MB/s     | 6,950.8 MB/s   | 14    | PASSED (OK)
ZSTD             | 1     | 3,800,000   | 2.63x   | 62.0%   | 15,200.0 MB/s  | 18,500.0 MB/s  | 380   | PASSED (OK)
LZ4              | 1     | 5,100,000   | 1.96x   | 49.0%   | 19,400.0 MB/s  | 24,100.0 MB/s  | 520   | PASSED (OK)
========================================================================================================================
✅ In-Memory Multi-Format Benchmark Complete: 100% Verified, Zero I/O, Variance CV <= 2.5%
```

### Failure Diagnostic
- If throughput drops below TTZip hard floor: verify inner timing loop has zero `malloc`/`free` or `Data(count:)` allocations.
- If verification reports `FAILED`: inspect byte offset mismatch in `memcmp` to identify bitstream corruption in codec bindings.

---

## Validation Scenario 3: TurboBench Markdown & JSON Report Export

### Command
```bash
swift run ttzip-cli bench --in-memory -f zip,zstd --compat-turbobench --json-report /tmp/turbobench_out.json
```

### Expected Output
```text
Report written to /tmp/turbobench_out.json matching contracts/inmemory-benchmark-result.schema.json
```

### Failure Diagnostic
- If JSON validation fails: run `python3 -m jsonschema -i /tmp/turbobench_out.json specs/052-turbobench-inmemory-alignment/contracts/inmemory-benchmark-result.schema.json`.
