# Implementation Plan: Codebase Algorithmic Optimization and Algebraic Kernels

**Feature Branch**: `089-codebase-algorithmic-optimization`
**Created**: 2026-08-18
**Status**: Ready for Tasks
**Specification**: [spec.md](./spec.md)

---

## 1. Technical Context & Scope

The TTZip codebase contains several performance-critical scalar loops, checksum remainder handlers, bitstream parsers, and metadata decoders in `Sources/CTTZipBridge/` and `Sources/TTZipCore/`. While large continuous buffers benefit from ARM64 NEON SIMD routines (e.g. `ttzip_adler32_neon_dotprod`, `ttzip_crc64_pmull`), scalar fallbacks, remainder bytes, and metadata parsing (such as TAR octal conversion and 7Z variable-length integer decoding) frequently process uneven chunk sizes.

Inspired by the mathematical elegance of `TTZIP_ADLER32_SCALAR_CHUNK` in `Sources/CTTZipBridge/CTTZipAdler32Neon.c` (which uses 4-way algebraic unrolling, deferred modulo arithmetic, and independent accumulator trees), this feature systematically applies:
1. **Algebraic Scalar Unrolling & Math Proofs**: Formally verifying and standardizing the 4-way unrolled Adler-32 scalar chunk with $N_{\max} = 5552$ bytes threshold.
2. **TAR SWAR Octal & Checksum Parsing**: Replacing libc `sscanf("%o")` and scalar 512-byte loops with 3-level binary SWAR octal decoding and NEON/SWAR dual signed/unsigned header checksum calculation with $O(1)$ linear field adjustment.
3. **7Z Variable-Length Integer Branchless Decoding**: Replacing double while/for loops with `__builtin_clz` leading-ones detection, unaligned 64-bit load, and shift clamping to eliminate control hazards and fix latent shift-by-64 Undefined Behavior.
4. **CRC64 / Checksum Remainder Permutations**: Standardizing overlapping vector load tail permutations and ACLE hardware instructions.

---

## 2. Constitution Check

| Principle | Assessment | Compliance Notes |
| :--- | :--- | :--- |
| **Zero-Cost Abstraction on Hot Paths** | **COMPLIANT** | Zero heap allocations (`malloc`/`free`), zero locks, pure register/stack computation. |
| **Fast-Path Bypass Preservation** | **COMPLIANT** | Retains Apple Silicon NEON DotProd/PMULL fast-paths; optimizes scalar fallbacks. |
| **Hard Throughput Floors** | **COMPLIANT** | Must pass `XCTestPerformanceMeasureTests` and all 13 performance gates. |
| **Subsystem Freeze Discipline** | **COMPLIANT** | Frozen ZIP engines (`ZipParallelExtractor.swift`, etc.) remain 100% untouched. |
| **SPDX Copyright & Documentation** | **COMPLIANT** | All modified C and Swift files retain full SPDX copyright and doc headers. |
| **No Bare Logging in C Bridge** | **COMPLIANT** | Zero `printf`, `fprintf`, or `NSLog` introduced. |

---

## 3. Phase 0: Research Items

- R001 [SUBAGENT:research] 《Adler-32 Scalar Chunk Mathematical Expansion & Proof》: Formally proved $N_{\max} = 5552$ bytes boundary and 4-way GPR unrolling efficiency vs. SWAR in `research.md`.
- R002 [SUBAGENT:research] 《TAR 512-Byte Header SWAR Octal & Checksum Parsing》: Formulated 3-level SWAR bit-packing, GNU base-256 binary fast-path, and NEON `vpadalq` dual checksum calculation in `research.md`.
- R003 [SUBAGENT:research] 《7Z Variable-Length Integer Branchless Decoding via CLZ》: Formulated `__builtin_clz` leading-ones extraction, 64-bit unaligned load, and UB-free shift clamping in `research.md`.
- R004 [SUBAGENT:research] 《CRC64 / CRC32 Checksum Remainder Permutations & Alignment》: Established vector-folding tail permutations and zero-copy unaligned load rules in `research.md`.

---

## 4. Phase 1: Design & Contracts

- **Data Model**: Defined in [data-model.md](./data-model.md) covering `TTZipTarHeaderEntryInfo`, `TTZipVarintDecodeResult`, `TTZipAdler32ChunkState`, and `TTZipKernelVerificationReport`.
- **Contracts**: Defined in [contracts/kernel-optimization-schema.json](./contracts/kernel-optimization-schema.json) adhering strictly to Draft-07 and zero bare object rules.
- **Validation Guide**: Defined in [quickstart.md](./quickstart.md) with 4 concrete test commands and failure diagnostics.

---

## 5. Component Change Matrix

| Component | Target File | Change Type | Description |
| :--- | :--- | :--- | :--- |
| **C Bridge Checksums** | `Sources/CTTZipBridge/CTTZipAdler32Neon.c` | Standardize | Maintain formal 4-way unrolling with documented mathematical proof and bounds comments. |
| **C Bridge 7Z Varint** | `Sources/CTTZipBridge/ttzip_7z_header_parser.c` | Modify | Implement branchless `ttzip_7z_read_varint_fast` with `__builtin_clz`, 64-bit load, and UB shift clamp. |
| **C Bridge TAR Headers** | `Sources/CTTZipBridge/ttzip_tar_native.c` | Modify | Implement `ttzip_octal_parse8_swar`, 512-byte zero block check, and NEON/SWAR dual checksum validation. |
| **C Bridge TAR Headers** | `Sources/CTTZipBridge/include/ttzip_tar_native.h` | Modify | Expose `ttzip_tar_header_parse_fast` and `ttzip_tar_entry_info` C structure. |
| **C Bridge Native Listing** | `Sources/CTTZipBridge/ttzip_native_archive.c` | Modify | Replace `sscanf("%o")` with `ttzip_octal_parse8_swar` and validate header checksums. |
| **Unit Tests** | `Tests/TTZipTests/HardwareChecksumTests.swift` | Modify | Add exhaustive property-based test cases for unaligned and slice boundaries. |
| **Unit Tests** | `Tests/TTZipTests/SevenZipHeaderParserTests.swift` | Modify / Verify | Validate 64-bit 9-byte varints and boundary conditions. |
