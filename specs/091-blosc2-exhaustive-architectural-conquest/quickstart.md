# Quickstart Validation Guide: Blosc2 Exhaustive Architectural Conquest

**Feature**: `specs/091-blosc2-exhaustive-architectural-conquest`  
**Date**: 2026-08-18  

---

## Scenario 1: Dynamic Filter/Codec Plugin Registration & Dispatch

### Purpose
Prove user-defined filter plugins in range $160\text{--}255$ can be registered without heap allocations and invoked with zero contention.

### Command
```bash
swift test --filter Blosc2PluginRegistryTests
```

### Expected Output
```text
Test Case '-[TTZipTests.Blosc2PluginRegistryTests testDynamicFilterPluginRegistrationAndDispatch]' passed (0.001 seconds).
Test Case '-[TTZipTests.Blosc2PluginRegistryTests testInvalidPluginIDRejection]' passed (0.000 seconds).
```

### Failure Diagnostic
- Verify plugin IDs are strictly bounded in $[160, 255]$.
- Check that atomic loads use `memory_order_acquire`.

---

## Scenario 2: Block-Level Lazy Slicing & Zero-Copy Range Extraction

### Purpose
Validate that extracting 4KB from a 4MB Chunk decompresses only the 1 intersecting 128KB micro-block, bypassing the remaining 31 blocks ($96.9\%$ bypass rate).

### Command
```bash
swift test --filter Blosc2LazySlicingTests
```

### Expected Output
```text
Test Case '-[TTZipTests.Blosc2LazySlicingTests testLazyBlockSlicingAccuracyAndBypass]' passed (0.002 seconds).
```

### Failure Diagnostic
- Ensure block index math `first_block = start / block_size` and `last_block = (start + length - 1) / block_size` is 64-bit safe.

---

## Scenario 3: Floating-Point Bit-Grooming & Precision Quantization

### Purpose
Verify scientific floating-point array with $\text{NSD} = 3$ achieves $> 500\%$ compression ratio boost when combined with BitShuffle + Deflate.

### Command
```bash
swift test --filter Blosc2BitGroomingTests
```

### Expected Output
```text
Test Case '-[TTZipTests.Blosc2BitGroomingTests testBitGroomingAccuracyAndCompressionSynergy]' passed (0.003 seconds).
```

### Failure Diagnostic
- Check that mantissa mask correctly preserves $\lceil 3.321928 \times \text{NSD} \rceil + 1$ bits for Float32.
