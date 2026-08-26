# Implementation Plan: Complete Optimization Wiring & Configuration Creep Audit

**Feature Directory**: `specs/093-complete-optimization-wiring-and-configuration-creep-audit/`  
**Status**: APPROVED FOR IMPLEMENTATION  
**Priority**: P1  
**Created**: 2026-08-18  

---

## 1. Technical Context & Constitution Check

### Technical Context
- **Target Subsystems**: `Sources/CTTZipBridge/ttzip_tar_native.c`, `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`, `Sources/TTZipCore/ArchiveCompressionTypes.swift`, `Sources/TTZipCore/ArchiveWriter+Dispatch.swift`.
- **Target Architectures**: Apple Silicon ARM64 (NEON, PMULL, L1D Cache 128KB, APFS Zero-Copy) + x86_64 fallback.
- **Invariants**: 100% Zero heap allocation (`malloc`/`free`) on file streaming write hot paths; Zero configuration creep (transparent SoC-aware parameter defaults); 100% in-process static C library bindings for all 16 formats.

### Constitution Check
- [x] **Zero-Cost Abstraction**: Replaces `malloc(1MB)` fallback with 64KB stack buffer.
- [x] **Zero Dynamic Allocation on Hot Paths**: Path concatenations replaced with stack `snprintf`.
- [x] **Fast-Path Preservation**: All 16 formats maintain direct fast-path dispatch.

---

## 2. Phase 0: Grounded Research Index

- `R001` [SUBAGENT:research]: Zero-Allocation Hot-Path Invariant Audit in `ttzip_tar_native.c` & Streaming Writers.
- `R002` [SUBAGENT:research]: Configuration Creep Analysis & Default Transparent Parameterization.
- `R003` [SUBAGENT:research]: 16-Format Full-Stack Engine Wiring & Dispatch Verification Matrix.

---

## 3. Phase 1: Design & Contract Index

- `Data Model`: [`data-model.md`](data-model.md)
- `Contracts`:
  - [`contracts/zero-allocation-audit-contract.json`](contracts/zero-allocation-audit-contract.json)
  - [`contracts/config-creep-contract.json`](contracts/config-creep-contract.json)
  - [`contracts/format-matrix-wiring-contract.json`](contracts/format-matrix-wiring-contract.json)
- `Quickstart Validation Guide`: [`quickstart.md`](quickstart.md)

---

## 4. Component Change Manifest

### C Native Bridge Layer (`Sources/CTTZipBridge/`)
- `[MODIFY]` `ttzip_tar_native.c`: Eliminate `malloc(chunk_cap)` in `write_reg_file_data`, replace with 64KB stack buffer loop; replace `asprintf` with stack `snprintf`.
- `[MODIFY]` `ttzip_tar_zstd_direct.c`: Replace scalar 512B zero loop with `ttzip_swar_is_zero_512b`; replace 4KB buffers with 64KB stack buffers.

### Swift Platform & Engine Layer (`Sources/TTZipCore/`)
- `[MODIFY]` `Sources/TTZipCore/ArchiveCompressionTypes.swift`: Clean up inert options documentation and ensure 100% automatic SoC physical defaults.
- `[MODIFY]` `Sources/TTZipCore/ArchiveWriter+Dispatch.swift`: Ensure all 16 formats route with zero CLI fallback and transparent adaptive tuning.

### Test Suite (`Tests/TTZipTests/`)
- `[NEW]` `Tests/TTZipTests/ExhaustiveOptimizationAuditTests.swift`: Comprehensive 16-format zero-allocation & transparent wiring test suite.
