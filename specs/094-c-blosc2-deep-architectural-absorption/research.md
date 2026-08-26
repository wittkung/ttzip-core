# Grounded Research: C-Blosc2 Exhaustive Architectural Absorption (Feature 094)

## R001: BloscLZ Byte-Oriented Fast LZ77 Engine with 3-Byte Short Matching & L1 Cache Residency

### 1. Decision
Implement native in-process `ttzip_blosclz_compress` and `ttzip_blosclz_decompress` in `Sources/CTTZipBridge/` as an integrated high-speed compression codec for Byte-Shuffled and Bit-Shuffled numerical data streams.

### 2. Rationale
- **3-Byte Short Match Encoding**: Unlike LZ4 which requires $\text{MINMATCH} = 4$, BloscLZ encodes 3-byte matches (`len = 3`), packing 3–8 byte matches with 13-bit distances directly into the control byte and 1 subsequent offset byte. On byte-shuffled numeric data (Float32/Float64/Int64), identical runs of 3 bytes occur with high frequency; BloscLZ captures these runs, achieving $15\%\text{--}40\%$ higher compression ratio than LZ4.
- **L1 D-Cache Table Residency**: Using `HASH_LOG = 12..14` (4,096 to 16,384 entries storing `uint16_t` relative offsets), the entire hash table occupies only $8\text{ KB}\text{--}32\text{ KB}$, completely residing within Apple Silicon M-series L1 data cache ($128\text{ KB}$ per P-core, $64\text{ KB}$ per E-core).
- **Branchless Unaligned 64-Bit Wild Copy**: Decompression performs unaligned 8-byte chunk copies (`wild_copy`), leveraging ARM64 64-bit load/store instructions (`ldr`/`str`) for sustained decompression speeds exceeding $9,000\text{ MB/s}$ per core.

### 3. Alternatives Considered
- **Standard LZ4 without Shuffle**: Rejected because LZ4 on unstructured IEEE-754 mantissa streams yields compression ratios $< 1.1\times$, whereas Shuffle + BloscLZ delivers $2.5\times\text{--}8.0\times$ ratios at multi-gigabyte throughput.
- **Snappy Engine**: Rejected due to branch-heavy varint parsing and tag-byte decoding which introduces pipeline stalls on Apple Silicon superscalar execution units.
- **Deflate (libdeflate/zlib) for In-Memory Slicing**: Rejected because Huffman tree construction caps throughput at $\sim 150\text{ MB/s}$ per core, making it an order of magnitude slower than BloscLZ for real-time memory-bound pipelines.

### 4. Source
- C-Blosc2 Official Repository: `https://github.com/Blosc/c-blosc2` (`blosc/blosclz.c`, `blosc/blosclz.h`, `blosc/shuffle.c`)
- FastLZ Codebase: `https://github.com/ariya/FastLZ` (`fastlz.c`, `fastlz.h`)
- BitShuffle Research Paper: Masui, K. et al., *Bitshuffle: Filter for improving compression of typed binary data* (`https://github.com/kiyo-masui/bitshuffle`)

---

## R002: N-Dimensional Multidimensional Tensor & Array Hyper-Cube Slicing (`b2nd` / `blosc2_nd.c`)

### 1. Decision
Implement `NDimTensorLayout` and `NDimHypercubeChunker` in `Sources/TTZipCore/` to support multi-dimensional array hyper-cube partitioning (Chunks at L3/SLC level $\to$ Blocks at L2 level) with $O(1)$ random-access block seeking via `bstarts` offset tables.

