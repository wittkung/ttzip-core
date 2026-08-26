# Phase 0: Research & Architectural Decision Records

## R001: ARM NEON Float Precision Truncation & Bit-Grooming Vectorization

- **Decision**: Implement fused IEEE-754 mantissa truncation with unbiased half-bit rounding using ARM NEON SIMD (`vdupq_n_u32`, `vaddq_u32`, `vandq_u32`) for `float32` and `vdupq_n_u64`, `vaddq_u64`, `vandq_u64` for `float64`.
- **Rationale**: Low-order mantissa bits in floating point tensors (e.g. ML checkpoints, sensor streams) represent random measurement noise. Zeroing the lowest $Q$ bits transforms noisy bytes into pure zeros. When passed through Byte Shuffle, these zeros coalesce into pure 0x00 planes, triggering maximum run-length matches in Deflate/Zstd and increasing compression ratio from 1.2x to 15x~40x at >15 GB/s.
- **Alternatives Considered**:
  - *Standard quantization (scaling + int cast)*: Rejected because it requires knowing global min/max bounds upfront and ruins dynamic range for very small/large exponents.
  - *Bit-shaving toward zero without bias*: Rejected because it introduces negative statistical drift ($\mathbb{E}[\Delta] < 0$). Half-bit rounding ensures $\mathbb{E}[\Delta] \approx 0$.
- **Source**: `c-blosc2/plugins/filters/truncate/truncate.c`, IEEE-754 standard, `Sources/CTTZipBridge/CTTZipFilterPipeline.c`.

---

## R002: Double-Buffered Slot-Based Async Prefetch Ring Buffer

- **Decision**: Implement a 2-slot or 4-slot ring buffer state machine (`SLOT_EMPTY` $\to$ `SLOT_LOADING` $\to$ `SLOT_READY` $\to$ `SLOT_CONSUMING`) protected by POSIX mutexes/condition variables and 128-byte aligned memory pages (`ttzip_core_aligned_alloc_128b`).
- **Rationale**: Sequential archive extraction from external SSDs or network storage suffers from I/O-CPU serialization bubbles. Double buffering allows the I/O thread to asynchronously prefetch block $K+1$ via POSIX `pread` while SIMD decompression worker threads consume block $K$, overlapping I/O latency completely.
- **Alternatives Considered**:
  - *Single large memory-mapped file*: Rejected for huge multi-gigabyte archives on memory-constrained systems as it triggers page fault thrashing and TLB shootdowns.
  - *Grand Central Dispatch Semaphore Throttle*: Rejected in hot loops due to kernel transition overhead; slot-based state machines with condition variables provide zero-allocation determinism.
- **Source**: `c-blosc2/blosc/schunk.c`, `Sources/CTTZipBridge/include/CTTZipSysAlloc.h`.

---

## R003: VLMeta Variable-Length Self-Compressed Metalayers Trailer Engine

- **Decision**: Standardize a binary trailer layout: `[Header: Magic "TTZIPVLM\0" + UncSize + CompSize + LayerCount][Zstd Compressed MessagePack Block][Footer: 16B: Magic + Offset]`.
- **Rationale**: ZIP Extra Fields are limited to 64KB and uncompressed. TAR Pax Headers are ASCII text streams. VLMeta trailers support up to 2GB of rich structured metadata (QuickLook thumbnails, search inverted indices, AES-256 AAD), compressed with Zstd, and appendable at EOF in $O(1)$ time without modifying preceding file offsets or rewriting gigabytes of payload.
- **Alternatives Considered**:
  - *Storing metadata in Central Directory extra fields*: Rejected due to 64KB uint16 size overflow and lack of native compression.
  - *Separate sidecar files (`.ttzip-meta`)*: Rejected because sidecars easily get lost during file moves and sharing.
- **Source**: `c-blosc2/blosc/frame.c` (VLMetalayers specification), PKWARE APPNOTE.TXT.

---

## R004: B2ND N-Dimensional Hyper-Cube Slicing & Coordinate Mapping

- **Decision**: Implement closed-form $D$-dimensional coordinate translation mapping global coordinates $X = (x_0, \dots, x_{D-1})$ into $(C_{\text{idx}}, B_{\text{idx}}, \Delta_{\text{elem}})$ indices, combined with bounding box pruning and zero-copy strided view slices.
- **Rationale**: Reading sub-slices of massive multi-dimensional tensors (e.g. Safetensors, HDF5, Zarr) historically suffered from 100x read amplification because full chunks had to be decompressed. Two-level hyper-cube partitioning enables microsecond-level random access to arbitrary coordinate planes.
- **Alternatives Considered**:
  - *1D linear chunking only*: Rejected because cross-dimension slicing touches every single chunk.
  - *Full uncompressed memory mapping*: Rejected because 100GB+ arrays cannot fit in RAM on standard client machines.
- **Source**: `c-blosc2/b2nd/b2nd.c`, `c-blosc2/caterva/caterva.c`.
