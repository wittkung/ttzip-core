# Phase 0 Research: Libdeflate Deep Architecture Integration & Legacy Fallback Elimination

**Feature**: [062-libdeflate-architecture-integration](spec.md)
**Status**: Completed

---

### [R001] [SUBAGENT:research] Replace Legacy `zlib.h` Fallback in `CTTZipStreamCoder.c` with Chunked `libdeflate` Pipeline

- **Decision**:
  Refactor the internal state (`internal_state`) of `ttzip_deflate_stream_state_t` in `Sources/CTTZipBridge/CTTZipStreamCoder.c` from legacy `z_stream` (`zlib.h`) to a dedicated bounded chunk streaming context (`ttzip_deflate_chunk_engine_t`) backed by `libdeflate`.
  1. **Chunk Staging & Double Buffering**: Introduce a bounded 256KB/1MB staging buffer (`stage_buf`) in the internal state. Incoming uncompressed or compressed streams are accumulated and processed block-by-block using thread-local compressors/decompressors (`ttzip_get_tls_compressor(level)` / `ttzip_get_tls_decompressor()`).
  2. **RFC Format Alignment**:
     - Raw DEFLATE (`window_bits = -15`, RFC 1951): Execute `libdeflate_deflate_compress` / `libdeflate_deflate_decompress`.
     - Standard ZLIB (`window_bits = 15`, RFC 1950): Execute `libdeflate_zlib_compress` / `libdeflate_zlib_decompress`.
     - GZIP (`window_bits = 31`, RFC 1952): Execute `libdeflate_gzip_compress` / `libdeflate_gzip_decompress`.
  3. **Checksum Accuracy Fix**: Replace the legacy assignment bug (`CTTZipStreamCoder.c:148, 213`) where `state->crc32_checksum = (uint32_t)strm->adler` with genuine hardware-accelerated `libdeflate_crc32` for CRC-32 and `libdeflate_adler32` for Adler-32 updated on each processed chunk.
  4. **Memory Invariant**: Memory footprint per stream instance is strictly bounded to the 256KB–1MB staging buffer + TLS compressor (~2MB shared per thread), ensuring total resident memory remains $\le 64\text{MB}$ across all concurrent streams.

- **Rationale**:
  `libdeflate` outperforms legacy `zlib.h` by $2\times$ to $5\times$ in compression throughput and $2\times$ to $3\times$ in decompression throughput on modern architectures. Legacy `zlib.h` incurs stateful byte-by-byte sliding window overhead and scalar matching loops. Replacing `z_stream` inside `CTTZipStreamCoder.c` eliminates the `zlib.h` dependency entirely for streaming pipelines while preserving 100% C ABI and Swift ABI compatibility (`DeflateStreamCompressor` and `DeflateStreamDecompressor` in `Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift`).

- **Alternatives Considered**:
  - *Alternative 1 (`zlib-ng` streaming dynamic dispatch)*: Rejected because linking an additional full copy of `zlib-ng` introduces potential symbol namespace collisions with macOS system `libz.dylib`, increases binary size, and achieves lower throughput than `libdeflate`'s SIMD-optimized block engine.
  - *Alternative 2 (Full-file unbounded buffering)*: Rejected because buffering entire uncompressed files in RAM violates the stream-first architecture and memory invariants ($\le 64\text{MB}$), risking out-of-memory crashes on multi-gigabyte files.

- **Source**:
  - `Sources/CTTZipBridge/CTTZipStreamCoder.c`: lines 96–228 (`#include <zlib.h>`, `ttzip_deflate_stream_init`, `ttzip_deflate_stream_process`, `ttzip_deflate_stream_free`, `ttzip_inflate_stream_init`, `ttzip_inflate_stream_process`, `ttzip_inflate_stream_free`).
  - `Sources/CTTZipBridge/include/CTTZipStreamCoder.h`: lines 44–54 (`ttzip_deflate_stream_state_t` struct definition).
  - `Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift`: lines 129–150, 209, 290, 320, 327–340.
  - `Sources/CTTZipBridge/CTTZipBridge_ZipChunkedStream.c`: lines 1–125 (1MB bounded chunked ring buffer model utilizing `libdeflate_deflate_compress` and `libdeflate_crc32`).

---

### [R002] [SUBAGENT:research] 7Z Native Decoder DEFLATE Method ID (0x040108) Direct Routing to `ttzip_libdeflate_decompress`

- **Decision**:
  Add an explicit dispatch branch for Method ID `0x040108` (and `0x40108`) in `Sources/CTTZipBridge/ttzip_7z_block_decoder.c` (`ttzip_7z_decode_payload_parallel`). Route the raw RFC 1951 bitstream directly to `ttzip_libdeflate_decompress(payload_start, payload_len, unpack_buf, total_unpack_bytes)`.
  - On return, update `total_unpack_bytes` with the actual unpacked byte count.
  - If decompression fails (`def_dec == 0`), clean up resources and return `TTZIP_ERR_CORRUPT_HEADER`.

