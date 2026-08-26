# Phase 1 Data Model: Adaptive Block Splitting & Fast Container Framing

**Feature Branch / Spec Directory**: `specs/103-adaptive-block-splitting-and-container-engine`  
**Created**: 2026-08-19  
**Status**: Completed  

---

## 1. Entities & Structural Models

### Entity 1: `AdaptiveBlockSplitStats`
Tracks histogram drift across 10 aggregate observation types (8 literal categories + 2 match categories).

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `num_observations` | `uint32` | $\ge 0$ | Total committed observation tokens in current block |
| `num_new_observations` | `uint32` | $0 \le n \le 512$ | Unmerged observation tokens since last check |
| `observations` | `Array<uint32>[10]` | Non-negative | Committed frequency histogram per token class |
| `new_observations` | `Array<uint32>[10]` | Non-negative | Unmerged frequency histogram per token class |

### Entity 2: `ContainerFramingResult`
Represents the result of a fast GZIP or ZLIB container serialization or deserialization.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `format` | `string` | `"gzip"` or `"zlib"` | Container format |
| `uncompressed_bytes` | `int64` | $\ge 0$ | Original payload byte count |
| `compressed_bytes` | `int64` | $\ge 0$ | Container payload including header and footer |
| `header_overhead_bytes` | `int32` | 18 for GZIP, 6 for ZLIB | Fixed container framing overhead |
| `checksum` | `uint32` | 32-bit integer | CRC-32 (GZIP) or Adler-32 (ZLIB) |
| `is_verified` | `boolean` | `true` | Assertion that checksum and lengths match |

---

## 2. Invariants & Bounds

1. **Minimum Block Invariant**: $L_{\text{block}} \ge 5000$ bytes for any non-final block.
2. **Soft Block Invariant**: $L_{\text{block}} \le 305000$ bytes (absorbing sub-5000B tail).
3. **GZIP Framing Invariant**: Header is exactly 10 bytes starting with `0x1F 0x8B 0x08`; trailer is 8 bytes with little-endian CRC-32 and ISIZE.
4. **ZLIB Framing Invariant**: Header is 2 bytes with `(CMF * 256 + FLG) % 31 == 0`; trailer is 4 bytes with big-endian Adler-32.
