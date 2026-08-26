# Research & Technical Decisions: C Test Harness Expansion & Advanced Microkernels

**Feature**: `156-156-test-harness-expansion`  
**Date**: 2026-08-20  
**Status**: Completed  

---

## 1. Blosc2, BitGroom & SuperChunk Architecture (`test_blosc_engine.c`)

### Decision
Implement `tests/c/test_blosc_engine.c` verifying:
- `ttzip_blosclz_compress` and `ttzip_blosclz_decompress` roundtrip across synthetic patterns (`clevel=5`, `hash_log=13`).
- Sub-16-byte literal run bypass opcode `0x0D` with zero LZ match overhead.
- NEON-accelerated BitGroom mantissa quantization (`ttzip_filter_bitgroom_float32_neon`, `NSD=3`).
- SuperChunk special MSB chunk tagging (`1ULL << 63`) for all-zero/all-constant blocks.

### Rationale
- BloscLZ provides line-rate compression for uncompressed memory chunks; verifying byte-exact roundtrip in pure C ensures SIMD optimizations never corrupt payloads.
- Stack-allocated 64KB lookup tables guarantee zero heap allocations during test execution.

### Alternatives Considered
- **Testing via Swift Blosc2 Wrapper**: Rejected due to high ARC/bridging overhead and inability to verify SIMD vector registers directly.
- **Pure C11 Harness (Selected)**: Instant execution (< 1ms) with ASan memory leak verification.

### Source
- `Sources/CTTZipBridge/include/ttzip_blosclz.h`
- `Sources/CTTZipBridge/include/CTTZipBitGroom.h`
- `Sources/CTTZipBridge/include/CTTZipSuperChunk.h`

---

## 2. Canonical Huffman & Adaptive Block Splitting (`test_huffman_inplace.c`)

### Decision
Implement `tests/c/test_huffman_inplace.c` verifying:
- `ttzip_make_canonical_huffman_code_inplace` enforcing Kraft-McMillan limits ($\sum 2^{-L_i} \le 1.0$) and $L_{\max} \le 15$ bits on Fibonacci frequency distributions.
- ARM64 `RBIT` hardware bit-reversal via `ttzip_canonical_bit_reverse`.
- Adaptive Deflate block splitting with L1 cache resident statistics (`ttzip_should_end_block`).
- Dynamic vs Static vs Uncompressed Deflate block cost arbitration (`ttzip_eval_best_block_type`).

### Rationale
- In-place Huffman construction uses output memory array directly as working space, guaranteeing $O(1)$ auxiliary heap memory.
- Verifying bitstream canonicality in C ensures full RFC 1951 compliance.

### Alternatives Considered
- **Swift XCTest InPlaceHuffmanTests**: Retained as wrapper test, but core algorithmic invariant moved to C.
- **Pure C11 Test Suite (Selected)**: Direct bitwise inspection and mathematical proof of code lengths.

### Source
- `Sources/CTTZipBridge/include/ttzip_huffman_inplace.h`
- `Sources/CTTZipBridge/include/ttzip_adaptive_block_split.h`
- `Sources/CTTZipBridge/include/ttzip_inplace.h`

---

## 3. Snappy Block & Framed Streams (`test_snappy_engine.c`)

### Decision
Implement `tests/c/test_snappy_engine.c` verifying:
- Raw Snappy block compression and decompression roundtrip (`ttzip_snappy_compress`, `ttzip_snappy_decompress`).
- Framed format (`.sz`) stream header `\xFF\x06\x00\x00sNaPpY` validation.
- Castagnoli hardware CRC32c calculation and masking/unmasking (`ttzip_snappy_mask_crc32c`).
- Error resilience on truncated headers, corrupt varints, and invalid chunk identifiers.

### Rationale
- Snappy delivers > 10,000 MB/s decompression throughput; native C testing ensures maximum throughput without language runtime interference.

### Source
- `Sources/CTTZipBridge/include/CTTZipBridge_Snappy.h`
- `Sources/CTTZipBridge/snappy/snappy-c.h`

---

## 4. Apple DMG Demuxing & LZFSE (`test_dmg_lzfse.c`)

### Decision
Implement `tests/c/test_dmg_lzfse.c` verifying:
- Apple UDIF `koly` trailer parsing at 512-byte EOF (`ttzip_dmg_probe`, `ttzip_dmg_read_koly`).
- LZFSE chunk decompression (`ttzip_lzfse_decompress_block`) with thread-local scratch memory.
- Lossless roundtrip on compressible text payloads.

### Rationale
- DMG archives contain Apple system disk images; testing demuxing in C ensures compatibility with macOS DMG images without spawning external `hdiutil` processes.

### Source
- `Sources/CTTZipBridge/include/ttzip_dmg_demux.h`
- `Sources/CTTZipBridge/include/CTTZipBridge_LZFSE.h`
- `Sources/CTTZipBridge/lzfse/lzfse.h`

---

## 5. Radix Tree Virtual Archive Filesystem (`test_archive_tree.c`)

### Decision
Implement `tests/c/test_archive_tree.c` verifying:
- Fast-path Radix trie path splitting, folder hierarchy aggregation, and statistics accumulation (`ttzip_tree_insert`).
- Case-insensitive substring and prefix search (`ttzip_tree_search`).
- In-memory entry extraction with zero disk I/O (`ttzip_archive_extract_entry_mem`).

### Rationale
- Radix trees enable sub-millisecond search across multi-million file archives in TTZip; testing in pure C guarantees zero memory leaks during tree destruction.

### Source
- `Sources/CTTZipBridge/include/ttzip_archive_tree.h`
- `Sources/CTTZipBridge/include/ttzip_archive.h`
