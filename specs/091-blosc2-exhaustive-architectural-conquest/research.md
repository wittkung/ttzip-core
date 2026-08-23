# Phase 0 Grounded Research: Blosc2 Exhaustive Architectural Conquest

**Feature**: `specs/091-blosc2-exhaustive-architectural-conquest`  
**Date**: 2026-08-18  

---

## Research Item 1: Dynamic Filter & Codec Plugin Registry (R001)

### Decision
Adopt a **Two-Tier Hybrid Architecture**:
1. **Tier 1 (Built-in IDs `0..15`)**: Hard-coded inline `switch` fast-path for standard SIMD filters (`Shuffle`, `BitShuffle`, `Delta`, `TruncateFloat32`, `TruncateFloat64`), ensuring direct inlined calls and zero indirect branch prediction penalty on hot paths.
2. **Tier 2 (User Plugin IDs `160..255`)**: Fixed 96-entry static array jump table indexed by `id - 160`. Read access uses acquire-release atomics (`atomic_load_explicit(..., memory_order_acquire)`), achieving **lock-free, zero-allocation read on parallel worker threads**. Registration is serialized via `pthread_mutex`.

### Rationale
- **Zero Overhead on Hot Paths**: Standard archives processed through built-in filters never touch a jump table or synchronization primitive.
- **Lock-Free Concurrency**: In multi-threaded chunk processing (`DispatchQueue.concurrentPerform` / multi-core decompression), multiple workers invoke plugins concurrently without mutex lock contention.
- **Zero Dynamic Allocation**: The registry uses static memory in `.bss` (< 1.5 KB total for 96 filter + 96 codec slots), strictly satisfying TTZip's Zero-Cost Abstraction and Performance Invariants.
- **C-Blosc2 Protocol Interoperability**: Conforms to Blosc2's canonical standard ID ranges (`0..159` built-in/global, `160..255` user plugins).

### Alternatives Considered
- **Dynamic Hash Table / Linked List with Mutex per Chunk Lookup**: Rejected due to dynamic heap allocations (`malloc`/`free`) on registration and mutex lock contention during hot parallel compression loops.
- **Dynamic Shared Library (`dlopen` / `dlsym`) Discovery**: Rejected due to non-deterministic I/O latency and Mac App Store (MAS) sandboxing restrictions (`-DMAS_BUILD`).

