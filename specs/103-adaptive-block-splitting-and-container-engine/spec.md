# Feature Specification: Adaptive 300KB/5KB Block Splitting & Zero-Overhead Container Streaming Engine

**Feature Branch / Spec Directory**: `specs/103-adaptive-block-splitting-and-container-engine`  
**Created**: 2026-08-19  
**Status**: Draft  

---

## 1. Executive Summary & Problem Statement

In Deflate-based archives (ZIP, GZIP, TAR.GZ, ZLIB), data blocks can be encoded as Dynamic Huffman blocks, Static Huffman blocks, or Store (uncompressed) blocks. Traditional compressors either use a rigid block size (e.g. 64KB/128KB) or trigger expensive block splits.
`libdeflate` solves this through:
1. **Adaptive 300KB Soft / 5KB Hard Dual-Threshold Block Splitting**:
   - `SOFT_MAX_BLOCK_LENGTH = 300,000` bytes allows the local Huffman statistical model to converge on deep patterns without memory explosion.
   - `MIN_BLOCK_LENGTH = 5,000` bytes prevents micro-block fragmentation from diluting tree header overhead.
   - Chi-square entropy shift monitoring detects content transitions and triggers match-cache rewinds to achieve optimal homogeneity within blocks.
2. **Zero-Overhead RFC 1950 (ZLIB) & RFC 1952 (GZIP) Container Fast-Path**:
   - RFC 1950 (2-byte header, trailing Adler-32) and RFC 1952 (10-byte header, trailing CRC-32 + ISIZE) can be serialized with direct unaligned big-endian/little-endian integer stores, bypassing heavyweight wrapper objects and redundant checksum state machines.

This specification defines the C11 native implementation, Swift architecture integration, and differential verification of adaptive block splitting and zero-overhead container serialization.

---

## 2. User Scenarios & Key Workflows

### User Scenario 1: Heterogeneous Multi-Content Stream Compression
- **Actor**: Streaming compressor processing mixed files (e.g. concatenated text + binary images + JSON).
- **Workflow**: The adaptive block splitter evaluates local entropy transitions. When entering a high-entropy image segment, it dynamically switches to Store blocks; when entering structured JSON, it expands up to 300KB Dynamic Huffman blocks, achieving 2-5% size reduction over fixed-size chunking.

### User Scenario 2: High-Throughput Zero-Copy GZIP & ZLIB Container Serialization
- **Actor**: Archive writer or stream coder packaging single-file GZIP (`.gz`) or ZLIB (`.zz`) streams.
- **Workflow**: Directly emits the 10-byte RFC 1952 header, streams the raw Deflate payload, and writes the 4-byte CRC-32 and 4-byte ISIZE via `put_unaligned_le32` in a single pass with zero redundant buffering.

---

## 3. Functional Requirements

- **FR-001 [Adaptive Block Splitter]**: Implement `ttzip_adaptive_block_split` in C11 (`Sources/CTTZipBridge/ttzip_adaptive_block_split.c` / `include/ttzip_adaptive_block_split.h`) with `SOFT_MAX_BLOCK_LENGTH = 300000` and `MIN_BLOCK_LENGTH = 5000`.
- **FR-002 [Three-Way Block Type Race]**: For each partitioned block, evaluate total bit cost of Dynamic Huffman vs Static Huffman vs Uncompressed Store and output the minimal bitstream.
- **FR-003 [Zero-Overhead GZIP Container Engine]**: Implement `ttzip_gzip_compress_fast` and `ttzip_gzip_decompress_fast` in C11 (`Sources/CTTZipBridge/ttzip_container_fast.c` / `include/ttzip_container_fast.h`) supporting OS flags, timestamp passthrough, and fused CRC32 + ISIZE injection.
- **FR-004 [Zero-Overhead ZLIB Container Engine]**: Implement `ttzip_zlib_compress_fast` and `ttzip_zlib_decompress_fast` supporting RFC 1950 CMF/FLG validation, FDICT handling, and fused Adler-32 injection.
- **FR-005 [Swift Architecture Bridging]**: Provide Swift adapter `ContainerFastEngine.swift` in `Sources/TTZipCore/Adapters/`.
- **FR-006 [Standard Consensus Validation]**: All GZIP and ZLIB outputs must pass 100% verification against Apple `libcompression`, macOS `gzip -d`, and `zlib`.

---

## 4. Success Criteria & Hard Performance Floors

| ID | Metric | Target Floor | Verification Method |
| :--- | :--- | :--- | :--- |
| **SC-001** | GZIP Fast Container Overhead | $\le 10\text{ ns}$ header/trailer serialization penalty | Microbenchmark vs raw Deflate |
| **SC-002** | Mixed-Content Compression Gain | $\ge 2.5\%$ size saving vs fixed 64KB chunking on mixed corpus | Differential benchmark |
| **SC-003** | GZIP / ZLIB Throughput Floor | $\ge 1500\text{ MB/s}$ (Debug) / $\ge 2000\text{ MB/s}$ (Release) | 10MB payload throughput measure |
| **SC-004** | Full Matrix Performance Invariance | 0 regression across all 13 standard floors | `swift test --filter XCTestPerformanceMeasureTests` |