### 2. Rationale
- **Two-Level Hyper-Cubic Partitioning ("Pineapple Partitioning")**: Solves the fundamental read amplification problem of single-level chunking in multidimensional data (HDF5 / Zarr v2). Chunks are sized for disk I/O and System-Level Cache (4MB–16MB), while sub-blocks are sized for CPU L2 cache (64KB–256KB).
- **Selective Block Decompression via `bstarts`**: When extracting an orthogonal 2D slice from a 3D tensor ($1024 \times 1024 \times 64$), the engine calculates coordinate intersections and uses the 32-bit chunk offset directory `bstarts[block_idx]` to seek and decompress only the intersecting micro-blocks, avoiding decompressing $95\%+$ of non-intersecting volume.
- **MessagePack Serialized Metadata (`"b2nd"`)**: Serializes tensor geometry (`ndim`, `shape`, `chunkshape`, `blockshape`, `dtype`) in a compact metalayer for instant zero-copy lookup.

### 3. Alternatives Considered
- **Single-Level Chunking (Standard HDF5 / Zarr v2 Layout)**: Rejected due to severe read amplification when slicing orthogonal axes, wasting $90\%\text{--}99\%$ of CPU and memory bandwidth.
- **Full Decompression to Memory-Mapped Sparse File on APFS**: Rejected because multi-gigabyte scientific arrays would trigger disk exhaustion, SSD wear, and multi-second latency before rendering the first preview frame.
- **External Caterva C Library Submodule**: Rejected because Caterva was formally deprecated and merged directly into C-Blosc2's native `b2nd` layer.

### 4. Source
- C-Blosc2 Repository & B2ND API Header: `https://github.com/Blosc/c-blosc2` (`include/b2nd.h`, `blosc/blosc2_nd.c`, `blosc/b2nd.c`)
- C-Blosc2 B2ND Official Documentation: `https://www.blosc.org/c-blosc2/reference/b2nd.html`
- Blosc2 Chunk & Frame Format Specification: `https://github.com/Blosc/c-blosc2/blob/main/README_FORMAT.rst`

---

## R003: Thread-Local Context Memory Pooling & 64-Byte Cacheline Alignment (`context.c`, `alloc.c`)

### 1. Decision
Adopt a **Thread-Local Execution Context & Scratchpad Memory Pool** (`ttzip_context_pool`) in `Sources/CTTZipBridge/` and `Sources/TTZipCore/NativeCoreArchitecture.swift`, providing lockless working buffer reuse with 64-byte SIMD alignment and 16KB Direct I/O page alignment.

### 2. Rationale
- **Elimination of Lock Bouncing across Performance Cores**: TTZip's existing `MemoryPageFlyweightPool` uses `NSLock` during `borrowBuffer`/`returnBuffer`. Under 8–16 M-series performance cores, global lock acquisition causes mutex contention and cacheline bouncing. Binding pre-allocated scratchpad buffers directly to worker contexts provides $O(1)$ zero-lock access on the hot path.
- **Amortized Multi-Block Memory Reuse**: Scratchpads are allocated once per worker thread upon pool creation and reused across all block/file compression cycles with zero intermediate `malloc`/`free` calls.
- **Hardware Page & SIMD Alignment**: Slabs retain 16KB page alignment (`posix_memalign`) for Direct I/O and 64-byte alignment for ARM NEON vector instructions (`vld1q_u8`/`vst1q_u8`), preventing unaligned memory access penalties.

### 3. Alternatives Considered
- **Centralized MPMC Lock-Free Atomic Ring Buffer**: Rejected because atomic CAS operations still incur CPU cacheline bouncing (`Atomic.compareExchange`) under heavy parallel contention.
- **Per-Task `malloc` / Darwin Nano Zone Allocator**: Rejected because system allocator syscalls and memory zeroing introduce latency spikes, violating TTZip's zero-heap-allocation hot-path invariant.

### 4. Source
- Blosc2 Context & Memory Allocator: `https://github.com/Blosc/c-blosc2` (`blosc/context.c`, `blosc/alloc.c`)
- TTZip Architecture: `Sources/TTZipCore/NativeCoreArchitecture.swift`, `Sources/TTZipCore/Flyweights/MemoryPageFlyweightPool.swift`