### Source
- `blosc2/filters-registry.h`, `blosc2/codecs-registry.h`, `blosc2.h` (https://github.com/Blosc/c-blosc2)
- TTZip: `Sources/CTTZipBridge/include/CTTZipFilterPipeline.h`, `Sources/CTTZipBridge/CTTZipFilterPipeline.c`

---

## Research Item 2: Block-Level Lazy Chunk Decompression & Range Slicing (R002)

### Decision
Implement a **Two-Tier Hierarchical Lazy Range-Slicing Engine** in TTZip (`ttzip_schunk_get_slice_buffer` & `ttzip_chunk_decompress_slice`) adapted from C-Blosc2's `blosc2_schunk_get_slice_buffer` / `blosc2_getitem_bytes_ctx`, tailored for Apple Silicon 128KB L1D micro-blocks with NEON zero-copy optimizations and MSB special-value bypass.

### Rationale
- **Targeted Sub-block Extraction**: For range $[S, S + L)$, only micro-blocks $b \in [\lfloor S / B \rfloor, \lfloor (S + L - 1) / B \rfloor]$ are decompressed. All leading and trailing blocks are bypassed (0 I/O reads, 0 decompression cycles).
- **True Zero-Copy for Interior Blocks**: Blocks fully contained in the slice are decompressed directly into the caller's destination buffer without intermediate copies.
- **Microsecond Preview Latency**: When reading a 4KB header from a 16MB chunk, $99.9\%$ of the chunk decompression is bypassed, reducing latency from milliseconds to microseconds.

### Alternatives Considered
- **Full-Chunk Decompress & Sub-slice (Naive Approach)**: Rejected because decompressing 4MB--32MB chunks for small reads wastes memory bandwidth and thrashes CPU caches.
- **Item-Based Indexing with Variable Typesize**: Rejected because large typesizes (> 255) cause truncation bugs in Blosc1 conventions; byte-oriented slicing eliminates ambiguity.

### Source
- `https://github.com/Blosc/c-blosc2/blob/main/README_CHUNK_FORMAT.rst`
- `https://github.com/Blosc/c-blosc2/blob/main/blosc/schunk.c` (`blosc2_schunk_get_slice_buffer`)
- TTZip: `Sources/CTTZipBridge/CTTZipSuperChunk.c`, `Sources/CTTZipBridge/include/CTTZipSuperChunk.h`

---

## Research Item 3: Floating-Point Precision Quantization & Bit-Grooming Filters (R003)

### Decision
Adopt **Bit-Preserving Mantissa Quantization (BitRound / BitGrooming)** as a pre-compression filter in floating-point pipelines, paired with **ARM NEON BitShuffle** and **Deflate / Zstd entropy coding**. Precision is parameterized by Number of Significant Digits (NSD) or Number of Significant Bits (NSB), directly masking IEEE-754 mantissa bitplanes.

### Rationale
- **Synergy with BitShuffle**: Discarded mantissa bits (e.g. 12--16 bits) are converted into solid $0\text{x}00$ or $0\text{xFF}$ bit-planes by BitShuffle, allowing LZ77/FSE to achieve massive compression ratios ($> 6\times\text{--}15\times$).
- **Bounded Relative Error**: Guarantees scale-invariant precision: $\frac{|x - x_{\text{quant}}|}{|x|} \le 0.5 \times 10^{1 - \text{NSD}}$.
- **Native IEEE-754 Compatibility**: Quantized arrays remain standard valid floats and can be directly mapped to Apple Accelerate / BLAS without dequantization passes.

### Alternatives Considered
- **Linear Dynamic Min-Max Quantization (Float32 to UInt16/UInt8)**: Rejected due to dynamic range collapse when values span multiple orders of magnitude, and mandatory dequantization CPU overhead.
- **ZFP / SZ Transform Codecs**: Rejected due to proprietary opaque bitstreams and high CPU compression overhead.

### Source
- Zender, C. S. (2016). *Bit Grooming: statistically accurate precision-preserving quantization*. Geosci. Model Dev., 9, 3199–3211.
- `https://github.com/ccr/ccr` / `https://github.com/Unidata/netcdf-c`
- `https://github.com/Blosc/c-blosc2` (`TRUNC_PREC`, `ZFP_ACC`)

---

## Research Item 4: Blosc2 Frame v2 Standard Serialization & Metalayers (R004)

### Decision
Adopt the **C-Blosc2 Frame v2 Standard Specification** across TTZip's native bridge:
1. **Header Protocol**: Standardize on MsgPack-compatible Header format (`b2frame\0` / `0x62326672`) encoding `typesize`, `blocksize` (128 KB Apple Silicon optimized), `chunksize`, `nchunks`, `nbytes`, `cbytes`, and static metalayers.
2. **Sparse Chunk Indexing**: Implement 64-bit chunk offset tables with MSB Special Tagging (`TTZIP_SPECIAL_TAG_MSB = 1ULL << 63`) for $O(1)$ zero-allocation sparse chunks.
3. **Two-Tier Metalayers**: Use Header metalayers for immutable shape/type schemas (`b2nd`), and Trailer self-compressed `VLMeta` (`TTZIPVLM` / Zstd Level 3) for extensible QuickLook thumbnails and search indexes at EOF.
4. **Hardware-Accelerated Integrity**: Enforce trailer-level CRC-32 (ARM NEON `__crc32d`) / CRC-64 verification.

### Rationale
- **Zero Disk Rewrites for Metadata**: Variable-length metalayers in the trailer allow updating search indices or QuickLook previews by writing only the trailer at EOF without rewriting gigabytes of compressed chunks.
- **Interoperability**: Binary alignment with standard Blosc2 CFrame v2 ensures compatibility with high-performance computing, Python `blosc2` / `b2nd` tensor pipelines, and cross-platform archive readers.

### Alternatives Considered
- **Sequential Tar/Zip Stream Container**: Rejected due to lack of sub-chunk cache locality and $O(1)$ random chunk lookup without linear scanning.
- **Monolithic Header-Only Metadata Layout**: Rejected because updating metadata requires shifting all subsequent compressed chunk payloads.

### Source
- `https://github.com/Blosc/c-blosc2/blob/main/README_CFRAME_FORMAT.rst`
- `https://github.com/Blosc/c-blosc2/blob/main/blosc/frame.c`, `blosc/schunk.c`
- TTZip: `Sources/CTTZipBridge/include/CTTZipSuperChunk.h`, `Sources/CTTZipBridge/include/CTTZipVLMeta.h`
