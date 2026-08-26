# Data Model: zlib-ng NEON LCP Acceleration & Dual-Platform Integration

**Feature**: `058-zlib-ng-neon-integration`
**Created**: 2026-08-17
**Status**: Draft

---

## 1. Entities & Structural Definitions

### 1.1 `DeflateTierMode` (Enum)
Defines the dispatch tier for Deflate compression and decompression operations.

| Value | Raw Value | Description |
| :--- | :--- | :--- |
| `tier1Block` | `1` | In-memory whole-buffer fast path via `libdeflate`. |
| `tier2Stream`| `2` | Stateful incremental sliding window stream via `zlib-ng`. |

---

### 1.2 `DeflateStreamConfig` (Structure)
Configuration parameters for initializing a Deflate streaming pipeline.

| Field Name | Type | Required | Constraints | Description |
| :--- | :--- | :--- | :--- | :--- |
| `tier_mode` | `integer` | Yes | Enum: `[1, 2]` | Tier 1 (libdeflate) or Tier 2 (zlib-ng). |
| `compression_level` | `integer` | Yes | Range: `[1, 9]` | Deflate compression level (1 = fastest, 9 = best). |
| `window_bits` | `integer` | Yes | `15` (zlib default), `31` (gzip header), `-15` (raw deflate) | Sliding window size and header format. |
| `mem_level` | `integer` | Yes | Range: `[1, 9]`, Default: `8` | Memory allocation tier for internal hash tables. |
| `strategy` | `integer` | Yes | `0` (Default), `1` (Filtered), `2` (Huffman Only), `3` (RLE), `4` (Fixed) | Deflate matching strategy. |

---

### 1.3 `DeflateStreamState` (Structure / C Handle)
Active execution context for a streaming Deflate session.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `magic` | `integer` (uint32) | Yes | Structural validation magic (`0x545A4453` = 'TZDS'). Reset to 0 on free. |
| `tier_mode` | `integer` | Yes | Active tier mode (`1` or `2`). |
| `total_in` | `integer` (uint64) | Yes | Total input bytes consumed across all chunks. |
| `total_out` | `integer` (uint64) | Yes | Total compressed/uncompressed bytes produced. |
| `adler32_checksum` | `integer` (uint32) | Yes | Running Adler-32 checksum (for zlib wrappers). |
| `crc32_checksum` | `integer` (uint32) | Yes | Running CRC-32 checksum (for gzip wrappers). |
| `is_finished` | `boolean` | Yes | Flag indicating stream reached `Z_STREAM_END`. |

---

### 1.4 `HybridMatchFinderInput` (Structure)
Input parameters passed to the hybrid SWAR/NEON longest-match comparison kernel.

| Field Name | Type | Required | Constraints | Description |
| :--- | :--- | :--- | :--- | :--- |
| `src0_address` | `integer` (uint64) | Yes | Non-null pointer address | Memory address of current search position. |
| `src1_address` | `integer` (uint64) | Yes | Non-null pointer address | Memory address of dictionary match candidate. |
| `max_len` | `integer` (uint32) | Yes | Range: `[0, 258]` for Deflate, `[0, 273]` for LZMA | Maximum allowed comparison length. |
| `nice_len` | `integer` (uint32) | Yes | Range: `[3, 258]` | Target match length threshold for early exit. |

---

### 1.5 `HybridMatchFinderResult` (Structure)
Output produced by the hybrid SWAR/NEON comparison kernel.

| Field Name | Type | Required | Constraints | Description |
| :--- | :--- | :--- | :--- | :--- |
| `match_length` | `integer` (uint32) | Yes | Range: `[0, 258]` | Total number of contiguous identical bytes found. |
| `dispatch_path` | `string` | Yes | Enum: `["swar_gpr", "neon_vector", "scalar_tail"]` | Execution path that resolved the comparison. |

---

### 1.6 `HardwareAccelerationCapabilities` (Structure)
Hardware CPU features detected at runtime for Deflate acceleration.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `has_arm_neon` | `boolean` | Yes | True if ARM64 NEON vector instructions are available. |
| `has_arm_crc32` | `boolean` | Yes | True if ARMv8 ACLE CRC32 instructions are available. |
| `has_x86_avx2` | `boolean` | Yes | True if x86_64 AVX2 256-bit SIMD instructions are available. |
| `has_x86_avx512`| `boolean` | Yes | True if x86_64 AVX-512 (F/BW/DQ/CD/VL) instructions are available. |
| `has_x86_vpclmul`| `boolean` | Yes | True if VPCLMULQDQ hardware CRC acceleration is available. |

---

## 2. Invariants & Validation Rules

1. **Bounds Invariant**: `HybridMatchFinderResult.match_length` must strictly satisfy: `0 <= match_length <= max_len`.
2. **Magic Lifecycle**: `DeflateStreamState.magic` must be set to `0x545A4453` upon successful `init` and cleared to `0x00000000` immediately prior to freeing resources in `free()`.
3. **No Dynamic Allocation in Coder**: Processing chunks via `DeflateStreamState` must perform zero calls to `malloc()` or `free()`.
