# Implementation Plan: Unified SOTA Codec Engine & Multi-Core Architecture

**Branch**: `142-unified-sota-codec-architecture` | **Date**: 2026-08-20 | **Spec**: [spec.md](spec.md)

**Input**: Architectural blueprint integrating Layer 0 SOTA Single-Core Kernels, Layer 1 Universal Multi-Core Parallel Scheduler, Layer 2 Decoupled Container Framing, and Layer 3 Swift 6 Domain Layer.

---

## Summary

This plan details the concrete implementation architecture for TTZip's core engine: establishing an ultra-fast, SOTA single-core algorithmic foundation (`libdeflate`, `fast-lzma2`, `zstd`, `lz4`, `lbzip2`, `blosc2`, PMULL/NEON), building a universal multi-core parallel scheduler with sliding-window dictionary priming, and cleanly decoupling high-level container formats (ZIP, 7Z, TAR, DMG, WIM) from underlying compression codecs.

---

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs + ARM64 Assembly (`arm64inc.S`).  
**Primary Dependencies**: In-process static C bindings (`libdeflate`, `fast-lzma2`, `libzstd`, `liblz4`, `libbrotli`, `libarchive` pristine worktree).  
**Storage**: APFS zero-copy file cloning (`clonefile`), memory-mapped I/O (`mmap`), POSIX asynchronous streams.  
**Testing**: `swift test`, `AllFormatsAndAdvancedParametersMatrixTests`, `CRC64HardwareTests`, `XCTestPerformanceMeasureTests`.  
**Target Platform**: macOS 14.0+ (Apple Silicon ARM64 NEON/PMULL/Crypto prioritized, Intel x86_64 compatible).  
**Project Type**: Desktop Application + Standalone CLI Tool (`ttzip-cli`) + Core In-Process Archiving Framework (`TTZipCore` / `CTTZipBridge`).  
**Performance Goals**: >4,000 MB/s Deflate multi-core compression, >28,000 MB/s 7Z packaging, 48,160 MB/s ARM64 PMULL CRC64, <0.001 ms cold-start latency.  
**Constraints**: Zero CLI subprocess execution (`posix_spawn`/`NSTask` strictly forbidden), $\le 64\text{MB} \sim 128\text{MB}$ memory footprint per streaming task, zero bare pointer leaks, `-warnings-as-errors` strict compliance.  
**Scale/Scope**: 4 architectural layers, 16 container formats, 14 compression/hashing codecs, 6 critical failure mode mitigations.

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] **Core Architecture Invariant**: 100% in-process static C bindings (`CTTZipBridge`), zero external CLI subprocess execution.
- [x] **Zero-Cost Hot Path Invariant**: Zero dynamic heap allocations in inner loops; direct pointer passes and stack buffers only.
- [x] **Stream-First Invariant**: Micro-buffering pull pipeline ($\le 64\text{MB}\sim 128\text{MB}$ peak memory); zero whole-file memory assumption.
- [x] **Invariant-First Invariant**: POSIX-level security guards (`ARCHIVE_EXTRACT_SECURE_SYMLINKS`, `ARCHIVE_EXTRACT_SECURE_NODOTDOT`, `ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS`, reverse mtime/permission fixup).
- [x] **Bounds-First Invariant**: Magic byte validation on C struct handles, zeroization of credentials via `ttzip_secure_zero`, narrowing clamp on 64-bit integers.
- [x] **Oracle-First Invariant**: Byte-accurate CRC32/SHA-256 validation against system oracle tools (`/usr/bin/tar`, `/usr/bin/unzip`).
- [x] **Hardware Grounding & Disassembly**: Micro-architectural validation of ARM64 PMULL, NEON SIMD, and AES vector instructions.

---

## Project Structure

### Documentation (this feature)

```text
specs/142-unified-sota-codec-architecture/
├── plan.md              # Implementation plan and architecture blueprint
├── research.md          # Phase 0 deep research synthesis (SOTA single-core & multi-core repos)
├── data-model.md        # Phase 1 data entities, codec VTable, and scheduler models
├── quickstart.md        # Phase 1 verification commands and benchmark assertions
├── contracts/           # Phase 1 strongly-typed JSON Schema interface definitions
│   ├── codec-vtable-schema.json
│   ├── parallel-scheduler-schema.json
│   ├── container-writer-schema.json
│   └── dictionary-priming-schema.json
├── checklists/
│   └── requirements.md  # Requirements quality checklist
└── tasks.md             # Phase 2 implementation task breakdown
```

### Source Code Architecture

