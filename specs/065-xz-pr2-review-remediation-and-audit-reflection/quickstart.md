# Quickstart: Reproducing CRC64 PMULL Benchmark & Verification

## 1. Standalone 1-Command Benchmark Reproduction

Compile and run the standalone benchmark (tested on macOS Apple Silicon & Linux AArch64):

```bash
clang -O3 scratch/reproduce_bench_crc64.c -o scratch/reproduce_bench_crc64 && scratch/reproduce_bench_crc64
```

### Expected Output
```text
[1/2] Standard Test Vectors:
  - Input: '123456789' (9 bytes) -> CRC64: 0x6C40DF5F0B497347 [PASS]
  - Input: 64 KB pseudo-random -> CRC64 match: PASS (100% Exact)

[2/2] Running Benchmark (3.12 GB across 50 iterations):
  1. Generic Slice-by-4 : 2.3374 s | 1,369.03 MB/s (1.34 GB/s)
  2. ARM64 PMULL        : 0.0666 s | 48,075.48 MB/s (46.95 GB/s)
  >>> Speedup: 35.12x faster (+3411.7%) <<<
```

### Failure Diagnostic
- If `vmull_p64` is unavailable, ensure the compiler supports ARMv8-A Crypto extensions (`-march=armv8-a+crypto`).

---

## 2. Running XZ Worktree CTest Regression

```bash
ctest --test-dir Vendor/worktrees/xz/pr2-arm64-crc64/build-asan --output-on-failure
```
