# Phase 0 Research: Streaming Fast-Path Decompressor & Dual-Symbol LUT

**Feature**: `124-streaming-fastpath-decompressor-dual-symbol-lut`
**Created**: 2026-08-19

---

## Research Items

### R001 [SUBAGENT:research] 10-Bit Dual-Symbol Decode Table Design
- **Decision**: Construct a primary 10-bit lookup table (`1024` entries, 4KB footprint). Each 32-bit entry encodes `flags (8-bit) | len_bits (8-bit) | symbol0 (8-bit) | symbol1 (8-bit)`.
- **Rationale**:
  - When `symbol0` is a short literal ($\le 5\text{ bits}$), the remaining bits in the 10-bit window can match a second short literal `symbol1` ($\le 5\text{ bits}$).
  - A single 10-bit lookup and 16-bit store writes 2 uncompressed bytes and advances the bitstream by `len_bits0 + len_bits1` without intervening branch instructions.
- **Alternatives Considered**:
  - *12-bit / 4096-entry table*: 16KB footprint causes L1 D-Cache contention with input/output buffers.
  - *Scalar 8-bit table*: Single-symbol only, maxing out decompression at ~4.5 GB/s.
- **Source**:
  - `Vendor/libdeflate-upstream/lib/decompress_template.h:80-160`
  - `Vendor/zlib-ng-upstream/arch/arm/slide_hash_neon.c`

---

### R002 [SUBAGENT:research] NEON 128-Bit & SWAR Fast Match Replication
- **Decision**: Use unaligned NEON 16-byte load/stores (`vld1q_u8` / `vst1q_u8`) when `offset >= 16`, and 64-bit SWAR pattern duplication when `offset < 8`.
- **Rationale**:
  - When `offset >= 16`, copying 16 bytes per cycle avoids pipeline stalls.
  - For $offset = 1$ (RLE), byte is broadcast to 8 bytes (`v = byte * 0x0101010101010101ULL`) and stored via 64-bit stores.
- **Alternatives Considered**:
  - *Byte-by-byte loop `while(len--) *dst++ = *src++`*: Stalls superscalar execution with sequential data dependencies.
- **Source**:
  - `Sources/CTTZipBridge/CTTZipNEONMatchFinder.c:50-90`
