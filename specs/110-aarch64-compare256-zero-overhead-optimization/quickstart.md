# Quickstart & Verification Guide: AArch64 compare256 Zero-Overhead Optimization

**Feature Directory**: `specs/110-aarch64-compare256-zero-overhead-optimization`  
**Date**: 2026-08-19  

---

## 1. Standalone Microbenchmark Validation

### Command
```bash
clang -O3 -Wall -Wextra \
  -I Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256 \
  -I Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/build \
  scratch/bench_arch_spectrum.c \
  -o scratch/bench_arch_spectrum && ./scratch/bench_arch_spectrum
```

### Expected Output
```text
====================================================================
⚡ Comparing Architectures across Short & Long Match Scenarios
====================================================================
Match Length =   4 bytes: <= 0.35 ns/op (Zero regression floor)
Match Length =   8 bytes: <= 0.38 ns/op
Match Length =  32 bytes: <= 0.75 ns/op
Match Length = 256 bytes: <= 1.50 ns/op (>= 3x speedup vs 4.95ns)
```

### Failure Diagnostic
- If 4-byte latency exceeds `0.35 ns`, verify that the initial 8-byte comparison is executed strictly in GPR64 without `FMOV` or vector reduction intrinsics.
- If 256-byte latency exceeds `1.50 ns`, check loop condition and ensure `LDP` or unrolled vector loads are functioning.

---

## 2. Full 8-Data-Type Macro Deflate Verification

### Command
```bash
cd Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256
./build/test/benchmarks/benchmark_zlib \
  --benchmark_data_types=all \
  --benchmark_filter="deflate_bench/level/.*" \
  --benchmark_repetitions=5 \
  --benchmark_report_aggregates_only=true
```

### Expected Output
- All 8 data types (`text`, `short_match`, `dna`, `random`, `literals`, `mixed`, `realistic_rgb`, `striped_rgb`) exhibit **$\le 1.0\%$ variance on worst-case and $\ge 3.0\%$ gain on structured/long-match datasets**.
- Zero regression on `literals` or random data.
