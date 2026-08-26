# Quickstart & Verification: AArch64 Pareto-Optimal compare256 Engine

## Verification Scenarios

### 1. Dual-Architecture Bit-Exact Validation (8,224 Test Cases)

- **Command**:
  ```bash
  clang -O3 scratch/verify_early_probe_suite.c -o scratch/verify_early_probe_suite && scratch/verify_early_probe_suite
  ```
- **Expected Output**:
  ```text
  ✅ Dual-Architecture Bit-Exact Validation: 8224 / 8224 combinations passed 100% bit-exact (AArch64 + ARMv7).
  ```
- **Failure Diagnostic**: If any offset or length fails, check bit index derivation `zng_first_diff_byte64(lane)`.

---

### 2. Standard zlib-ng CTest Regression Suite (71 Tests)

- **Command**:
  ```bash
  cd Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/build && cmake --build . -j && ctest --output-on-failure
  ```
- **Expected Output**:
  ```text
  100% tests passed out of 71
  ```
- **Failure Diagnostic**: If test fails, verify that `fallback_builtins.h` and `neon_intrins.h` prototypes match.

---

### 3. Full Short-Length (< 128 Bytes) Microbenchmark

- **Command**:
  ```bash
  clang -O3 scratch/bench_compare256_official_short.c -o scratch/bench_compare256_official_short && scratch/bench_compare256_official_short
  ```
- **Expected Output**:
  - Median latency table across lengths 0..128B with 0 regressions vs `develop`.
- **Failure Diagnostic**: Check system thermal throttling or background daemon CPU spikes.
