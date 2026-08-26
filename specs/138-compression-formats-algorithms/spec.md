# Feature Specification: Full Compression Formats and Algorithms Analysis

**Feature Branch**: `138-compression-formats-algorithms`  
**Created**: 2026-08-20  
**Status**: Draft  
**Input**: User description: "全面分析我们支持的所有压缩格式，底层涉及到的所有压缩算法 /speckit-specify"  

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Full-Matrix Format & Algorithm Architecture Exploration (Priority: P1)

As an enterprise engineer, software architect, or power user inspecting TTZip's archiving core, I want to access an exhaustive, mathematically grounded, and system-level taxonomy of all supported archive container formats and their underlying compression algorithms, so that I can make optimal decisions regarding compression ratio, throughput ceilings, memory bounds, and algorithmic compatibility across macOS and cross-platform ecosystems.

**Why this priority**:
Provides the fundamental technical truth and architectural map for all 16+ archive formats, ensuring clear operational boundaries between container encapsulation, stream framing, match-finding heuristics, entropy coding, and hardware acceleration vectors.

**Independent Test**:
Can be fully verified by executing format-to-algorithm matrix queries and verifying that every supported archive format maps to its precise underlying compression engines, encoding mathematics, sliding window configurations, and execution constraints without ambiguity.

**Acceptance Scenarios**:
1. **Given** any of the 16 primary container formats (ZIP, 7Z, TAR, TAR.GZ, TAR.ZST, TAR.BZ2, TAR.XZ, LZIP, LZ4, BROTLI, LRZIP, AAR, SNAPPY, WIM, DMG, ISO) or auxiliary extraction formats (RAR, CAB, CPIO, XAR), **When** an architect requests the format's structural blueprint, **Then** the system outputs the complete container framing model, stream specification, and header semantics.
2. **Given** any underlying compression algorithm (Deflate, LZMA/LZMA2, Zstandard, BZIP2/BWT, LZ4, Brotli, LZFSE, Snappy, LRZIP, LZX, XPRESS, PPMd, BCJ/BCJ2, Delta/Shuffle), **When** the theoretical foundations are inspected, **Then** the exact mathematical principles (match-finding heuristics, dictionary mechanics, entropy coding models like Huffman, tANS/FSE, Range Coder, and context modeling) are systematically articulated.
3. **Given** an Apple Silicon hardware environment, **When** examining inner-loop execution, **Then** the hardware acceleration mapping (ARM64 PMULL, NEON SIMD, AES extensions, cache topology tuning) is precisely documented.

---

### User Story 2 - Algorithmic Trade-off & Pareto Frontier Navigation (Priority: P2)

As a DevOps engineer or performance specialist selecting compression profiles for different workloads (massive small files, high-entropy binaries, continuous log streams, and massive multi-gigabyte packages), I want to analyze the Pareto trade-offs across compression speed, decompression speed, space savings ratio, memory consumption bounds, and streaming friendliness.

**Why this priority**:
Enables users and client systems to dynamically choose the optimal compression algorithm and tier level based on concrete empirical characteristics rather than guesswork.

**Independent Test**:
Can be verified by cross-referencing workload categories with algorithmic characteristics to produce unambiguous profile recommendations and resource limits.

**Acceptance Scenarios**:
1. **Given** a specific workload category (e.g., streaming I/O with ultra-low latency vs. archive storage requiring maximum density), **When** evaluating candidate algorithms, **Then** the system presents explicit throughput, ratio, and memory occupancy trade-off curves.
2. **Given** strict resource constraints (e.g., fixed $\le 64\text{MB}$ heap envelope or single-pass non-seekable pipes), **When** filtering algorithms, **Then** compatible algorithms (e.g., streaming Deflate/Zstandard) are separated from high-memory or multi-pass algorithms (e.g., solid LZMA2 or LRZIP rzip passes).

---

### User Story 3 - Cryptographic & Data Integrity Invariant Analysis (Priority: P3)

