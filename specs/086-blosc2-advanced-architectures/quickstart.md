# Quickstart & Verification Scenarios

## Scenario 1: NEON Float Truncate Precision & Shuffle Synergy
- **Command**: `swift test --filter Blosc2AdvancedArchitecturesTests/testNeonFloatTruncateFilterAndShuffleSynergy`
- **Expected Output**: Float32 truncate + NEON Shuffle + Deflate passes with >= 10x ratio and zero numerical drift.
- **Failure Diagnostic**: Verify ARM NEON SIMD availability (`__ARM_NEON`) and mantissa bit masking logic.

---

## Scenario 2: Double-Buffered Async Prefetch Pipeline Multi-Thread Stress
- **Command**: `swift test --filter Blosc2AdvancedArchitecturesTests/testDoubleBufferedAsyncPrefetchPipeline`
- **Expected Output**: 10,000 slots acquired and released across 8 concurrent threads in < 0.05s with 0 deadlocks.
- **Failure Diagnostic**: Check mutex lock/condition variable signal order and slot state machine transitions.

---

## Scenario 3: VLMeta Variable-Length Self-Compressed Trailer Append & Query
- **Command**: `swift test --filter Blosc2AdvancedArchitecturesTests/testVLMetaTrailerSerializationAndAppend`
- **Expected Output**: Metadata layer appended to archive EOF and retrieved with 100% byte fidelity in < 1 ms.
- **Failure Diagnostic**: Check 16-byte trailer footer offset calculation and Zstd decompression context.

---

## Scenario 4: N-Dimensional Tensor Strided Slicing & Zero-Copy View
- **Command**: `swift test --filter Blosc2AdvancedArchitecturesTests/testNDimensionalTensorHyperCubeSlicing`
- **Expected Output**: 4D tensor coordinates correctly map to Chunk/Block/Elem indices and 2D cross-section is extracted in < 2 ms.
- **Failure Diagnostic**: Validate row-major stride multiplication formula and bounding box min/max chunk index ranges.
