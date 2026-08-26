# Quickstart: 130-benchmark-harness-and-methodology-investigation

## Scenario 1: Execute Full Macro Deflate Benchmark Suite (25 Points)

### Command
```bash
cd Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256
./build/test/benchmarks/benchmark_zlib \
  --benchmark_data_types=all \
  --benchmark_filter="deflate_bench/level/.*/1048576/(1|3|6|9)" \
  --benchmark_format=json > /tmp/macro_bench.json
```

### Expected Output
```json
{
  "context": {
    "date": "2026-08-19...",
    "num_cpus": 18,
    "mhz_per_cpu": 4200
  },
  "benchmarks": [
    {
      "name": "deflate_bench/level/text/1048576/1",
      "run_name": "deflate_bench/level/text/1048576/1",
      "cpu_time": 1723000.0,
      "time_unit": "ns"
    }
  ]
}
```

### Failure Diagnostic
- **Issue**: `unknown data type` error.
- **Remedy**: Verify that `--benchmark_data_types=all` is passed and `benchmark_data_types.cc` is compiled into `benchmark_zlib`.
- **Issue**: High variance across runs ($> 5\%$).
- **Remedy**: Close high-load background applications; ensure RAM-to-RAM execution with zero active disk I/O.

---

## Scenario 2: Execute Match Counting Microbenchmark Sweep (0..256 Bytes)

### Command
```bash
cd Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256
./build/test/benchmarks/benchmark_zlib \
  --benchmark_filter="compare256/native" \
  --benchmark_format=json > /tmp/micro_bench.json
```

### Expected Output
```text
compare256/native/1       0.763 ns
compare256/native/10      0.921 ns
compare256/native/40      2.047 ns
compare256/native/80      1.752 ns
compare256/native/175     2.744 ns
compare256/native/256     3.437 ns
```

### Failure Diagnostic
- **Issue**: `compare256/native/1` latency $> 1.5\text{ ns}$.
- **Remedy**: Verify that the scalar extraction fast path (`vgetq_lane_u64`) is not using `vmaxvq_u8` on bytes 0..15.

---

## Scenario 3: Generate Automated Markdown Comparison Report

### Command
```bash
python3 /Users/kevintung/.gemini/antigravity/brain/3ac96734-1cc6-454b-a0a2-ea64d74fac52/scratch/generate_reply.py
```

### Expected Output
```text
Generated pr2416_maintainer_reply_benchmarks.md cleanly!
```

### Failure Diagnostic
- **Issue**: FileNotFoundError for baseline JSON.
- **Remedy**: Verify `develop_bench_all_types.json` exists in `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/`.
