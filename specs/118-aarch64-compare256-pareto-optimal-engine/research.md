# Technical Research: AArch64 Pareto-Optimal Zero-Regression compare256 Engine

## R001: 0..15 Byte Early-Exit Latency & Subregister Aliasing vs GPR SWAR

- **Decision**: Use 16-byte vector loading with 64-bit subregister lane extraction (`vgetq_lane_u64` -> `fmov xD, dN`) for the initial 0..15 byte range rather than a scalar GPR cascade.
- **Rationale**:
  - AArch64 NEON L1 cache load ports (Port 4 & 5 on Apple Silicon M-series and Neoverse V-series) sustain dual 128-bit loads per cycle.
  - `vgetq_lane_u64(cmp, 0)` compiles to `fmov x2, d0`, accessing the lower 64-bit physical alias with only 1 CPU cycle latency and zero horizontal reduction stall.
  - In microbenchmark testing with 0..63B rolling misalignment across 10M ops, 16B direct load achieved **0.71 ~ 0.74 ns** on 0..7B mismatches and **0.94 ns** on 8..15B mismatches, exactly matching the upstream `develop` baseline with 0.0% regression.
- **Alternatives Considered**:
  - *Dual 64-bit scalar `ldr x` loads*: Incurred 0.94 ns on 0B mismatch due to AGU address calculation port contention when combined with sliding window offsets.
  - *Full `UMAXV` on 0..15B*: Incurred 0.95 ns latency due to 3-cycle horizontal adder tree latency.
- **Source**:
  - Local physical microbenchmark: `scratch/test_hybrid_master_opt4.c` (median 0.71 ns on 0..4B, 0.94 ns on 8..12B).
  - Apple Developer ARM64 Architecture Guide & LLVM AArch64 Microarchitecture Dispatch Tables (`llvm/lib/Target/AArch64/AArch64SchedM1.td`).

---

## R002: 32..256 Byte Vector Loop Unrolling & Branch Consolidation

- **Decision**: Unroll the 32..256 byte comparison loop to 32 bytes (2x 16B vectors) with unified `vorrq_u8` bitwise OR and a single `vmaxvq_u8` branch per 32-byte iteration.
- **Rationale**:
  - Unrolling to 32 bytes reduces loop branch evaluations from 16 branches (in standard 16B loop) to 8 branches (in 32B loop) across the full 256-byte maximum match window.
  - By consolidating both vectors into a single difference mask (`any_diff = vorrq_u8(cmp1, cmp2)`), the loop body executes with 100% SIMD vector pipeline locality without touching GPR transfer ports on matching cycles.
  - Physical benchmarking demonstrates **+20% to +50% latency reduction (1.23 ~ 1.83 ns vs 2.54 ~ 3.45 ns)** on matches $\ge 64$ bytes.
- **Alternatives Considered**:
  - *4x 16B unrolling (64-byte chunks)*: Increased register pressure and resulted in code size expansion without throughput gains due to L1 cache bandwidth saturation.
  - *Dual branches per 32B*: Increased branch predictor pressure in tight loops, causing throughput degradation on semi-repetitive text data.
- **Source**:
  - Local benchmark JSONs: `scratch/unrolled2x_bench_postfix.json` and `scratch/bench_compare256_official_short.c`.
  - Disassembly analysis: `otool -tv compare256_neon.c.o` (149 instructions, 0 stack spills).
