# Upstream Open-Source Contribution & Optimization Portfolio

This document codifies TTZip's institutional contributions, microarchitectural research briefs, and performance patches contributed back to Tier-1 and Tier-2 foundational compression and archiving engines.

---

## 1. Contribution Matrix Overview

| Upstream Project | PR / Feature Identifier | Microarchitectural Focus & Breakthrough | Target ISA / Hardware | Artifact / Patch Reference |
| :--- | :--- | :--- | :--- | :--- |
| **libarchive** | `pr-3388-7z-aes` | 7z container AES-CBC hardware-accelerated block cipher pipeline | Apple Silicon / ARM64 Crypto | `patches/upstream-contributions/libarchive-7z-aes.patch` |
| **libarchive** | `pr-3391-crc32` | Hardware PMULL & polynomial vectorization for POSIX container headers | ARMv8.1-A ACLE CRC32 | `patches/upstream-contributions/libarchive-crc32.patch` |
| **libarchive** | `pr-3393-preallocate` | APFS/ext4 fcntl `F_PREALLOCATE` chunked sequential allocation | macOS APFS / Linux | Upstream Merged |
| **XZ Utils (liblzma)** | `pr1-arm64-bcj` | AArch64 Branch-Call-Jump (BCJ) filter vectorization | Apple Silicon / ARM64 | `patches/upstream-contributions/xz-arm64-bcj.patch` |
| **XZ Utils (liblzma)** | `pr2-arm64-crc64` | Galois Field polynomial multiplication (`vmull_p64`) accelerating CRC64 to 48 GB/s | ARM64 NEON + PMULL | `patches/upstream-contributions/xz-arm64-crc64.patch` |
| **XZ Utils (liblzma)** | `pr3-arm64-neon-memcmplen` | 128-bit vector match length probing (`vld1q_u8` + `veorq_u8` + `vgetq_lane_u64` + `ctzll`) | ARM64 NEON | `patches/0001-liblzma-arm-neon-memcmplen.patch` |
| **libdeflate** | `pr-gzip-parallel` | SIMD match-finder parallel chunk scheduler | Apple Silicon / x86_64 AVX2 | `third_party/patches/libdeflate-gzip.patch` |
| **libdeflate** | `pr-chunked-stream-example` | High-throughput streaming decompressor reference implementation | Cross-Platform C-API | `docs/examples/chunked_stream.c` |
| **LZ4** | `lz4/arm64-neon` | NEON block scanning and 8-byte unaligned branchless copy | ARM64 NEON | `patches/upstream-contributions/lz4-arm64-neon.patch` |
| **LZ4** | `lz4/partial-guard` | High-speed memory boundary safety guard for short token blocks | POSIX / Windows | Upstream Merged |
| **snappy** | `fix-darwin-universal-binary` | CMake multi-architecture fat-binary symbol export fix | macOS ARM64 + x86_64 | Upstream Merged |
| **snappy** | `test-malformed-stream` | Fuzz-driven corrupted frame boundary test harness | Fuzz Testing CI | Upstream Merged |
| **zlib-ng** | `pr-2415 / pr-2416` | Multi-workload 8-stage regression audit gate and register domain crossing analysis | ARM64 AArch64 | `specs/133-upstream-contribution-lessons-and-governance` |
| **zlib-ng** | `feat-arm64-pmull-16way` | 16-Way PMULL+EOR3 CRC-32 superscalar polynomial folding (25+ GB/s throughput) | Apple Silicon (M-Series) | `patches/upstream-contributions/zlib-ng-pmull-16way.md` |
| **zlib-ng** | `feat-arm64-swar-compare256` | Byte-parallel SWAR 256-bit match length comparison | ARM64 64-bit GPRs | `patches/upstream-contributions/zlib-ng-swar-256.patch` |
| **zlib-ng** | `feat-darwin-sysctl-crc32` | Zero-cost macOS `sysctlbyname("hw.optional.armv8_crc32")` CPU feature probe | macOS Darwin ARM64 | Upstream Merged |
| **Zstandard (zstd)** | `pr3-arm64-neon-rle-detect` | 16-byte vector Run-Length Encoding (RLE) run detection | ARM64 NEON | `patches/upstream-contributions/zstd-neon-rle.patch` |

---

## 2. Microarchitectural Breakthrough Highlight: 16-Way PMULL Folding

For modern 8~10 issue-width architectures (Apple Silicon M1~M5, AWS Graviton 3/4), scalar and 9-way PMULL loops leave vector execution pipelines underutilized.

Using polynomial modular exponentiation $x^N \pmod P$ ($P = \text{0x104c11db7}$), we derived the exact folding constant matrix:

$$k_0 = x^{8N-33} \pmod P, \quad k_1 = x^{8N+31} \pmod P$$

| Fold Span | $k_0$ | $k_1$ | Optimal Hardware Pipeline Target |
| :--- | :--- | :--- | :--- |
| **16 Bytes** | `0xccaa009e` | `0xae689191` | Tree reduction terminal fold |
| **32 Bytes** | `0x81256527` | `0xf1da05aa` | 2-vector reduction |
| **64 Bytes** | `0x1d9513d7` | `0x8f352d95` | 4-vector reduction |
| **128 Bytes** | `0x910eeec1` | `0x33fff533` | 8-vector reduction |
| **256 Bytes** | `0xe95c1271` | `0xce3371cb` | 16-vector superscalar main loop (25+ GB/s throughput) |

---

## 3. Maintenance & Contribution Rules

1. **Monorepo Hygiene**: Upstream PR worktrees and full third-party codebases are decoupled from the Monorepo Git tree.
2. **Patch-First Invariant**: All external proposals must be formulated as self-contained `.patch` files accompanied by assembly disassembly proofs and multi-workload benchmark reports.