As a security auditor or compliance officer, I want to inspect the data integrity verification algorithms and cryptographic encryption primitives associated with each format (AES-256-GCM/CTR/CBC, ZipCrypto, header encryption, hardware PMULL CRC64, CRC32, Adler32, xxHash64, BLAKE2sp), so that I can verify cryptographic hardening, memory sanitization (`explicit_bzero`), and stream validation correctness.

**Why this priority**:
Ensures strict adherence to security invariants, sandboxing rules, and tamper-resistance standards across all encrypted archive types.

**Independent Test**:
Can be tested by auditing cryptographic schemes and checksum algorithms across formats to verify cipher modes, key derivation functions (PBKDF2, Argon2), and hardware acceleration paths.

**Acceptance Scenarios**:
1. **Given** an encrypted archive format (7Z AES-256, ZIP AES-256 WinZip format, WIM, DMG encrypted UDIF), **When** analyzing cryptographic pipelines, **Then** the key derivation, initialization vector handling, authentication tags, and zeroization mechanisms are comprehensively documented.
2. **Given** an integrity verification stage, **When** assessing checksum throughput, **Then** the algorithm (ARM64 PMULL CRC64/CRC32, Adler32 NEON, xxHash64) and its mathematical invariance are clearly specified.

---

### Edge Cases

- **Non-Seekable Pipe Streaming**: How algorithms behave when standard seek operations (`lseek` / `fseek`) are impossible (e.g., standard input/output streaming in TAR, GZ, ZST vs. random-access requirements in ZIP Central Directory or 7Z Header).
- **Solid vs. Non-Solid Archiving**: Impact on random entry extraction latency and error propagation when individual blocks are compressed collectively (7Z/TAR.XZ) versus independently (ZIP).
- **High-Entropy / Incompressible Data Handling**: Algorithmic detection mechanisms for uncompressible data (random/encrypted bytes) and zero-overhead fallback to Store mode without expanding archive footprint.
- **Sparse File & Extended Attribute Preservation**: Handling of APFS resource forks, Extended Attributes (xattr), Access Control Lists (ACLs), and sparse blocks across container formats (AAR, TAR PAX, ZIP extra fields).

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The analysis MUST categorize all 16 primary supported container formats (ZIP, 7Z, TAR, TAR.GZ, TAR.ZST, TAR.BZ2, TAR.XZ, LZIP, LZ4, BROTLI, LRZIP, AAR, SNAPPY, WIM, DMG, ISO) and auxiliary formats (RAR, CAB, CPIO, XAR), detailing their container framing, header structures, and metadata capabilities.
- **FR-002**: The analysis MUST delineate the 14 core compression and transform algorithms utilized across the engine:
  1. *Deflate (RFC 1951)* (Sliding Window LZ77 + Dynamic/Static Canonical Huffman Trees)
  2. *LZMA / LZMA2* (Dictionary LZ77 + Binary Range Coder + Markov Chain State Machine)
  3. *Zstandard (RFC 8878)* (Finite State Entropy / tANS + Hash/BT Matcher + Huffman + LDM)
  4. *BZIP2 / BWT* (Burrows-Wheeler Block Sorting + Move-To-Front + RLE + Huffman)
  5. *LZ4 / LZ4-HC* (Byte-oriented token stream + fast hash matching + raw literal runs)
  6. *Brotli (RFC 7932)* (2nd-Order Context Modeling + LZ77 + Huffman + 120KB Static Dictionary)
  7. *LZFSE* (Apple Lempel-Ziv + 2-State Finite State Entropy / tANS)
  8. *Snappy* (Google high-throughput byte-aligned LZ77, zero entropy overhead)
  9. *LRZIP* (rzip 1st-stage multi-gigabyte hash tree search + 2nd-stage backend engine)
  10. *LZX & XPRESS* (Microsoft sliding-window LZ77 + Huffman for WIM/CAB)
  11. *PPMd (Order-$k$ PPM)* (Statistical context modeling + Range Coder for text/source)
  12. *RAR Engine Family* (RAR 1.5–5.0 proprietary LZ77 + Huffman + Multimedia/Executable Delta)
  13. *Instruction / Byte Preprocessing Filters* (BCJ, BCJ2, Byte-Shuffle, Bit-Grooming, Delta)
  14. *Store Mode* (Zero-compression pass-through with APFS zero-copy file cloning)
