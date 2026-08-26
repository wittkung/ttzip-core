# Implementation Plan: 147-full-122-files-c-migration-and-swift-slimming

## Technical Context
- **Target Architecture**: Pure C11 High-Performance Engine Core (`libttzip.a`) + Thin Swift 6.0 Presentation Shell (`TTZipApp`).
- **Scope**: Migration / Decoupling of 122 Swift files (22,341 lines) across 4 major clusters:
  1. Cluster 1 (37 files): Container & Archive Formats (`Zip/`, `SevenZip/`, `Tar/`, `Split/`, `InPlaceEdit/`)
  2. Cluster 2 (14 files): Security, FEC, Crypto & Search (`Security/`, `Crypto/`, `Search/`, `VFS/`)
  3. Cluster 3 (11 files): Frontend Heavy Calculations (`TTZipApp/Services/`, `ViewModels/`)
  4. Cluster 4 (60 files): Standalone CLI & Benchmarks (`CLI/`, `Benchmark/`)
- **Key Deliverables**:
  - `Sources/CTTZipBridge/include/ttzip_split.h` + `Sources/CTTZipBridge/ttzip_split.c`
  - `Sources/CTTZipBridge/include/ttzip_inplace.h` + `Sources/CTTZipBridge/ttzip_inplace.c`
  - `Sources/CTTZipBridge/include/ttzip_security.h` + `Sources/CTTZipBridge/ttzip_security.c`
  - `Sources/TTZipCore/Adapters/ArchiveEngineBridge.swift` (Unified Thin C Binding)
  - `CMakeLists.txt` updated with new C source files
  - `scripts/local-ci.sh` verified

## Constitution Check
- **Zero Heap Allocations on Hot Paths**: Verified. Uninitialized stack and mmap buffers used.
- **Zero Apple GCD Calls**: Verified. 100% `ttzip_threadpool` and `ttzip_parallel_for`.
- **Zero GPL-3 Dependencies**: Verified. All C modules are MIT/BSD/Apache-2.0.
- **Hardware Vector Dual-ISA**: Verified. ARM64 PMULL/NEON + x86_64 PCLMULQDQ/AVX2.

## Phase 0: Outline & Research
- - R001 [SUBAGENT:research] 《Multi-Volume Split Stream in Pure C》: Zero-copy chunk splitting across `.z01`, `.z02` and `.001`, `.002`.
- - R002 [SUBAGENT:research] 《In-Place Archive Mutation & Central Directory Patching》: Modifying and appending entries in-place via C memory mapping.
- - R003 [SUBAGENT:research] 《Reed-Solomon FEC & Sensitive Credential Scrubbing》: Dead Store Elimination (DSE) immune memory wipe and parity matrix calculation in C11.

## Phase 1: Design & Contracts
- `contracts/ttzip-engine-full-contract.json`
- `data-model.md`
- `quickstart.md`
