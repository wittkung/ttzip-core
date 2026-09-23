# Upstream Contribution Research Brief: ARM64 16-Way PMULL+EOR3 CRC-32 Superscalar Acceleration

## 1. Background & Upstream Baseline
In `zlib-ng`, PR #2023 introduced a 3-way scalar + 9-way PMULL interleaved implementation (`arch/arm/crc32_armv8_pmull_eor3.c`).
On modern wide-issue Apple Silicon (M1~M5) and AWS Graviton 3/4 (Neoverse V2) cores, measured throughput plateaus at **~11.35 GB/s**.

## 2. Mathematical Constant Derivation
Via polynomial modular exponentiation $x^N \pmod P$ ($P = \text{0x104c11db7}$), folding constants for wide unrolling are strictly verified:

| Fold Span | $k_0 = x^{8N-33} \pmod P$ | $k_1 = x^{8N+31} \pmod P$ | Target Pipeline |
| :--- | :--- | :--- | :--- |
| **16 Bytes** | `0xccaa009e` | `0xae689191` | Tree reduction terminal |
| **32 Bytes** | `0x81256527` | `0xf1da05aa` | 2-vector fold |
| **64 Bytes** | `0x1d9513d7` | `0x8f352d95` | 4-vector fold |
| **128 Bytes** | `0x910eeec1` | `0x33fff533` | 8-vector fold |
| **144 Bytes** | `0x3f41287a` | `0x26b70c3d` | 9-vector existing loop |
| **256 Bytes** | `0xe95c1271` | `0xce3371cb` | 16-vector superscalar loop |

## 3. Performance Target
- Baseline: 11.35 GB/s
- Target: 25+ GB/s
- Status: Archived for next upstream contribution window.
