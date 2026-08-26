# Feature Specification: Codebase Algorithmic Optimization and Algebraic Kernels

**Feature Branch**: `089-codebase-algorithmic-optimization`
**Created**: 2026-08-18
**Status**: Draft
**Input**: User description: "Analyze algebraic unrolling, mathematical identity simplification, ILP acceleration, and zero-branching optimizations across TTZip codebase inspired by the Adler32 scalar chunk architecture"

## Clarifications

### Session 2026-08-18
- Q: What specific kernel subsystems are within scope for algebraic and scalar unrolling optimization? → A: Adler32/CRC64 scalar remainders, TAR 512-byte header parsing/checksums, and 7Z variable-length integer bit decoding in `CTTZipBridge`. Frozen ZIP engines remain strictly untouched.
- Q: What are the primary mathematical criteria for scalar loop unrolling? → A: Eliminating loop-carried dependency chains, minimizing modulus/division operations via bounded chunk delays, and utilizing branchless bitwise operations for multi-byte word processing.
- Q: How is numerical correctness validated across diverse memory alignments? → A: Via deterministic property-based differential oracles comparing against standard RFC/POSIX definitions across 0 to 64MB lengths and arbitrary pointer offsets.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Accelerated Small-Buffer and Tail Checksum Computation (Priority: P1)

Users and background archive workers frequently calculate integrity checksums (Adler32, CRC32, CRC64) on varying chunk sizes, small files, unaligned buffer slices, and tail bytes where SIMD vectors cannot fill a full vector register. Users require maximum physical throughput and minimal CPU cycle consumption across all buffer lengths without pipeline stalls.

**Why this priority**: Checksums are on the critical path of every compression and decompression pipeline (ZIP, GZ, ZSTD, 7Z, TAR). Tail and scalar path latency directly determines small-file archiving performance.

**Independent Test**: Can be tested independently via checksum microbenchmarks across buffer lengths from 1 byte to 64 MB with unaligned memory offsets, verifying 100% bit-for-bit equivalence with golden RFC/standard test vectors and measurable throughput gains on scalar/tail paths.

**Acceptance Scenarios**:
1. **Given** an arbitrary unaligned buffer of size between 1 and 63 bytes, **When** computing Adler-32 or CRC-64 checksums, **Then** the result must match the standard definition exactly with zero branch misprediction and at least 30% lower CPU instruction latency compared to byte-by-byte loops.
2. **Given** large buffers (> 64 KB) with non-multiple tail bytes, **When** calculating checksums in streaming mode, **Then** scalar tail processing must transition seamlessly from SIMD blocks without redundant state copies or pipeline stalls.

---

### User Story 2 - Zero-Overhead Archive Header and Number Parsing (Priority: P1)

When opening, indexing, or extracting large archives with hundreds of thousands of entries (e.g., TAR, 7Z, ZIP), metadata parsing (octal integers, variable-length integers, header checksums) often dominates the initial inspection latency. Users require instantaneous archive tree loading and header decoding.

**Why this priority**: Archive opening responsiveness and directory scanning speed define the user experience for archive exploration and batch operations.

**Independent Test**: Can be tested independently by benchmarking TAR header field parsing (octal conversions and 512-byte header checksum verification) and 7Z variable-length integer decoding over synthetic and real-world archives containing 50,000+ files.

**Acceptance Scenarios**:
1. **Given** a 512-byte standard TAR header, **When** validating the header checksum and extracting octal file sizes and timestamps, **Then** the header validation and integer conversions must complete without per-byte branch iteration, reducing total header decode time by at least 40%.
2. **Given** arbitrary 7Z variable-length encoded integers (`UInt64`), **When** decoding fields from the header bitstream, **Then** parsing must execute via branchless leading-bit extraction with zero redundant conditional jumps.

---

### User Story 3 - High-Efficiency Core Transformation and Filter Kernels (Priority: P2)

When compressing and decompressing executable binaries, instruction displacement filters (such as ARM64/x86 BCJ branch converters) and range coder bit models process millions of symbols sequentially. Users require branchless, vectorized transformations that maximize CPU instruction-level parallelism.

**Why this priority**: Pre-filtering and bitstream modeling represent compute-intensive stages in LZMA2/7Z pipelines; eliminating control hazards yields direct end-to-end throughput gains.

**Independent Test**: Can be tested independently by running BCJ filtering and range coder decoding on Silesia corpus executable binaries (`x86`, `arm64`) and verifying exact byte reversibility and higher processing rates.

**Acceptance Scenarios**:
1. **Given** executable machine code input, **When** applying branch conversion filters, **Then** address displacement transformations must operate using branchless arithmetic masks and aligned word transfers.
2. **Given** compressed bitstreams with entropy codes, **When** decoding range-coded state, **Then** probability updates and state normalization must minimize sequential data hazards.

---

