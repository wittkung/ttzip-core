# Implementation Plan: Full C11/SIMD Migration of 7 Core Engine Modules

**Feature ID**: `161-seven-modules-c-migration`  
**Date**: 2026-08-20  
**Spec**: [spec.md](./spec.md)  
**Research**: [research.md](./research.md)  
**Data Model**: [data-model.md](./data-model.md)  
**Quickstart**: [quickstart.md](./quickstart.md)  

---

## 1. Technical Context & Scope

The goal of this feature is migrating 7 core mathematical, parsing, and data-path algorithms from Swift to high-performance C11 with ARM64 NEON acceleration in `Sources/CTTZipBridge/`, updating the corresponding Swift facades in `Sources/TTZipCore/` to delegate to the new C primitives with zero breaking changes.

### The 7 Modules:
1. **Module 1**: `ReedSolomonFEC` ➔ `ttzip_reed_solomon.c` / `ttzip_reed_solomon.h` (GF(2^8) NEON vector lookup)
2. **Module 2**: `PathPatternFilterEngine` ➔ `ttzip_path_filter.c` / `ttzip_path_filter.h` (Zero-alloc pointer sliding & POSIX glob)
3. **Module 3**: `ZipExtraFieldParser` ➔ `ttzip_zip_extra_field.c` / `ttzip_zip_extra_field.h` (In-place stack TLV parsing)
4. **Module 4**: `SevenZipHeaderReader` ➔ `ttzip_7z_header_parser.c` (Consolidated signature & folder descriptor reading)
5. **Module 5**: `PasswordRecoveryEngine` Kernel ➔ `ttzip_password_verifier.c` / `ttzip_password_verifier.h` (In-memory multi-core parallel verification)
6. **Module 6**: `ArchiveSearchIndex` ➔ `ttzip_search_index.c` / `ttzip_search_index.h` (Flat columnar memory & NEON substring search)
7. **Module 7**: `NDimTensorLayout` ➔ `CTTZipTensorSlicing.c` / `CTTZipTensorSlicing.h` (Unrolled coordinate hypercube block solver)

---

## 2. Phase 0: Research Items

- - R001 [SUBAGENT:research] 《Reed-Solomon GF(2^8) & Cauchy Matrix C/NEON Acceleration》: Define Cauchy generator matrix in GF(2^8) and vectorize byte multiplication via ARM NEON `vqtbl1q_u8` nibble decomposition.
- - R002 [SUBAGENT:research] 《PathPatternFilter & Glob Matching Engine》: Implement zero-allocation pointer sliding for POSIX fnmatch and predefined OS junk filters.
- - R003 [SUBAGENT:research] 《ZipExtraFieldParser TLV Engine》: Implement in-place zero-allocation parser for Zip64, UT, up, ux, and WinZip AES.
- - R004 [SUBAGENT:research] 《SevenZipHeader & Signature Reader Consolidation》: Consolidate 32-byte 7z signature and descriptor parsing directly in C.
- - R005 [SUBAGENT:research] 《Fast In-Memory Password Verification Kernel》: Implement in-memory PVV and ZipCrypto header verification using multi-core parallel thread pool.
- - R006 [SUBAGENT:research] 《ArchiveSearchIndex Flat Columnar SIMD Filter》: Implement contiguous flat buffer layout with NEON substring vector scan.
- - R007 [SUBAGENT:research] 《NDimTensor Hypercube Geometry & Slicing Kernel》: Implement stack-based unrolled hypercube block intersection coordinate solver.

---

## 3. Phase 1: Contracts & Data Model

- [SUBAGENT:research] `contracts/reed_solomon_fec.json`: Systematic Cauchy Reed-Solomon validation schema.
- [SUBAGENT:research] `contracts/zip_extra_fields.json`: Standard ZIP Extra Fields TLV data structure schema.
- [SUBAGENT:research] `contracts/password_verifier.json`: In-memory multi-core password verification result schema.
- [SUBAGENT:research] `contracts/search_index.json`: Flat columnar memory search query and result schema.
- [SUBAGENT:research] `contracts/ndim_tensor.json`: N-dimensional hypercube partition and slicing schema.

---

## 4. Constitution & Invariant Checks

| Constitution Invariant | Compliance Strategy | Status |
| :--- | :--- | :--- |
| **Zero Memory Leaks** | Strict RAII cleanup, explicit `free()` of internal buffers, stack allocation wherever bounded | COMPLIANT |
| **Cryptographic Zeroing** | All temporary password and key buffers wiped with `ttzip_secure_zero` | COMPLIANT |
| **No Bare Objects / Strong Types** | Strongly-typed C structs declared in public bridge headers | COMPLIANT |
| **Zero Compiler Warnings** | Strict C11 `-Wall -Wextra -Werror` and Swift 6 Sendable compliance | COMPLIANT |
| **Anti-Regression Hard Floor** | All 912 existing unit and integration tests must pass with 0 failures | COMPLIANT |

---

## 5. Component Implementation Breakdown

### 5.1 C Bridge Headers & Implementations (`Sources/CTTZipBridge/`)
- `include/ttzip_reed_solomon.h` + `ttzip_reed_solomon.c`
- `include/ttzip_path_filter.h` + `ttzip_path_filter.c`
- `include/ttzip_zip_extra_field.h` + `ttzip_zip_extra_field.c`
- `include/ttzip_7z_header_parser.h` + `ttzip_7z_header_parser.c` (extend signature parser)
- `include/ttzip_password_verifier.h` + `ttzip_password_verifier.c`
- `include/ttzip_search_index.h` + `ttzip_search_index.c`
- `include/CTTZipTensorSlicing.h` + `CTTZipTensorSlicing.c` (extend hypercube solver)

### 5.2 Swift Facade Bridging (`Sources/TTZipCore/`)
- `Security/ReedSolomonFEC.swift`
- `Security/PathPatternFilterEngine.swift`
- `Standards/ZipExtraFieldParser.swift`
- `SevenZip/SevenZipHeaderReader.swift`
- `PasswordRecoveryEngine.swift`
- `Search/ArchiveSearchIndex.swift`
- `NDim/NDimTensorLayout.swift`
