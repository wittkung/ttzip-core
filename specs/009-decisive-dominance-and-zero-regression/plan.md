# Implementation Plan: Decisive Dominance & Zero Regression (009)

## Proposed Changes

### Phase 0: Research & Benchmarking
- Profile 500MB L1 LZMA2 fast encoder memory allocations.
- Analyze TAR.ZST direct extraction vs libarchive.

### Phase 1: Contracts & Architecture
- Maintain C bridge zero-cost abstraction contracts.
- Define multi-threaded TAR.ZST extraction pipeline.

### Phase 2: Implementation Tasks
1. **7Z 500MB L1 No-Encryption Speedup**:
   - In `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`, tune HC3 hash chain probing for large zero streams to ensure $>= 5,500\text{ MB/s}$ throughput.
2. **TAR.ZST In-Process Direct Decompression**:
   - Implement direct `ZSTD_decompressStream` loop with zero-copy tar header parsing in `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`.
3. **High-Entropy Fast Strategy**:
   - Ensure high entropy streams bypass deep match finding and achieve $>= 6,000\text{ MB/s}$ in TAR.ZST and 7Z.
4. **Regression Audit & Verification**:
   - Run `AllFormatsPkSuiteTests`, assert zero regressions $> 10\%$, and verify 11 performance gates.
