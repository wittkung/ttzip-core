# Implementation Plan: Full Compression Formats and Algorithms Analysis

**Branch**: `138-compression-formats-algorithms` | **Date**: 2026-08-20 | **Spec**: [spec.md](spec.md)

**Input**: Comprehensive technical analysis of all archive formats and underlying compression algorithms supported by TTZip.

---

## Summary

This feature establishes an authoritative, publication-grade architectural and algorithmic specification of TTZip's core capabilities. It covers all 16 primary archive formats (ZIP, 7Z, TAR, TAR.GZ, TAR.ZST, TAR.BZ2, TAR.XZ, LZIP, LZ4, BROTLI, LRZIP, AAR, SNAPPY, WIM, DMG, ISO), 4 auxiliary formats (RAR/CBR, CAB, CPIO, XAR), and 14 underlying compression, entropy coding, pre-filtering, and hardware acceleration algorithms.

---

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs + ARM64 Assembly (`arm64inc.S`).  
**Primary Dependencies**: Static in-process C engines (`libdeflate`, `zlib-ng`, `LZMA SDK`, `fast-lzma2`, `libzstd`, `liblz4`, `brotli`, `liblzfse`, `snappy`, `libarchive` pristine worktree).  
**Storage**: APFS zero-copy file cloning (`clonefile`), memory-mapped I/O (`mmap`), POSIX asynchronous streams.  
**Testing**: `swift test` (525+ unit & regression tests), `XCTestPerformanceMeasureTests`, `ZipBenchPkTests` monotonic benchmark suite.  
**Target Platform**: macOS 14.0+ (Apple Silicon ARM64 NEON/PMULL/Crypto prioritized, Intel x86_64 compatible).  
**Project Type**: Desktop Application + Standalone CLI Tool (`ttzip-cli`) + Core In-Process Archiving Framework (`TTZipCore` / `CTTZipBridge`).  
**Performance Goals**: >12,000 MB/s extraction throughput, >28,000 MB/s 7Z packaging throughput, 48,160 MB/s ARM64 PMULL CRC64 throughput, <0.001 ms cold-start latency.  
**Constraints**: Zero CLI subprocess execution (`posix_spawn`/`NSTask` strictly forbidden), $\le 64\text{MB} \sim 128\text{MB}$ memory footprint per streaming task, zero bare pointer leaks, `-warnings-as-errors` strict compliance.  
**Scale/Scope**: 16 primary container formats, 4 auxiliary formats, 14 underlying compression algorithms, 46 head-to-head benchmark scenarios, full Pareto frontier mapping.

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] **Core Architecture Invariant**: 100% in-process static C bindings (`CTTZipBridge`), zero external CLI subprocess execution.
- [x] **Zero-Cost Hot Path Invariant**: Zero dynamic heap allocations in inner loops; direct pointer passes and stack buffers only.
- [x] **Stream-First Invariant**: Micro-buffering pull pipeline ($\le 64\text{MB}\sim 128\text{MB}$ peak memory); zero whole-file memory assumption.
- [x] **Invariant-First Invariant**: POSIX-level security guards (`ARCHIVE_EXTRACT_SECURE_SYMLINKS`, `ARCHIVE_EXTRACT_SECURE_NODOTDOT`, `ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS`, reverse mtime/permission fixup).
- [x] **Bounds-First Invariant**: Magic byte validation on C struct handles, zeroization of credentials via `memset_s` / `explicit_bzero`, narrowing clamp on 64-bit integers.
- [x] **Oracle-First Invariant**: Byte-accurate CRC32/SHA-256 validation against system oracle tools (`/usr/bin/tar`, `/usr/bin/unzip`).
- [x] **Hardware Grounding & Disassembly**: Micro-architectural validation of ARM64 PMULL, NEON SIMD, and AES vector instructions.

---

## Project Structure

### Documentation (this feature)

```text
specs/138-compression-formats-algorithms/
├── plan.md              # Implementation plan and architecture blueprint
├── research.md          # Phase 0 deep research synthesis (5 research subagent findings)
├── data-model.md        # Phase 1 data entities, formats taxonomy, and algorithm models
├── quickstart.md        # Phase 1 verification commands, benchmark assertions, and diagnostics
├── contracts/           # Phase 1 strongly-typed JSON Schema interface definitions
│   ├── format-matrix-schema.json
│   ├── algorithm-spec-schema.json
│   ├── engine-dispatch-schema.json
│   └── benchmark-profile-schema.json
├── checklists/
│   └── requirements.md  # Requirements quality checklist
└── tasks.md             # Phase 2 implementation task breakdown
```

