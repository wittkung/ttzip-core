### Summary
Expand `tests/decompress-partial.c` test coverage to exhaustively verify all prefix lengths from 1 byte up to full uncompressed size (`srcLen`), ensuring returned decompressed lengths satisfy `result >= target` and decoded bytes are bit-identical without regression.

---

### Motivation & Testing Gap

`LZ4_decompress_safe_partial()` is a critical API widely utilized in performance- and latency-sensitive downstream systems—such as the Linux kernel (EROFS filesystem), ClickHouse, and RocksDB—to probe short metadata headers without decompressing full blocks.

#### 1. First-Principles: The Non-Linear Token Progression
An LZ4 compressed block progresses through discrete, non-uniform sequences containing variable literal runs, match offsets, and match lengths:

```text
[ Token (1B) ] -> [ Literal Length Ext ] -> [ Literal Payload ] -> [ Match Offset (2B) ] -> [ Match Length Ext ]
```

When a caller requests an arbitrary `targetOutputSize`, decompression legitimately truncates across four distinct micro-architectural boundaries:
- **Mid-Literal Boundary**: Output target satisfied during literal copy; trailing match offsets must not be probed.
- **Mid-Match Boundary**: Output target satisfied halfway through overlapping or dictionary match copy.
- **Wildcopy Overshoot Boundary**: Bounded writes within `dstCapacity` safely containing wildcopy overshoot without corrupting destination bounds.
- **Shortcut vs. Safe-Loop Boundary**: Transitioning across `op <= shortoend` (the 32-byte safe shortcut window) and general safe decode paths.

#### 2. The Testing Gap in Existing Suite
In the current upstream `tests/decompress-partial.c`:
```c
/* Existing test in tests/decompress-partial.c */
for (i = cmpSize; i < cmpSize + 10; ++i) {
    int result = LZ4_decompress_safe_partial(cmpBuffer, outBuffer, i, srcLen, BUFFER_SIZE);
    if ((result < 0) || (result != srcLen) || memcmp(source, outBuffer, srcLen)) {
        return -1;
    }
}
```
- **The Blindspot**: The existing loop only validates `targetOutputSize == srcLen` (full uncompressed length).
- **Consequence**: When `target == srcLen`, execution runs through all sequences to the end of the block, effectively degrading `LZ4_decompress_safe_partial()` into a standard `LZ4_decompress_safe()` pass. It leaves the actual mid-sequence and mid-match early truncation logic branches completely unexercised.

---

### Proposed Changes

Enhance `tests/decompress-partial.c`:
1. **Exhaustive Prefix Sweep**: Loop over every prefix length from 1 byte to full uncompressed size (`1 <= i <= srcLen`).
2. **Buffer Capacity Decoupling**: Pass `BUFFER_SIZE` (2048 bytes) as `dstCapacity`, verifying that the function safely truncates decompression at `targetOutputSize = i` without overflowing or requiring an exact-sized destination buffer.
3. **Triple Invariant Verification**:
   - **Monotonicity**: Assert that `result >= i` (at least the requested target length is produced).
   - **Memory Bounds**: Assert `result <= BUFFER_SIZE` without exceeding buffer boundaries.
   - **Bit-Identical Precision**: Assert that `memcmp(source, outBuffer, (size_t)i) == 0` (decoded prefix matches original uncompressed content byte-for-byte).
4. **Test Isolation**: Zero memory (`memset(outBuffer, 0, sizeof(outBuffer))`) before each iteration to ensure complete test isolation across loop passes.
5. **C90 Strict Conformance**: Adhere strictly to C90 syntax with top-of-block variable declarations and `/* ... */` comment formatting.

---

### Verification
- `make -C tests decompress-partial && ./tests/decompress-partial`: Passed (`test decompress-partial OK`).
- `make -C tests test-lz4-basic`: Passed (0 warnings under strict `-Wc++-compat` and ANSI C89 flags).
- `make -C tests test-frametest`: Passed (83,870 fuzz iterations).

---

### Acknowledgements
> Thank you, Yann Collet (@Cyan4973) and the LZ4 maintainers, for your continuous dedication to high-performance compression, safety, and robust engineering!
