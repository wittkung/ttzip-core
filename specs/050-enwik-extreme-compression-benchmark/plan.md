# Implementation Plan: enwik8 / enwik9 Extreme Compression Benchmark

**Feature**: `050-enwik-extreme-compression-benchmark`
**Created**: 2026-08-17
**Status**: Ready for Task Generation

---

## 1. Technical Context & Motivation

This feature introduces the enwik8 (100 MB) and enwik9 (1 GB) extreme compression ratio benchmark suite to TTZip. The corpus consists of structured Wikipedia XML containing extensive long-distance repetitive patterns. It provides automated benchmarking and gating for:
1. High-level dictionary algorithms (LZMA2 L5~9, ZSTD L19~22 Ultra, BZIP2 L9).
2. Peak Resident Set Size (RSS) memory ceiling enforcement via zero-overhead Darwin Mach / Linux `/proc` kernel telemetry.
3. Out-of-tree localized cache management with multi-mirror resilient download and POSIX `flock` multi-process safety.
4. Deterministic XML pattern synthesizer (> 2000 MB/s) providing zero-network fallback.

---

## 2. Constitution & Engineering Invariant Check

- **Stream-First**: 
  - Zero full-file unconstrained loading in hot paths.
  - Page-aligned micro-buffering (`PlatformMemory.allocateAlignedPageBuffer`) with single reusable 64 KB buffers.
  - Zero `Data(count:)` zeroing faults.
- **Bounds-First**:
  - `MemoryCeilingSnapshot` bounds peak resident memory to $\le 512$ MB on enwik8.
  - Buffer offsets clamped to `SSIZE_MAX`.
- **Invariant-First**:
  - POSIX `flock` RAII wrappers guarantee automatic lock cleanup on abnormal termination, preventing CI hangs.
  - Temporary download staging to `.tmp.<pid>` with atomic POSIX `rename`.
- **Oracle-First**:
  - 100% byte-for-byte SHA-256 parity assertions on all decompressed output.
  - Golden fingerprinted hash checking on raw payload before executing benchmarks.

---

## 3. Phase 0: Research Items Index

- - R001 [SUBAGENT:research] 《Zero-Overhead Memory Telemetry》: Darwin Mach `task_info` vs. Linux `/proc/self/statm` for peak RSS capture. (Documented in [research.md](research.md))
- - R002 [SUBAGENT:research] 《Out-of-Tree Cache & Concurrency Coordination》: Multi-mirror download, in-process extraction, and POSIX `flock` file locking. (Documented in [research.md](research.md))
- - R003 [SUBAGENT:research] 《High-Throughput Deterministic XML Corpus Synthesis》: Seed-indexed $O(1)$ memory chunk synthesis architecture. (Documented in [research.md](research.md))

---

## 4. Phase 1: Design Artifacts & Contracts Index

- **Data Models**: Defined in [data-model.md](data-model.md)
- **Contracts**:
  - [SUBAGENT:research] [enwik-benchmark-request.schema.json](contracts/enwik-benchmark-request.schema.json)
  - [SUBAGENT:research] [enwik-benchmark-result.schema.json](contracts/enwik-benchmark-result.schema.json)
  - [SUBAGENT:research] [enwik-fixture-manifest.schema.json](contracts/enwik-fixture-manifest.schema.json)
  - [SUBAGENT:research] [memory-telemetry-snapshot.schema.json](contracts/memory-telemetry-snapshot.schema.json)
- **Quickstart & Verification**: Defined in [quickstart.md](quickstart.md)

---

## 5. Planned Changes by Component

### Component 1: `TTZipCore/Platform` (Memory Telemetry & File System Locks)
- **[MODIFY]** `Sources/TTZipCore/Platform/PlatformMemory.swift`: Add zero-allocation `PlatformMemory.currentMemoryUsage() -> MemoryCeilingSnapshot` using Darwin Mach `task_info` / Linux `statm`.
- **[MODIFY]** `Sources/TTZipCore/Platform/PlatformFileSystem.swift`: Add `PlatformFileSystem.withFileLock(atPath:type:_:)` supporting POSIX `flock` advisory locks.

### Component 2: `TTZipCore/Benchmark` (Corpus Generator & Cache Manager)
- **[NEW]** `Sources/TTZipCore/Benchmark/SyntheticXmlCorpusGenerator.swift`: High-throughput (> 2000 MB/s) seed-indexed structured XML synthesizer.
- **[NEW]** `Sources/TTZipCore/Benchmark/EnwikFixtureCacheManager.swift`: Out-of-tree cache manager with multi-mirror download, SHA-256 validation, and in-process extraction.

### Component 3: `TTZipTests` (Benchmark & Regression Suite)
- **[NEW]** `Tests/TTZipTests/ExtremeRatioBenchmarkSuiteTests.swift`: Comprehensive multi-algorithm (LZMA2, ZSTD, BZIP2) enwik8/enwik9 benchmark tests with peak RSS memory assertions.
- **[NEW]** `Tests/TTZipTests/SyntheticXmlCorpusGeneratorTests.swift`: Tests for throughput, deterministic SHA-256 parity, and long-distance pattern correctness.
- **[NEW]** `Tests/TTZipTests/EnwikFixtureCacheManagerTests.swift`: Tests for cache hit, lock concurrency, and mirror fallback.