```text
Sources/
├── CTTZipBridge/                       # Layer 0 & Layer 1: Low-level C bridge & parallel scheduler
│   ├── codecs/
│   │   ├── ttzip_codec_deflate.c       # SOTA libdeflate single-core wrapper
│   │   ├── ttzip_codec_lzma2.c         # SOTA fast-lzma2 single-core wrapper
│   │   ├── ttzip_codec_zstd.c          # SOTA libzstd single-core wrapper
│   │   ├── ttzip_codec_lz4.c           # SOTA liblz4 single-core wrapper
│   │   └── ttzip_codec_bzip2.c         # SOTA lbzip2 single-core wrapper
│   ├── parallel/
│   │   ├── ttzip_parallel_engine.c     # Universal multi-core chunk scheduler
│   │   ├── ttzip_dict_overlap.c        # Zero-copy sliding ring dictionary buffer
│   │   └── ttzip_bitstream_seq.c       # Format-aware bitstream sequencer (BFINAL management)
│   ├── hardware/
│   │   ├── ttzip_crc64_pmull.c         # ARM64 PMULL CRC64 (48.16 GB/s)
│   │   ├── ttzip_crc32_neon.c          # ARMv8 ACLE CRC32 (65 GB/s)
│   │   └── ttzip_crypto_aes.c          # ARMv8 8-way interleaved AES-256
│   └── include/
│       ├── ttzip_codec.h               # Unified C ABI VTable
│       └── ttzip_parallel_engine.h     # Multi-core parallel API
└── TTZipCore/                          # Layer 2 & Layer 3: Swift 6 containers & domain
    ├── Containers/                     # Decoupled container framing
    │   ├── Zip/                        # ZIP / Zip64 / WinZip AES
    │   ├── SevenZip/                   # 7Z StartHeader / Coders DAG / Solid
    │   ├── Tar/                        # POSIX PAX 512B streams
    │   └── DiskImages/                 # Apple UDIF (DMG) / ISO / WIM
    └── Engines/
        └── ArchiveWriter.swift         # Unified multi-core write pipeline
```

---

## Phase 0: Research Items (Grounded Codebase & Upstream Survey)

- - R001 [SUBAGENT:research] 《Single-Core SOTA Engine Integration》: Microarchitecture evaluation of `libdeflate`, `fast-lzma2`, `libzstd`, `liblz4`, `lbzip2`, `google/brotli`, and `Blosc/c-blosc2`.
- - R002 [SUBAGENT:research] 《Multi-Core Parallel Scheduling & Dictionary Priming》: Chunk boundary sliding dictionary inheritance, lock-free work distribution, and memory-page flyweight pooling.
- - R003 [SUBAGENT:research] 《Format-Aware Bitstream Sequencer & Standard Invariants》: Strict RFC 1951 Deflate BFINAL management, LZMA2 chunk control markers, and PAX extended stream compliance.
- - R004 [SUBAGENT:research] 《Asymmetric Topology & Adaptive Dual-Track Routing》: P-core vs E-core chunk sizing, work-stealing queues, and small-file vs large-file dual-track routing.

---

## Phase 1: Design & Contracts

- `data-model.md`: Domain models for `CodecVTableDescriptor`, `ParallelSchedulerConfig`, `ContainerWriterDescriptor`, `DictionaryPrimingWindow`, and `DualTrackWorkloadProfile`.
- `contracts/`: Strongly-typed Draft-07 JSON Schemas:
  - `contracts/codec-vtable-schema.json`: Layer 0 single-core C ABI VTable contract.
  - `contracts/parallel-scheduler-schema.json`: Layer 1 multi-core scheduler configuration contract.
  - `contracts/container-writer-schema.json`: Layer 2 decoupled container writer contract.
  - `contracts/dictionary-priming-schema.json`: Sliding dictionary overlap and memory buffer contract.
- `quickstart.md`: Runnable validation scenarios, Silesia/Enwik8 benchmarks, and standard oracle verification.

---

## Complexity Tracking

| Invariant / Feature | Why Needed | Simpler Alternative Rejected Because |
| :--- | :--- | :--- |
| **SOTA Single-Core VTable ABI** | Eliminates 300% single-core bottleneck from legacy `zlib`/scalar codebases | Reusing generic `zlib` caps total multi-core ceiling at $\le 1.2\text{ GB/s}$ |
| **Zero-Copy Sliding Dictionary View** | Prevents L1/L2 cache line thrashing and high-frequency `memcpy` | Copying 32KB~2MB dictionary per chunk wastes memory bandwidth |
| **Format-Aware Bitstream Sequencer** | Guarantees 100% compliance with `/usr/bin/unzip` and standard OS decoders | Generating isolated Deflate chunks causes stream truncation errors in external tools |
