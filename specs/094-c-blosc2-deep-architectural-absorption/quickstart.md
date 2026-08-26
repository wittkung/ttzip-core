# Quickstart: C-Blosc2 Exhaustive Architectural Absorption (Feature 094)

## Validation Scenario 1: Native BloscLZ 3-Byte Match Compression & Roundtrip Parity

### Command
```bash
swift test --filter BloscLZNativeEngineTests/testBloscLZCompressAndDecompressParity
```

### Expected Output
- Execution of BloscLZ roundtrip across uniform, linear-ramp, floating-point sensor array, and high-entropy blocks.
- Bitwise parity verified (`XCTAssertEqual(decompressed, source)`).
- Throughput verified: Compression $\ge 3,500\text{ MB/s}$, Decompression $\ge 9,000\text{ MB/s}$.

### Failure Diagnostic
- If assertion fails on large distance matches ($> 8191$), verify `MAX_FARDISTANCE` 16-bit offset unpacking in `ttzip_blosclz_decompress`.
- If memory corruption occurs, check `wild_copy` 8-byte pointer boundary bounds checks against destination end buffer.

---

## Validation Scenario 2: N-Dimensional Tensor Orthogonal Hyper-Cube Slicing (`b2nd`)

### Command
```bash
swift test --filter NDimTensorHypercubeSlicingTests/test3DTensorOrthogonalCrossSectionSlicing
```

### Expected Output
- Construction of a $512 \times 512 \times 64$ Float32 3D tensor partitioned into $128 \times 128 \times 16$ chunks and $32 \times 32 \times 4$ blocks.
- Extraction of orthogonal 2D cross-section plane $[0..512, 0..512, 32..33]$ in $< 3.0\text{ ms}$.
- Assert that decompressed block count is $\le 5\%$ of total dataset blocks.

### Failure Diagnostic
- If intersecting block coordinate calculations return out-of-bound indices, check coordinate floor-division rounding in `NDimHypercubeChunker.swift`.

---

## Validation Scenario 3: Thread-Local Context Memory Pool Zero Allocation Verification

### Command
```bash
swift test --filter ContextMemoryPoolTests/testMultiThreadedZeroHeapAllocationInHotLoop
```

### Expected Output
- 10,000 iterations of 64KB block compression across 16 worker tasks.
- 0 dynamic `malloc`/`free` calls during iteration execution.
- 100% 64-byte SIMD and 16KB page alignment verified on all working pointers.

### Failure Diagnostic
- If memory allocation count $> 0$, inspect intermediate Swift `Data(count:)` instantiation in buffer bridging methods.
