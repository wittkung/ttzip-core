# Feature Specification: Blosc2 Advanced Architectures Comprehensive Integration

## Overview

Comprehensive integration of Blosc2's four state-of-the-art architectures into TTZip:
1. **NEON Float Precision Truncation & Bit-Grooming Filter**: IEEE-754 mantissa truncation with half-bit rounding and Bit-Grooming, achieving 5x~20x compression ratio multiplication when fused with NEON Shuffle and Deflate/Zstd.
2. **Double-Buffered Async Prefetch Pipeline**: Slot-based ring buffer state machine overlapping background disk/network I/O with SIMD decompression cores.
3. **VLMeta Variable-Length Self-Compressed Metalayers Engine**: Appendable EOF trailer with Zstd-compressed MessagePack structured metadata for O(1) thumbnail and search index updates.
4. **N-Dimensional Tensor Hyper-Cube Slicing Engine**: Two-level coordinate mapping (Chunk/Block) enabling sub-5ms arbitrary strided slicing and zero-copy views on large multi-gigabyte Safetensors and array archives.

---

## User Scenarios & Acceptance Criteria

### User Scenario 1: High-Ratio Scientific & Model Tensor Archiving (Float Truncate + Shuffle)
- **Actor**: ML Engineer / Data Scientist packaging model checkpoints or numerical datasets.
- **Action**: Packs float datasets specifying mantissa precision retention (e.g. 7 bits or 10 bits).
- **Expectation**: Raw float arrays are filtered at > 12 GB/s on Apple Silicon NEON, zeroing noisy mantissa bytes; NEON Shuffle groups zeros into contiguous planes; Deflate/Zstd achieves 8x~25x compression ratio without observable numerical bias.

### User Scenario 2: High-Throughput Streaming Extraction from External / Network Disks (Double-Buffered Prefetch)
- **Actor**: Power user unarchiving a 50GB file from external SSD or SMB network mount.
- **Action**: Triggers parallel extraction pipeline.
- **Expectation**: Background prefetch worker loads slot K+1 while decompression workers consume slot K, eliminating CPU idle bubbles and saturating read bus bandwidth.

### User Scenario 3: Instant Archive Thumbnail & Search Index Updates (VLMeta Trailer)
- **Actor**: TTZip Desktop App / QuickLook Preview generator.
- **Action**: Updates archive metadata, QuickLook thumbnail cache, or search inverted index.
- **Expectation**: New metadata is serialized, compressed via Zstd, and appended to the EOF trailer in O(1) time (< 1 ms), leaving gigabytes of compressed payloads untouched.

### User Scenario 4: Instant Safetensors / Array Sub-Cube Slicing
- **Actor**: App inspecting a 50GB multi-dimensional model archive.
- **Action**: Requests a 2D slice from a 4D tensor (e.g. `[0, :, :, 0]`).
- **Expectation**: Slicing arbiter calculates exact Chunk and Block coordinates via closed-form geometry, prunes non-intersecting blocks, and extracts only the requested 100KB slice in < 5 ms with zero full-tensor decompression.

---

## Functional Requirements

- **FR-001**: `CTTZipFilterPipeline` must support `ttzip_filter_truncate_float32_neon` and `ttzip_filter_truncate_float64_neon` with unbiased half-bit rounding.
- **FR-002**: Float truncation combined with Byte Shuffle must achieve bit-for-bit lossless precision recovery for the preserved bit width.
- **FR-003**: `CTTZipPrefetchPipeline` must implement a 2-slot or 4-slot ring buffer state machine with `ttzip_prefetch_acquire` and `ttzip_prefetch_release`.
- **FR-004**: Prefetch ring buffers must use 128-byte cacheline aligned page buffers allocated via `ttzip_core_aligned_alloc_128b`.
- **FR-005**: `CTTZipVLMeta` must encode a binary trailer with magic `"TTZIPVLM\0"`, compressed metadata payload, and 16-byte fixed footer.
- **FR-006**: Appending a new VLMeta layer to an existing archive must operate strictly at EOF in $O(1)$ time without modifying preceding file offsets.
- **FR-007**: `CTTZipTensorSlicing` must implement closed-form $D$-dimensional coordinate translation to `(chunk_idx, block_idx, elem_offset)`.
- **FR-008**: B2ND strided slicing must execute bounding-box pruning and zero-copy view generation for contiguous dimensions.

---

## Success Criteria

- **SC-001**: Float32 truncate filter with 7-bit mantissa on Apple Silicon NEON executes at >= 10,000 MB/s.
- **SC-002**: Truncate + Shuffle + Deflate on synthetic float corpus achieves >= 10x compression ratio (vs < 1.3x un-filtered).
- **SC-003**: Prefetch pipeline acquires and releases 10,000 slots concurrently across multiple threads without deadlocks or memory leaks.
- **SC-004**: VLMeta trailer serialization, append, and readback roundtrip passes with 100% data integrity.
- **SC-005**: N-dimensional slicing extracts a 2D cross-section from a 4D tensor in <= 5.0 ms with zero memory corruption.
- **SC-006**: Full regression test suite (525+ tests) and 13 performance floors maintain 100% green pass.

---

## Clarifications

1. **Precision Range**: Float32 mantissa bits configurable from 1 to 23 bits (default 7 bits for single-precision ML weights); Float64 configurable from 1 to 52 bits (default 14 bits).
2. **Container Compatibility**: VLMeta trailers are appended after the standard archive payload / Central Directory, preserving full backward compatibility with standard `/usr/bin/unzip`, `/usr/bin/tar`, and `7z`.
