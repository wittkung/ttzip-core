# Quickstart & Verification Guide: Comprehensive CPI & Microarchitectural Optimization Audit

**Feature ID**: `160-cpi-microarchitecture-optimization-audit`  
**Created**: 2026-08-20  
**Status**: Ready for Tasks  

---

## 1. Scenario 1: Native C CPI & Codec Benchmark Execution

### Command
```bash
cmake -B build -DCMAKE_BUILD_TYPE=Release && cmake --build build --target ttzip_benchmark_runner && ./build/tests/c/ttzip_benchmark_runner --codecs --checksums
```

### Expected Output
- Execution completes in $< 2.0\text{s}$.
- Output table contains columns: `Kernel`, `Size`, `Time (ns)`, `Speed (MB/s)`, `CPB`, `Est. IPC`.
- PMULL CRC32 achieves $\text{CPB} \le 0.06$ on buffers $\ge 1\text{MB}$.
- Adler32 NEON achieves $\text{CPB} \le 0.15$ on buffers $\ge 1\text{MB}$.

### Failure Diagnostic
- If `CPB` is $> 0.20$ on 1MB PMULL CRC32: check if ARM ACLE target `+crypto` or `__ARM_FEATURE_CRC32` is missing from compiler flags in `CMakeLists.txt`.
- If build fails: inspect `tests/c/ttzip_benchmark_harness.h` for syntax errors or platform monotonic clock compatibility.

---

## 2. Scenario 2: C Unit & Microarchitectural Regression Suite

### Command
```bash
cmake -B build -DCMAKE_BUILD_TYPE=Debug -DENABLE_ASAN=ON && cmake --build build --target ttzip_test_runner && ./build/tests/c/ttzip_test_runner
```

### Expected Output
- All unit tests in `tests/c/test_*.c` pass with `0` failures.
- AddressSanitizer and UndefinedBehaviorSanitizer report `0 leaks` and `0 errors`.

### Failure Diagnostic
- If `test_matchfinder_neon` fails: verify that `ttzip_neon_match_len` correctly calculates match length across 0, 1..7, 8..15, 16..63, and $\ge 64$ byte boundaries without off-by-one errors.
- If ASan reports out-of-bounds read: check pointer limit arithmetic in GPR SWAR unrolling.

---

## 3. Scenario 3: Full 5-Stage Local CI Pipeline

### Command
```bash
./scripts/local-ci.sh
```

### Expected Output
- All 5 stages (Static Analysis, Swift Build, Swift Tests, CMake Native C Build, CMake Native C Tests) complete with status `PASS`.
- `0 compiler warnings` and `0 linker warnings`.

### Failure Diagnostic
- If Swift build fails: check if any C bridge header signatures were altered incompatibly.
- If Git hooks block: check for unformatted files or lingering debug `printf` statements.