### Source Code Architecture

```text
Sources/
├── CTTZipBridge/        # Layer 1: Low-level C bridge and ARM64 assembly micro-kernels
│   ├── CTTZipBridge_Archive.c
│   ├── CTTZipBridge_7z.c / 7zNativeDecoder.c / 7zParallel.c
│   ├── CTTZipBridge_ZipWrite.c / ZipChunkedStream.c
│   ├── CTTZipBridge_Zstd.c / LZFSE.c / Snappy.c / UnRAR.c
│   ├── CTTZipCRC32Neon.c / CTTZipAdler32Neon.c / ttzip_arm64_pmull.c
│   └── fast-lzma2/
├── TTZipCore/           # Layer 2: Swift 6 core domain models, pipelines, and engines
│   ├── ArchiveCompressionTypes.swift
│   ├── ArchiveEngineFamilyFactory.swift
│   ├── ArchiveReader.swift / ArchiveExtractor.swift / ArchiveWriter.swift
│   ├── AppleSiliconTuner.swift
│   └── Adapters/
└── TTZipCLI/            # Layer 3: Command-line interface and benchmark driver
```

---

## Phase 0: Research Items (Dispatched to Subagents)

- - R001 [SUBAGENT:research] 《Archive Containers & Framing Architecture》: Complete breakdown of ZIP, 7Z, TAR (PAX/UStar), WIM, DMG (UDIF/koly), ISO 9660, and Apple Archive container framing and stream layouts.
- - R002 [SUBAGENT:research] 《Deflate & LZMA Family Deep Compression Theory》: Mathematical and bitstream analysis of RFC 1951 Deflate (LZ77, Canonical Huffman, optimal parsing, Zopfli DAG) and LZMA/LZMA2 (Range Coder, Markov state modeling, context trees, BCJ filters).
- - R003 [SUBAGENT:research] 《Modern High-Throughput Algorithms (Zstandard, LZ4, LZFSE, Snappy, Brotli)》: Theoretical mechanics of tANS/FSE, hash/binary tree matchers, byte tokens, and pre-trained dictionaries in Zstandard, LZ4, LZFSE, Snappy, and Brotli.
- - R004 [SUBAGENT:research] 《BWT, Statistical & Specialty Algorithms (BZIP2, PPMd, LRZIP, Blosc2, Bit-Grooming, RAR)》: Formulation of Burrows-Wheeler Transform, MTF, RLE, PPMd context models, rzip multi-GB hash trees, Blosc2 byte shuffling, Bit-Grooming mantissa zeroing, and RAR 1.5–5.0 engines.
- - R005 [SUBAGENT:research] 《Hardware Acceleration & Cryptographic Integrity Subsystems》: ARM64 PMULL CRC64 Galois Field vectorization, ARMv8 AES-256 cryptography, NEON SIMD match counting, and cache-topology-aware thread dispatch.

---

## Phase 1: Design & Contracts

- `data-model.md`: Domain entity models for `ArchiveContainerFormat`, `CompressionAlgorithm`, `EntropyCodingTechnique`, `MatchFinderStrategy`, `HardwareKernelBinding`, and `FormatAlgorithmMatrix`.
- `contracts/`: Strongly-typed Draft-07 JSON Schemas:
  - `contracts/format-matrix-schema.json`: Complete format capability and algorithm mapping contract.
  - `contracts/algorithm-spec-schema.json`: Algorithm theoretical parameters, sliding window, and entropy coder contract.
  - `contracts/engine-dispatch-schema.json`: In-process C engine binding and thread dispatch configuration schema.
  - `contracts/benchmark-profile-schema.json`: Multi-workload Pareto performance and throughput metrics contract.
- `quickstart.md`: Verification commands for inspecting formats, running in-memory micro-benchmarks, and auditing hardware vector paths.

---

## Complexity Tracking

| Invariant / Feature | Why Needed | Simpler Alternative Rejected Because |
| :--- | :--- | :--- |
| **In-Process C11 Engine Bindings** | Sub-microsecond latency and >10 GB/s throughput | External subprocesses (`posix_spawn`) incur 10,000x latency penalty and fork overhead |
| **ARM64 PMULL Galois Field Vectorization** | 48,160 MB/s wire-speed checksum validation | Software table lookups saturate CPU at ~1,350 MB/s, creating an extraction bottleneck |
| **Micro-Buffering Pull Architecture** | Constant $\le 64\text{MB}$ memory consumption under 100GB+ archives | Whole-file memory allocation causes OS memory pressure, OOM crashes, and page-in stalls |