- **Rationale**:
  In the 7z specification, Method ID `0x040108` (3-byte sequence `04 01 08`) represents standard raw DEFLATE (RFC 1951). In the current implementation of `ttzip_7z_block_decoder.c` (lines 111–158), only Store (`0x00`/`0x06F10701`), Zstandard (`0x04F71101`), and LZMA2 (`0x21`) are explicitly handled. Any 7z archive compressed using DEFLATE currently falls through into `ttzip_lzma2_decode_block_native`, causing decode failures. Since 7z stores raw DEFLATE streams without headers, `ttzip_libdeflate_decompress` (which wraps `libdeflate_deflate_decompress` via thread-local decompressor instances in `CTTZipStreamCoder.c:39–47`) provides in-process, zero-copy, NEON-accelerated decompression directly into the output buffer.

- **Alternatives Considered**:
  - *Alternative 1 (Routing 7z DEFLATE through libarchive `archive_read_support_format_7zip`)*: Rejected because libarchive involves stream abstraction overhead, per-entry memory allocations, and loses the benefit of TTZip's fast zero-copy memory mapping and directory caching pipeline.
  - *Alternative 2 (Spawning external `7za`/`7z` CLI subprocess)*: Rejected because TTZip maintains a strict 100% In-Process C static binding architecture (zero external CLI process execution).

- **Source**:
  - `Sources/CTTZipBridge/ttzip_7z_block_decoder.c`: lines 111–158 (`ttzip_7z_decode_payload_parallel` method ID dispatch logic).
  - `Sources/CTTZipBridge/ttzip_7z_header_parser.c`: lines 130–140 (`out_info->primary_method_id = mid` parsing 7z method IDs).
  - `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c`: lines 118–143 (dispatching `ttzip_7z_decode_payload_parallel`).
  - `Sources/CTTZipBridge/CTTZipStreamCoder.c`: lines 39–47 (`ttzip_libdeflate_decompress` implementation).

---

### [R003] [SUBAGENT:research] Verification of Apple Silicon NEON / PMULL Hardware Acceleration Paths for CRC32 and CRC64

- **Decision**:
  1. **CRC-32 Standardization**: Verify and confirm that all active CRC-32 calculation paths across the C bridge layer (`CTTZipCRC32Neon.c`, `CTTZipSIMD.c`, `CTTZipUtils.c`, `CTTZipBridge_7zStore.c`, `CTTZipBridge_ZipChunkedStream.c`, `CTTZipBridge_ZipWrite.c`) route directly to `libdeflate_crc32()`. On ARM64 / Apple Silicon, `libdeflate_crc32` compiles to ARMv8-A CRC32 instructions (`__crc32b/w/d` / PMULL vector folding), achieving $> 30\text{ GB/s}$ throughput.
  2. **CRC-64 PMULL Folding**: Verify that `Sources/CTTZipBridge/ttzip_crc64.c` uses 4-way 64-byte vector folding via ARM64 `vmull_p64` instructions with Barrett modular reduction (`ttzip_crc64_pmull`), with the scalar table loop strictly isolated behind `#else` fallback for non-ARM targets.
  3. **Identified Legacy Hotspots to Modernize**:
     - `CTTZipStreamCoder.c` (lines 148, 213): Update from erroneous `strm->adler` assignment to incremental `libdeflate_crc32()` / `libdeflate_adler32()`.
     - `ZipCryptoEngine.swift` (lines 36–43): Legacy 4-bit nibble table lookup in `ZipCryptoKeys.crc32` is located in a frozen module (`.agents/rules/zip-engine-freeze.md`), but all new and active data planes use `ttzip_compute_buffer_crc32_neon` / `libdeflate_crc32`.

- **Rationale**:
  Apple Silicon processors provide dedicated hardware instructions for polynomial math: ARMv8 CRC32 extension for IEEE 802.3 CRC-32 (`0xEDB88320`) and PMULL (`vmull_p64`) for ECMA-182 CRC-64 (`0xC96C5795D7870F42`). Codebase scanning confirms that all archive production paths (ZIP writing, 7Z reading/writing, streaming chunk compression, APFS integrity verification) are fully mapped to `libdeflate_crc32` and `ttzip_crc64_pmull`, with zero scalar table loops on Apple Silicon hot paths.

- **Alternatives Considered**:
  - *Alternative 1 (Writing custom assembly `.s` files for CRC32)*: Rejected because `libdeflate_crc32` is already benchmarked, highly tuned, and maintained upstream in `Vendor/libdeflate.a`, avoiding duplicate maintenance overhead.
  - *Alternative 2 (Using system zlib `crc32()`)*: Rejected because system `libz` uses a legacy scalar table loop ($\sim 1.2\text{ GB/s}$), which is $\sim 25\times$ slower than `libdeflate_crc32` ($> 30\text{ GB/s}$).

- **Source**:
  - `Sources/CTTZipBridge/CTTZipCRC32Neon.c`: lines 11–14 (`ttzip_core_crc32_neon_single` $\rightarrow$ `libdeflate_crc32`).
  - `Sources/CTTZipBridge/CTTZipSIMD.c`: lines 4–7 (`ttzip_simd_crc32` $\rightarrow$ `libdeflate_crc32`).
  - `Sources/CTTZipBridge/CTTZipUtils.c`: lines 75–114 (`ttzip_compute_buffer_crc32_neon`).
  - `Sources/CTTZipBridge/ttzip_crc64.c`: lines 13–125 (`ttzip_crc64_pmull`).
  - `Sources/TTZipCore/HashCalculator.swift`: lines 35–55.
