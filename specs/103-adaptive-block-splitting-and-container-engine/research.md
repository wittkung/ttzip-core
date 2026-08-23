# Phase 0 Technical Research: Adaptive Block Splitting & Container Fast-Path

**Feature Branch / Spec Directory**: `specs/103-adaptive-block-splitting-and-container-engine`  
**Created**: 2026-08-19  
**Status**: Completed  

---

## Research Item R001 [SUBAGENT:research]: libdeflate 300KB/5KB Block Splitting Heuristic & 3-Way Block Type Optimization

- **Decision**: Adopt the 10-dimensional aggregate observation sampling (8 literal classes + 2 match length classes) with cross-multiplication L1 drift testing, 300KB soft / 5KB hard dual-threshold boundaries, and 3-way exact bit-cost arbitration (Store vs Static vs Dynamic).
- **Rationale**:
  1. **Tree Header Amortization**: `MIN_BLOCK_LENGTH = 5000` guarantees dynamic Huffman tree headers (150-300 bytes) never exceed 3-6% of block payload.
  2. **Tail Fragmentation Immunity**: `choose_max_block_end` absorbs sub-5KB trailing remnants into the previous block up to 305KB, preventing negative compression expansion.
  3. **Zero-Division L1 Drift**: The 10-class observation histogram sampled every 512 tokens detects entropy phase shifts using integer cross-multiplication with zero division or floating-point ALU operations.
  4. **Precise Bit-Cost Arbitration**: Dynamic cost, Static cost, and Uncompressed Store cost are accurately calculated before bitstream emission, breaking ties in favor of lower decompression complexity (`Store > Static > Dynamic`).
- **Alternatives Considered**:
  - *Zopfli Exhaustive Recursive Splitting*: Rejected due to $O(N^2)$ algorithmic complexity and 100x slowdown.
  - *Fixed 64KB/128KB Slicing*: Rejected because it cannot adapt to content transitions (e.g. JSON text preceding Base64 image).
- **Source**: `Vendor/libdeflate-upstream/lib/deflate_compress.c` lines 50-108, 438-449, 1706-1875, 2053-2218.

---

## Research Item R002 [SUBAGENT:research]: Zero-Overhead GZIP & ZLIB Container Serialization with Single-Pass SIMD Checksum Fusion

- **Decision**: Implement pre-allocated in-place container framing (`ttzip_gzip_compress_fast` & `ttzip_zlib_compress_fast`) directly reserving 10 bytes (GZIP) or 2 bytes (ZLIB) in output buffers, streaming raw Deflate payloads, and emitting hardware-vectorized CRC-32/ISIZE (little-endian) or Adler-32 (big-endian) trailers with zero intermediate memory copies.
- **Rationale**:
  1. **Zero Intermediate Memory Copies**: Output buffers are pre-sized with `18 + deflate_bound` (GZIP) or `6 + deflate_bound` (ZLIB). Deflate operates directly at `out + header_len`, and trailers are written via single `put_unaligned_le32` / `put_unaligned_be32` instructions.
  2. **RFC 1950 & 1952 Strict Endianness**: ZLIB headers enforce `(CMF * 256 + FLG) % 31 == 0` and big-endian Adler-32; GZIP headers enforce little-endian timestamps, CRC32, and ISIZE.
  3. **Decoupled High-Throughput Checksums**: Separate hardware PMULL CRC-32 and NEON Adler-32 passes operate at $> 20$ GB/s, avoiding register spills and pipeline stalls inside the LZ77 matchfinding hot loops.
- **Alternatives Considered**:
  - *In-Loop Interleaved Checksum Calculation*: Rejected because computing CRC/Adler inside LZ77 hot loops causes register spills and disables 64-byte NEON vectorization, dropping compression throughput by 35-50%.
  - *Chunked State Machine Framing*: Rejected due to redundant pointer indirection and struct state overhead on contiguous buffers.
- **Source**: `Vendor/libdeflate-upstream/lib/gzip_compress.c` lines 31-90, `lib/gzip_decompress.c` lines 32-133, `lib/zlib_compress.c` lines 31-82, `lib/zlib_decompress.c` lines 32-93.
