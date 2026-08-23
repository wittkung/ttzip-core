# Research Findings: AArch64 compare256 Microarchitectural Optimization

**Feature Directory**: `specs/110-aarch64-compare256-zero-overhead-optimization`  
**Target Upstream**: `zlib-ng/zlib-ng` (`arch/arm/compare256_neon.c`)  

---

## R001: Industry Match Counting & Memory Comparison Survey

- **Decision**: Adopt NEON wide-vector pipelining over pure 64-bit scalar SWAR for zlib-ng.
- **Rationale**: While `libdeflate` and `zstd` use 64-bit GPR SWAR (`LDR` + `EOR` + `CTZ`) for general match counting, in Deflate match finding with `longest_match` and hash chains, 8–16B match candidates (`short_match`) suffer a severe **-9.8% regression** under pure SWAR due to the need for 2 separate 64-bit memory loads and pointer increments compared to a single 128-bit vector load.
- **Alternatives Considered**: Pure 64-bit SWAR (rejected due to -9.8% `short_match` regression and -13.5% `random` L9 regression).
- **Source**: `libdeflate/lib/matchfinder_common.h`, `zstd/lib/common/zstd_internal.h`, and empirical measurements in `scratch/compare_pure_swar.py`.

---

## R002: AArch64 Pipeline, Dual Vector Load Pipes & UMAXV Latency

- **Decision**: Process 32 bytes per loop iteration combining two 16B XORs with `VORR` and a single `UMAXV`.
- **Rationale**: On Apple Silicon (Firestorm/Avalanche/M4/M5 cores) and ARM Neoverse (V1/V2), there are **two independent 128-bit SIMD load execution ports**. Loading two 16B vectors back-to-back (`LDR Q0` / `LDR Q2`) executes concurrently in 1 cycle. Merging their XOR differences with `VORR.16B` and performing a single `UMAXV.16B` reduces the reduction latency from 6 cycles (two 3-cycle UMAXVs) to 4 cycles (one 1-cycle VORR + one 3-cycle UMAXV) per 32 bytes.
- **Alternatives Considered**: 16-byte single UMAXV (PR #2416 original, rejected due to +4.3% `literals` regression caused by un-amortized 3-cycle reduction latency on every 16B chunk).
- **Source**: ARM Architecture Reference Manual ARMv8-A, Apple Silicon microarchitecture port mappings, `otool -tv` disassembly analysis.

---

## R003: Inlining Footprint & Stack Spill Safety

- **Decision**: Maintain strict post-indexed addressing (`COMPARE256_NEON_POSTINDEX`) and clean straight-line unrolling.
- **Rationale**: `longest_match_neon` inlines `compare256_neon_static`. The 2x-unrolled loop expands to 8 iterations of straight-line 32-byte chunks. The compiler allocates vector registers `v0..v3` and GPRs `x8..x9` without spilling any callee-saved registers to stack (`0 Spilling`).
- **Alternatives Considered**: Dynamic branching / two-tier runtime dispatch inside `compare256_neon_static` (rejected because extra branches disrupt loop unrolling and add +32 bytes of branch misprediction cost).
- **Source**: `build/CMakeFiles/zlib-ng-static.dir/arch/arm/compare256_neon.c.o` `otool -tv` output.

---

## R004: Empirical Five-Architecture Full-Matrix Comparison

- **Decision**: Select `Unrolled2x` (2x Unrolled NEON + VORR + Single UMAXV) as the global optimum.
- **Rationale**: Physical 5-repetition median benchmarks across all 25 official Google Benchmark test points on Apple M5 Max:

| Metric | Develop Baseline | PR #2416 (16B UMAXV) | Pure SWAR64 | Cascaded | **Unrolled2x (Selected)** |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Wins / 25 points** | 0 | 6 | 4 | 3 | **12 (Winner)** |
| **`literals` L3 (0 match)** | 10.357 ms | 10.801 ms (🔴 +4.3%) | 10.362 ms (⚪ 0.0%) | 10.301 ms (⚪ -0.5%) | **10.200 ms (🟢 -1.5%)** |
| **`text` L1 (natural text)** | 2.497 ms | 1.942 ms (🟢 -22.2%) | 1.690 ms (🟢 -32.3%) | 1.920 ms (🟢 -23.1%) | **1.950 ms (🟢 -21.9%)** |
| **`striped_rgb` L6 (long match)**| 0.184 ms | 0.151 ms (🟢 -17.5%) | 0.158 ms (🟢 -13.7%) | 0.157 ms (🟢 -14.4%) | **0.149 ms (🟢 -19.1%)** |
| **`short_match` L3 (8-16B)** | 5.284 ms | 5.199 ms (🟢 -1.6%) | 5.799 ms (🔴 +9.8%) | 5.294 ms (⚪ +0.2%) | **5.292 ms (⚪ +0.2%)** |
| **Regression Count ($> 1.0\%$)** | — | 4 points | 5 points | 1 point | **0 points (Zero Regression)** |

- **Alternatives Considered**: All 4 competing architectures were physically implemented and evaluated. `Unrolled2x` is the only architecture with **zero regressions across all 25 test points** while capturing top speedups.
- **Source**: `develop_bench_all_types.json`, `pr_bench_all_types.json`, `pure_swar64_bench.json`, `cascaded_bench.json`, `unrolled2x_bench.json`.