### User Story 4 - Cross-Platform Algebraic Equivalence & Formal Zero-Regression (Priority: P3)

Developers and automated CI pipelines require all mathematical and algebraic kernel optimizations to be provably sound, formally bounded against integer overflow, and verified across all supported hardware targets (Apple Silicon ARM64 and Intel x86_64).

**Why this priority**: Absolute data integrity is the primary requirement for any archive and compression tool. Performance optimizations must never compromise byte accuracy under any alignment or boundary condition.

**Independent Test**: Can be tested independently by running automated property-based fuzzing and standard differential oracles comparing optimized kernels against reference implementations on $10^7$ pseudo-random permutations.

**Acceptance Scenarios**:
1. **Given** edge-case buffer slices (0 bytes, 1 byte, prime lengths, boundary-aligned, cross-page boundaries), **When** evaluated under property-based testing, **Then** zero assertion failures, zero memory violations, and 100% golden oracle equivalence are observed.

---

## Edge Cases

- **Zero-length and NULL buffers**: Kernels must immediately return base initial states without performing unaligned pointer arithmetic or out-of-bounds reads.
- **Buffers smaller than the unroll chunk size**: Inputs with $0 < n < \text{chunk\_size}$ must gracefully fall through to exact scalar remainder loops without underflow or buffer overrun.
- **Unaligned memory addresses**: Pointers not aligned to 4, 8, or 16-byte boundaries must be handled correctly without causing unaligned trap faults or performance penalties on strict architectures.
- **Arithmetic overflow near modulo boundaries**: Deferred modulo accumulators must guarantee that worst-case inputs ($255 \times N$) cannot exceed register width ($2^{32}-1$ or $2^{64}-1$) before reduction.
- **Corrupted metadata and malformed headers**: Header parsers must validate character sets and field limits, rejecting non-conformant inputs without infinite loops or buffer over-reads.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide algebraic scalar unrolling and ILP-optimized kernels for checksum calculations (Adler32, CRC32, CRC64) across all buffer lengths.
- **FR-002**: System MUST defer mathematical modulo operations in checksum accumulation loops up to the provable mathematical threshold without 32-bit/64-bit overflow.
- **FR-003**: System MUST execute TAR header checksum computation and octal field conversions using multi-byte word operations and branchless arithmetic.
- **FR-004**: System MUST decode 7Z variable-length encoded integers using branchless bitwise operations and hardware leading-zero count primitives.
- **FR-005**: System MUST maintain full bit-for-bit backward and forward compatibility with standard archive format specifications (RFC 1950, POSIX 1003.1 ustar, 7z Format Specification).
- **FR-006**: System MUST preserve all existing hardware SIMD acceleration fast-paths (Apple Silicon NEON DotProd / PMULL) while upgrading scalar fallbacks and remainder handlers.
- **FR-007**: System MUST satisfy zero-heap-allocation on all hot-path kernel executions, utilizing exclusively registers, stack storage, or pre-allocated caller buffers.
- **FR-008**: System MUST pass 100% of existing unit tests, performance regression gates, and golden corpus differential validation tests.

---

### Key Entities

- **Checksum Computation Engine**: Manages running state, SIMD dispatch, and algebraic scalar fallback accumulators for Adler32, CRC32, and CRC64.
- **Archive Header Parser**: Decodes raw metadata structures (TAR 512-byte records, 7Z folder/stream headers, ZIP central directory records) into strongly-typed archive entry representations.
- **Bitstream Transformation Filter**: Applies reversible transformations (BCJ architecture filters, delta encodings) and entropy decoding operations over byte streams.
- **Verification Oracle**: Reference validation harness providing differential verification against system standard tools and official test vectors.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Checksum calculation throughput on small buffers ($\le 4\text{KB}$) and tail remainder bytes improves by at least 25% over naive byte-by-byte loops.
- **SC-002**: TAR header verification and metadata parsing speed increases by at least 35% across 50,000-entry archive benchmark suites.
- **SC-003**: 100% bit-exact equivalence maintained across all test vectors, fuzz corpora, and golden archive test suites with zero data discrepancy.
- **SC-004**: Zero performance regressions across all 16 supported archive formats and zero drop below established constitution performance floors.
- **SC-005**: 100% passing rate across full test suite (`swift test`, 525+ tests) with clean build (zero warnings).

---

## Assumptions

- Target operating environment is macOS 14.0+ running on 64-bit architectures (Apple Silicon ARM64 and Intel x86_64).
- Modern C compilers (Clang 15+) support standard bitwise intrinsics (`__builtin_clzll`, `__builtin_ctzll`, vector extensions) and auto-vectorization hints.
- System memory pages are safely accessible within the bounds of allocated buffers, and unaligned word loads are safe on target hardware architectures.
- Existing CTTZipBridge architecture and Swift-to-C interop conventions remain the standard integration layer.
