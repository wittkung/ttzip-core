# Comprehensive Research: Compression Formats and Underlying Algorithmic Foundations

**Feature**: `138-compression-formats-algorithms`  
**Date**: 2026-08-20  
**Status**: Consolidated & Grounded

---

## 1. Executive Synthesis & Architectural Landscape

TTZip's archiving engine bridges high-level user and CLI interactions to low-level POSIX and Apple Silicon hardware primitives. The core system manages two orthogonal dimensions:
1. **Container Framing & Packaging Topology**: How files, directories, POSIX attributes, and compression chunks are encapsulated, indexed, and navigated (Random-Access vs. Sequential Streaming).
2. **Underlying Compression, Entropy, and Transform Algorithms**: The mathematical heuristics, sliding-window dictionary models, statistical contexts, entropy coders, and SIMD hardware kernels operating across the byte stream.

```
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                   ARCHIVE CONTAINER LAYER                                       │
│   Random-Access (ZIP, 7Z, WIM, DMG, ISO)   │   Sequential Streaming (TAR, TAR.ZST, GZ, AAR)     │
└────────────────────────────────────────┬─────────────────────────────────────────────────────────┘
                                         │ Stream & Chunk Routing
┌────────────────────────────────────────▼─────────────────────────────────────────────────────────┐
│                                 DOMAIN PRE-FILTERING LAYER                                       │
│   * BCJ / BCJ2 (x86/ARM Branch Call Delta) │   * Bit-Grooming / BitRound (IEEE 754 Floats)       │
│   * Byte-Shuffle / Bit-Shuffle (SIMD Trans)│   * Transposed Delta Filter (d_i = x_i - x_{i-1})    │
│   * rzip Long-Range Hash Tree (Multi-GB)   │   * Multimedia / Channel Delta (RAR / Audio)        │
└────────────────────────────────────────┬─────────────────────────────────────────────────────────┘
                                         │ Byte Slices
┌────────────────────────────────────────▼─────────────────────────────────────────────────────────┐
│                             MATCH FINDING & DICTIONARY PLANE                                     │
│   * SWAR 64-bit Word XOR Matching          │   * Apple Silicon NEON 128-bit SIMD Vector Search   │
│   * 3-Byte / 4-Byte Multiplicative Hash    │   * Binary Trees & Hash Chains (BT4, HC4)           │
│   * 2-Step Lazy Evaluation                 │   * Forward Dynamic Programming (DAG Shortest Path) │
│   * Burrows-Wheeler Transform (BWT Suffix) │   * PPMd Order-k Statistical Context Memory         │
└────────────────────────────────────────┬─────────────────────────────────────────────────────────┘
                                         │ Tokens / Frequencies
┌────────────────────────────────────────▼─────────────────────────────────────────────────────────┐
│                              ENTROPY & ARITHMETIC CODING PLANE                                   │
│   * Finite State Entropy (tANS / FSE)      │   * Binary Arithmetic Range Coder (11-bit BARC)     │
│   * Canonical Huffman (Dynamic / In-Place) │   * 4-Stream Interleaved Superscalar Huffman        │
│   * Subbotin Range Coder                   │   * Raw Byte Tokens (Zero-Entropy Wildcopy)         │
└────────────────────────────────────────┬─────────────────────────────────────────────────────────┘
                                         │ Instructions / Micro-Kernels
┌────────────────────────────────────────▼─────────────────────────────────────────────────────────┐
│                           HARDWARE ACCELERATION & INTEGRITY KERNELS                              │
│   * ARM64 PMULL CRC64 (`vmull_p64` @ 48.16 GB/s)  │   * ARMv8 ACLE CRC32 (`__crc32b/w/d` @ 65 GB/s)  │
│   * ARM NEON DotProduct Adler-32 (N_max = 5552)   │   * ARMv8 AES-256 Vector Pipeline (8-Way)    │
│   * Thread-Local L2 Cache Scratch Arenas          │   * APFS Zero-Copy `clonefile` / `mmap`      │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Research Item R001: Container Framing and Layout Architecture

### Decision
TTZip implements native in-process C decoders and parsers for two distinct container paradigms across 16 primary formats:
1. **Random-Access / Seekable Containers**:
   - **ZIP / Zip64**: Employs Central Directory File Header (CDFH) and End-of-Central-Directory (EOCD / Zip64 Locator) anchoring. Features Local File Headers (`0x04034b50`), CDFH (`0x02014b50`), EOCD (`0x06054b50`), WinZip AES extra field (`0x9901`), and Info-ZIP POSIX timestamp/UID extra fields (`0x5455`, `0x7875`).
   - **7Z**: Employs 32-byte Start Header signature (`0x37 0x7A 0xBC 0xAF 0x27 0x1C`) indexing relative tail Header Streams via branchless Varint decoders. Supports complex Coders DAG graphs (chaining LZMA2, BCJ, AES) and Encrypted Header Streams (`0x17`) that secure filenames and directory hierarchies.
   - **WIM (Windows Imaging Format)**: 208-byte WIM header (`MSWIM\0\0\0`), 32KB chunk offset tables allowing $O(1)$ random seek inside compressed streams, and SHA-1 single-instance content deduplication tables.
   - **DMG (Apple UDIF)**: 512-byte `koly` trailer at EOF (`0x6B6F6C79`), base64 XML plist embedded resource fork, and `mish` block descriptor tables indexing sparse `ZERO`, `RAW`, `ZLIB`, `BZIP2`, `LZFSE`, and `LZMA` chunk extents.
   - **ISO 9660 / ECMA-119**: Sector 16 Primary/Supplementary Volume Descriptors (PVD/SVD), dual-endian ("Both-Endian") integers, Joliet UCS-2BE Unicode, and Rock Ridge POSIX attribute records (`PX`, `NM`, `SL`).
2. **Sequential Streaming Containers**:
   - **TAR Family (.tar, .tar.gz, .tar.zst, .tar.bz2, .tar.xz, .tar.lz, .tar.sz)**: Continuous 512-byte block alignment, UStar and POSIX.1-2001 PAX extended headers (`typeflag 'x'` and `'g'`), GNU sparse descriptors, and double 512-byte zero block EOF sentinels.
   - **Apple Archive (.aar)**: Structured streaming key-value attributes (`TYP`, `PAT`, `DAT`, `MOD`, `XAT`, `ACL`, `FLAG`) coupled with chunked LZFSE / LZ4 / ZSTD byte streams.

### Rationale
- **Zero-Copy Mmap**: Seekable formats allow `mmap` with `posix_madvise(MADV_WILLNEED)` to traverse directory indices in virtual address space without copying gigabytes of payload into heap memory.
- **NEON SIMD Anchor Hunting**: ZIP EOCD discovery backward search is vectorized via ARM64 NEON (`vceqq_u32`), scanning 16 bytes per clock cycle.
- **APFS Pre-Allocation**: Uncompressed payload sizes discovered in container headers trigger APFS contiguous allocation (`fstore_t`), eliminating filesystem fragmentation on SSDs.

### Alternatives Considered
- **Single Universal Streaming Container (e.g. Forcing all outputs to TAR)**: Rejected because TAR requires $O(N)$ sequential decompression scans for random file inspection, destroying GUI and QuickLook latency on multi-gigabyte archives.
- **Single Universal Random-Access Container (e.g. Forcing all outputs to ZIP)**: Rejected because ZIP cannot stream over standard Unix pipelines (`stdin`/`stdout`) without non-standard data descriptors and post-hoc central directory writes.

### Source
- `Sources/TTZipCore/ArchiveCompressionTypes.swift`
- `Sources/CTTZipBridge/CTTZipParser.c` & `include/CTTZipParser.h`
- `Sources/CTTZipBridge/ttzip_7z_header_parser.c` & `include/ttzip_7z_header_parser.h`
- `Sources/CTTZipBridge/ttzip_tar_native.c` & `ttzip_tar_zstd_direct.c`
- `Sources/CTTZipBridge/ttzip_dmg_demux.c`
- `Sources/TTZipCore/NativeAppleArchiveEngine.swift`
- PKWARE APPNOTE.TXT v6.3.9, 7z Format Spec, POSIX.1-2001 Pax Spec, Apple UDIF Spec.

---

## 3. Research Item R002: Deflate & LZMA Family Deep Compression Theory

### Decision
1. **Deflate (RFC 1951)**:
   - **LZ77 Model**: 32KB sliding window ring buffer ($2^{15}$ bytes), match lengths 3..258 bytes, backward distances 1..32,768 bytes. Overlapping matches ($L > D$) produce efficient periodic run-length patterns.
   - **Match Finders**: 3-byte / 4-byte multiplicative hash chains, compact 4-way direct associative buckets, 64-bit SWAR (SIMD Within A Register) `CTZLL` XOR word comparison, and 128-bit ARM64 NEON vector comparison.
   - **Entropy & Parsing**: Canonical Huffman coding (Moffat-Katajainen in-place package-merge bounded to $L_{max} \le 15$), dynamic header RLE bitstream packing, greedy parsing, 2-step lazy evaluation, forward dynamic programming (DAG shortest path), and Zopfli iterative entropy shortest-path optimization.
2. **LZMA / LZMA2**:
   - **Architecture**: Scalable dictionary up to 1GB ($2^{30}$ bytes), match lengths 2..273 bytes, and 4 LRU repeat match distance registers (`rep0`..`rep3`) enabling 1-bit `ShortRep` RLE tokens.
   - **Binary Arithmetic Range Coder (BARC)**: 11-bit fixed-point probability engine ($M=2048$, $P_{init}=1024$), branchless probability adaptation ($P \leftarrow P \pm (2048-P) \gg 5$), and 12-state Markov chain tracking literal/match context history.
   - **Context Trees & Filters**: Literal context bits ($lc=3$), literal position bits ($lp=0$), position bits ($pb=2$), 64 distance slots with reverse bit trees, and in-place BCJ / BCJ2 instruction delta pre-filters for ARM64 (`B`/`BL` opcode `0x14000000`) and x86 (`0xE8`).
   - **LZMA2 Framing**: 2MB uncompressed chunk encapsulation, dictionary reset flags, and uncompressed fallback blocks to prevent data expansion.

### Rationale
- **Complementary Trade-offs**: Deflate guarantees minimal memory footprint (32KB window) and multi-gigabyte/sec decompression throughput for web and ZIP standards; LZMA2 provides extreme compression density (20%–40% smaller than Deflate) and large-dictionary deduplication for cold software distribution.

### Alternatives Considered
- **Monolithic LZMA1**: Rejected due to lack of chunk framing, inability to handle uncompressible data without size inflation, and lack of multi-threaded parallel decompression.
- **QuickLZ / LZO**: Evaluated for pure speed, but rejected as core standard formats due to weak compression ratios and lack of formal international container standardization.

### Source
- `Sources/CTTZipBridge/native_deflate/` (`ttzip_deflate_engine.c`, `ttzip_deflate_huffman.c`, `ttzip_deflate_fast.c`, `ttzip_deflate_lazy.c`)
- `Sources/CTTZipBridge/fast-lzma2/` (`fast-lzma2.h`, `lzma2_enc.c`, `range_enc.h`, `radix_mf.c`)
- `Sources/CTTZipBridge/ttzip_bcj_arm64_neon.c` & `ttzip_zopfli_engine.c`
- IETF RFC 1951, Igor Pavlov LZMA SDK Documentation.

---

## 4. Research Item R003: Modern High-Throughput Compression Algorithms

### Decision
TTZip integrates five modern state-of-the-art compression engines:
1. **Zstandard (Zstd / RFC 8878)**:
   - **Finite State Entropy (FSE / tANS)**: Division-free 1-state decoding table transitions ($s \to s'$ via 1 table lookup + 1 bitstream extract + 1 addition). Interleaves Literals Length, Match Length, and Offset states concurrently.
   - **Match Finders**: Single Hash Table (Fast L1-2), Double Fast Table (Short 4B + Long 8B for L3-4), Binary Tree Lazy (L5-15), Optimal Graph Parsing (L16-22), and Long Distance Matching (LDM) with 64-bit Gear rolling hash spanning up to 2GB windows.
   - **Huffman & Dictionaries**: 4-stream interleaved superscalar Huffman decoding (>3.5 GB/s) and Cover / FastCover pre-trained dictionary engines.
2. **LZ4 / LZ4-HC**:
   - **Byte-Aligned Token Stream**: 1-byte token ($4\text{b } LL \mid 4\text{b } ML$) + 2-byte little-endian offset (64KB window) + raw literal runs.
   - **SIMD Wildcopy**: Uncompressed literal runs copied directly via 16B/32B/64B vector instructions without bit shifts or entropy decoders, achieving >4.5 GB/s decompression.
3. **Apple LZFSE**:
   - Proprietary Apple algorithm combining LZ77 matching with 4-state interleaved Finite State Entropy for literals and 3 sequence states ($L, M, D$).
   - Utilizes thread-local 2.03MB scratch arenas (`s_decode_scratch_key`), fitting entirely within Apple Silicon unified L2 cache (12MB–32MB) for optimal energy efficiency.
4. **Snappy**:
   - Google byte-oriented LZ77 engine using 1-byte tag headers (Literal, 1B Copy, 2B Copy, 4B Copy). Bypasses entropy coders entirely for multi-gigabyte throughput.
5. **Brotli (RFC 7932)**:
   - 2nd-order context modeling (64 literal contexts, 4 distance contexts), sliding window up to 16MB/1GB, and a 120KB built-in static dictionary containing 13,504 common web strings and 121 transformations.

### Rationale
- Modern applications require decompressors that scale with NVMe and memory bus bandwidth (>2–5 GB/s). FSE and byte-token streams eliminate the sequential instruction bottlenecks of Huffman and Range Coders on modern Out-of-Order CPU microarchitectures.

### Alternatives Considered
- **Using Zstandard for all tasks**: While Zstandard is versatile, LZ4 is 2.5x–3x faster at decompression, LZFSE is optimized for Apple Silicon battery conservation, and Brotli provides superior density on small web assets due to its 120KB static dictionary.

### Source
- `Sources/CTTZipBridge/CTTZipBridge_Zstd.c`, `CTTZipBridge_LZFSE.c`, `CTTZipBridge_Snappy.c`
- `Vendor/lz4-upstream/lib/lz4.c` & `lz4hc.c`
- `Vendor/zstd-upstream/lib/compress/` (`zstd_fast.c`, `zstd_double_fast.c`, `zstd_opt.c`, `zstd_ldm.c`, `fse_compress.c`)
- RFC 8878, RFC 7932, Jarek Duda tANS Papers (arXiv:1311.2540).

---

## 5. Research Item R004: BWT, Statistical, Multi-Gigabyte & Domain-Specific Engines

### Decision
1. **BZIP2 / Burrows-Wheeler Transform**:
   - 5-stage pipeline: RLE1 (caps identical runs to 4 bytes) $\to$ BWT Suffix Sorting (Larsson-Sadakane / dual-pivot quicksort over 100KB–900KB blocks) $\to$ Move-To-Front (MTF) $\to$ RLE2 (`RUNA`/`RUNB` bijective base-2 zero-run encoding) $\to$ Multi-Tree Canonical Huffman (2–6 code tables clustered per 50-symbol chunks).
   - Inversion: Linear-time $O(N)$ zero-comparison reconstruction via transformation vector $T[i] = C[L[i]] + \text{rank}(L[i], i)$.
2. **PPMd (Dmitry Shkarin Model H / I)**:
   - Order-$k$ ($k=4..16$) trie context tree estimating $P(x_n \mid x_{n-1} \dots x_{n-k})$.
   - Implements the Exclusion Principle and Secondary Error Estimation (SEE) tables for dynamic escape probability prediction.
   - Managed via a static slab Sub-Allocator (`SubAlloc`) with buddy-bin indexing and Dmitry Subbotin Range Coding.
3. **LRZIP (Long Range ZIP)**:
   - 1st-stage `rzip`: Large-scale dictionary search in physical RAM indexing files via sliding block hash trees (Rabin-Karp / rolling hashes on matches $\ge 32$ bytes separated by megabytes to tens of gigabytes).
   - 2nd-stage backend: Passes separated literal and match offset streams to Zstandard, LZMA2, or Bzip2.
4. **Domain Pre-Filters & Transforms**:
   - **Bit-Grooming & BitRound**: Precision-preserving quantization on IEEE 754 float32/float64 numbers. Calculates $prc = \lceil 3.32 \times NSD \rceil + 1$ and masks stochastic mantissa thermal noise using NEON vector instructions (`CTTZipBitGroom.c`).
   - **Byte-Shuffle / Bit-Shuffle**: Transposes $N \times K$ byte matrices into contiguous byte planes (and $8 \times 8$ bit matrices) via `ttzip_neon_transpose_8x8_2x`, transforming floating-point exponents into long compressible zero-runs.
   - **Transposed Delta**: Stores relative differences $d_i = x_i - x_{i-1}$, inverted via ARM64 NEON prefix-sum vector pipelines (`vextq_u8`, `vaddq_u8`).
5. **RAR Engine Family**:
   - Evolution from RAR 1.5–2.0 (LZSS + multimedia delta) to RAR 3.0/4.0 (PPMII + RAR Bytecode VM) and RAR 5.0 (eliminated VM attack surface, added up to 4GB dictionary, ARM/x86 executable call filters, AES-256-CBC, Blake2sp).

### Rationale
- Scientific floating-point arrays compress poorly with standard LZ ($1.05\times$), but achieve $5.5\times \sim 18.2\times$ after Bit-Grooming and Byte-Shuffle.
- Multi-gigabyte virtual disk images overflow standard LZ window sizes, but LRZIP's RAM-scale rzip pass strips multi-gigabyte duplications cleanly.
- Natural language and source code benefit from PPMd's higher-order context modeling where rigid byte-string matches fail.

### Alternatives Considered
- **Bit-Shaving (Truncation to Zero)**: Rejected because all-zero truncation introduces systematic negative bias into scientific datasets; Bit-Grooming and BitRound preserve ensemble means and physical conservation laws.

### Source
- `Sources/CTTZipBridge/CTTZipBitGroom.c` & `CTTZipFilterPipeline.c`
- `Sources/CTTZipBridge/CTTZipBridge_UnRAR.c`
- `Sources/TTZipCore/SevenZip/SevenZipModels.swift`
- Burrows & Wheeler (1994), Shkarin (2002), Tridgell (1999), Zender (2016).

---

## 6. Research Item R005: Hardware Acceleration & Cryptographic Integrity Subsystems

### Decision
1. **Hardware Checksums & Hashing**:
   - **ARM64 PMULL CRC64 (`ttzip_crc64.c`)**: 4-way unrolled 512-bit SIMD vector folding using Galois Field polynomial carry-less multiplication (`vmull_p64`) and Barrett reduction without division. Achieves **48,160 MB/s (47.0 GB/s)**.
   - **ARMv8 ACLE CRC32 (`CTTZipCRC32Neon.c`)**: 12-way PMULL vector folding paired with hardware assembly instructions `__crc32d/w/h/b`, sustaining $>65\text{ GB/s}$.
   - **ARM NEON DotProduct Adler-32 (`CTTZipAdler32Neon.c`)**: Bypasses modulo 65521 for up to $N_{\max} = 5552$ bytes via mathematical proof of 32-bit non-overflow. Employs `vdotq_u32` for horizontal dot products at $25\sim30+\text{ GB/s}$.
   - **Cryptographic Hashes**: xxHash64 block headers, BLAKE2sp 8-way parallel SIMD tree hashing, and hardware `FEAT_SHA256`.
2. **Cryptographic Engine**:
   - **7Z AES-256-CBC**: Stack-allocated 536-byte zero-heap SHA-256 KDF buffer with 512KB parallel chunk decryption.
   - **WinZip AES-256 (AE-1 / AE-2)**: AES-CTR mode with PBKDF2-HMAC-SHA1 (1,000 iterations), 8-slot thread-local key caching, and 10-byte HMAC-SHA1 authentication tags.
   - **ARMv8 Crypto Vector Extensions (`CTTZipBridge_Crypto.c`)**: 8-way interleaved pipelining ($8 \times 16 = 128$ bytes/iter) using `vaeseq_u8` and `vaesmcq_u8`, saturating CPU execution units.
   - **Memory Eradication**: Strict enforcement of `ttzip_secure_zero` (`memset_s` / `explicit_bzero` + compiler barrier) on keys, passwords, and intermediate states.
3. **Microarchitectural Invariants**:
   - **Topology Tuning (`AppleSiliconTuner.swift`)**: Dynamic P/E-core core pool configuration, `QOS_CLASS_USER_INTERACTIVE` thread boosting, and memory-bandwidth scaling.
   - **Zero-Copy Memory-Mapped I/O (`CTTZipBridge_Mmap.c`)**: APFS page-aligned buffers with `posix_madvise(MADV_WILLNEED | MADV_SEQUENTIAL)`.

### Rationale
- Moving cryptographic and checksum verification into hand-crafted C and ARM64 vector assembly eliminates kernel context-switching and ensures wire-speed throughput on PCIe Gen 4/5 SSDs and Apple Silicon unified memory.

### Alternatives Considered
- **Software Table-Lookup CRC32/CRC64**: Rejected because table lookups thrash L1 D-cache and cap throughput at $\sim 1.35\text{ GB/s}$, creating severe extraction bottlenecks.
- **External CLI Subprocess Invocations (`posix_spawn`/`7zz`/`pigz`)**: Rejected because fork/exec incurs $15\sim45\text{ ms}$ latency per file, whereas in-process C execution completes in $< 0.001\text{ ms}$ ($< 1\text{ }\mu\text{s}$).

### Source
- `Sources/CTTZipBridge/ttzip_crc64.c`, `CTTZipCRC32Neon.c`, `CTTZipAdler32Neon.c`, `CTTZipBridge_Crypto.c`
- `Sources/TTZipCore/AppleSiliconTuner.swift`, `Sources/CTTZipBridge/CTTZipBridge_Mmap.c`
- `docs/PERFORMANCE.md` & `docs/ASSEMBLY_INFRASTRUCTURE_ARCHITECTURE.md`.
