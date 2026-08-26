# Phase 0 Research: SIMD Canonical Huffman Coding & Multi-Symbol Emission

**Feature**: `122-simd-canonical-huffman-multi-symbol-emission`
**Created**: 2026-08-19

---

## Research Items

### R001 [SUBAGENT:research] 64-Bit Multi-Symbol Bitstream Word Packing Architecture
- **Decision**: Implement `ttzip_bs_write_bits64` allowing up to 56 bits in a single 64-bit store, and pack all 4 components of a Deflate match token ($len\_code + len\_extra + dist\_code + dist\_extra \le 48\text{ bits}$) into a single operation.
- **Rationale**:
  - In RFC 1951 Deflate, length codewords are at most 15 bits, length extra bits are at most 5 bits, distance codewords are at most 15 bits, distance extra bits are at most 13 bits ($15+5+15+13=48$ bits).
  - Executing 4 separate calls to `ttzip_bs_write_bits` per match token incurs 4 branch checks, 4 shifts, and 4 buffer pointer advances.
  - By pre-packing into a single `uint64_t`, 1 match token is emitted in a single branchless operation, eliminating 75% of function calls in the token serialization loop.
- **Alternatives Considered**:
  - *Scalar 32-bit loop*: Requires 4 independent updates per match token, saturating CPU instruction decoder on long files.
  - *Lookup-table pre-baked 64-bit structs*: Requires a 256KB table to pre-bake all combinations of distance and length, polluting the L1 Data Cache.
- **Source**:
  - `Vendor/libdeflate-upstream/lib/deflate_compress.c:450-520`
  - `Sources/CTTZipBridge/native_deflate/ttzip_deflate_bitstream.h:60-90`

---

### R002 [SUBAGENT:research] Small-File Static Huffman Threshold & Fast Dynamic Tree Bypass
- **Decision**: On files $< 4\text{KB}$ (or chunks where `num_tokens < 512`), evaluate static vs. dynamic Huffman bit cost. If static Huffman bit cost is within 3% of dynamic Huffman, directly use precomputed static Huffman codes.
- **Rationale**:
  - Building a dynamic Huffman tree involves computing frequencies, 2-queue in-place length limiting, precode run-length encoding, and emitting a ~300-bit dynamic header.
  - On a 1KB~2KB file, this header alone adds 30~50 bytes of overhead, negating any compression savings from dynamic codewords.
  - Static Huffman uses compile-time constant tables (`s_static_codes`), skipping tree construction and header generation in $0.0\mu s$.
- **Alternatives Considered**:
  - *Always Dynamic Huffman*: Adds 50~100 microseconds per small file, capping 500-file mixed workspace throughput at ~460 MB/s.
- **Source**:
  - `Sources/CTTZipBridge/native_deflate/ttzip_deflate_huffman.c:40-95`
  - `Tests/TTZipTests/CompoundMixedCorpusBenchmarkPkTests.swift:150-200`
