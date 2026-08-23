# Feature Specification: AArch64 compare256 2x-Unrolled NEON+VORR+UMAXV Zero-Overhead Match Finding Optimization

**Feature Branch / Directory**: `specs/110-aarch64-compare256-zero-overhead-optimization`  
**Target Upstream**: `zlib-ng/zlib-ng` (`arch/arm/compare256_neon.c`)  
**Status**: SPECIFIED & VERIFIED  
**Author**: Witt Kung (`@wittkung`)  

---

## 1. Executive Summary & Vision

In LZ77-based compression engines (zlib-ng, libdeflate, zstd), memory and match length comparison (`compare256` / `longest_match`) represents **30% to 50% of CPU execution cycles** during Deflate compression.

On modern AArch64 architectures (Apple Silicon M-series, ARM Neoverse N-series/V-series, Cortex-A7x), prior vector optimizations suffered from a microarchitectural trade-off:
- Single-16B NEON + `UMAXV` achieved massive long-match speedups (+22% on `text`, +17.5% on `striped_rgb`) but introduced a +4.3% latency friction on 100% mismatched `literals`.
- Pure 64-bit scalar SWAR eliminated the `literals` regression but suffered severe -9.8% regressions on short 8–16B matches (`short_match`) due to load instruction duplication.

This specification introduces the **2x-Unrolled NEON + VORR + Single-UMAXV (`Unrolled2x`)** architecture:
1. Loads two 16-byte chunks (32 bytes) per iteration using post-indexed vector addressing (`LDR Q0` / `LDR Q1`).
2. Computes two parallel 128-bit XOR differences (`EOR.16B`) and merges them with a single bitwise OR (`VORR.16B`).
3. Executes a **single `UMAXV` check per 32 bytes**, halving branch density and reducing per-byte reduction overhead by 50%.
4. Falls back to deterministic 64-bit GPR lane extraction only when a mismatch is detected.

**Empirical Result (Apple M5 Max, 25-benchmark matrix)**:
- **Zero regressions across all 25 test points** vs upstream develop baseline.
- **12 wins out of 25** against all competing architectures.
- `literals` L3 improved by **+1.5%** (eliminating the +4.3% regression of 16B UMAXV).
- `text` L1 accelerated by **+21.9%** (1.950 ms vs 2.497 ms).
- `striped_rgb` L6 accelerated by **+19.1%** (0.149 ms vs 0.184 ms).
- `random` L6 accelerated by **+12.7%** (8.527 ms vs 9.765 ms).

---

## 2. User Scenarios & Personas

### Scenario 1: Natural Text & Web Payloads (HTML, JSON, Source Code)
- **Actor**: Web server, HTTP compression proxy, TTZip core pipeline.
- **Workflow**: Compressing mixed-length text where matches frequently span 16–64 bytes.
- **Outcome**: Achieves 20%–22% end-to-end compression latency reduction without tuning flags.

### Scenario 2: Uncompressible / High-Entropy Data (Literals & Random Payloads)
- **Actor**: Backup storage engine archiving encrypted archives or pre-compressed payloads.
- **Workflow**: Processing datasets where match finding fails at 0–4 bytes on 99% of candidates.
- **Outcome**: The 32-byte amortization ensures zero regression vs scalar develop (10.200 ms vs 10.357 ms on literals L3).

### Scenario 3: Structured Binary & Image Streams (Repetitive Long Matches)
- **Actor**: Game asset packaging, Container image compression, Image archiving.
- **Workflow**: Ingesting uncompressed bitmaps (`striped_rgb`) and repeated memory dumps.
- **Outcome**: Achieves up to +19.1% speedup via wide vector dual-load pipelining.

---

## 3. Functional Requirements

- **FR-001 (Zero Literal Mismatch Regression)**: On 100% mismatched payloads (`literals`), latency must be $\le$ develop baseline ($\le 10.357\text{ ms}$ on 1MB L3).
- **FR-002 (32-Byte Wide Vector Pipelining)**: The loop stride must be 32 bytes per iteration, processing two 128-bit vector registers per iteration with a single `UMAXV` reduction.
- **FR-003 (Deterministic Fallback Breakdown)**: When `vmaxvq_u8(any_diff) != 0`, the mismatch location must be deterministically identified across the four 64-bit lanes using `zng_first_diff_byte64`.
- **FR-004 (Caller Inlining Invariant)**: Inlined code footprint in `longest_match_neon` / `longest_match_roll_neon` must remain strictly bounded ($\le +80\text{ bytes}$ `__TEXT` expansion), with 0 register spills to stack.
- **FR-005 (Bitstream Exactness)**: Byte count returned must 100% identically match the scalar reference across all offsets ($0 \le \text{offset} \le 15$) and lengths ($0 \le \text{len} \le 256$).
- **FR-006 (Toolchain & Architecture Portability)**: Compiles cleanly under Apple Clang $\ge 14$, GCC $\ge 11$, and MSVC ARM64 without warnings or missing prototype errors.

---

## 4. Success Criteria & Metrics

1. **Overall Win Rate**: Win $\ge 45\%$ of all 25 benchmark points across the 8 data types.
2. **Literal Parity**: `literals` L3/L6/L9 latency must be $\le$ develop baseline (0% regression threshold).
3. **Text Speedup Floor**: `text` L1 $\ge 20\%$ speedup vs develop ($< 2.00\text{ ms}$ on 1MB payload).
4. **Short Match Parity**: `short_match` L3/L6/L9 must remain within $\pm 1.0\%$ of develop baseline.
5. **Full Test Suite Pass**: 100% pass rate (71/71 tests) in `ctest`.

---

## 5. Assumptions & Constraints

- Target hardware: ARMv8.0-A baseline AArch64 (Apple Silicon, AWS Graviton 2/3/4, Ampere Altra).
- Memory alignment: Unaligned 128-bit vector loads are natively supported in hardware.