- **FR-003**: The analysis MUST systematically explain the mathematical principles of each algorithm, covering match finding (hash chains, binary trees, SWAR, hybrid SIMD), sliding window dynamics, and entropy coding (Huffman, Range Coding, ANS / FSE, Context Mixing).
- **FR-004**: The analysis MUST map each format to its underlying physical C/Assembly engine implementation (`libdeflate`, `zlib-ng`, `LZMA SDK`, `fast-lzma2`, `libzstd`, `liblz4`, `brotli`, `liblzfse`, `snappy`, `libarchive`, Apple Archive APIs).
- **FR-005**: The analysis MUST document the hardware acceleration architecture on Apple Silicon (ARM64 PMULL CRC64/CRC32, NEON SIMD match counting, AES hardware vector cryptography, CPU topology-aware thread dispatch).
- **FR-006**: The analysis MUST establish a multi-dimensional comparison matrix evaluating: Compression Ratio, Compression Throughput, Decompression Throughput, Memory Bounds during Execution, Streaming Support, Random Access Capability, and Security/Encryption Features.
- **FR-007**: The analysis MUST provide clear architectural recommendations matching real-world workloads to optimal formats and algorithms.

---

### Key Entities *(include if feature involves data)*

- **`ArchiveContainerFormat`**: Represents the outer framing specification, packaging metadata, entry headers, central directories, and stream layout (e.g., ZIP, 7Z, TAR).
- **`CompressionAlgorithm`**: Represents the underlying mathematical data transformation and entropy reduction engine (e.g., Deflate, LZMA2, Zstandard, BWT).
- **`MatchFinderModel`**: Represents the pattern detection algorithm operating over the sliding window (e.g., Hash Chain, Binary Tree, 64-bit SWAR, 128-bit NEON vector unrolling).
- **`EntropyCoderModel`**: Represents the statistical symbol coding model transforming literal/match tokens into bitstreams (e.g., Canonical Huffman, Finite State Entropy / tANS, Range Coder).
- **`HardwareAccelerationKernel`**: Represents the low-level micro-architecture SIMD / vector assembly routine optimizing hot paths on Apple Silicon.
- **`SecurityAndIntegrityScheme`**: Represents the checksum verification algorithms (CRC32, CRC64 PMULL, xxHash64) and cryptographic encryption ciphers (AES-256, ZipCrypto).

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of all archive container formats supported by TTZip (16 primary + 4 auxiliary) are thoroughly cataloged with their exact structural specifications and engine bindings.
- **SC-002**: 100% of underlying compression, pre-filtering, and entropy coding algorithms are mathematically articulated with clear algorithmic steps and complexity bounds.
- **SC-003**: A complete 2-dimensional matrix is generated correlating every archive format with its allowed compression algorithms, supported compression levels (-5 to 22), encryption ciphers, and streaming characteristics.
- **SC-004**: Hardware acceleration pathways on Apple Silicon (NEON SIMD, PMULL, AES-NI/ARMv8 Crypto) are mapped with zero ambiguity to their corresponding algorithmic hot paths.
- **SC-005**: Clear operational guidance is established for 4 distinct industrial workload profiles (Massive Small Files, Structured Log Text, High-Entropy Binary, Large Contiguous Blocks).

---

## Assumptions

- **Target Architecture**: macOS 14.0+ running on Apple Silicon (ARM64 / ARMv8.4-A+) with backward compatibility for Intel x86_64 architectures.
- **Zero Subprocess Policy**: All analyzed formats and algorithms operate in-process via static C11 ABI libraries and direct Swift 6 bindings without invoking external command-line binaries.
- **System Memory Bounds**: Streaming operations adhere to the $\le 64\text{MB} \sim 128\text{MB}$ memory footprint invariant per task, while block-parallel modes scale dynamically with available unified memory and core count.
