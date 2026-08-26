# Tasks: enwik8 / enwik9 Extreme Compression Benchmark

**Feature**: `050-enwik-extreme-compression-benchmark`
**Created**: 2026-08-17
**Status**: Ready for Implementation

---

## Phase 1: Platform Memory Telemetry & File Locking Infrastructure

- [x] T001 [P] [US2] Implement zero-overhead `MemoryCeilingSnapshot` and Mach `task_info` / Linux `statm` sampling in `Sources/TTZipCore/Platform/PlatformMemory.swift`
- [x] T002 [P] [US1] Implement POSIX `flock` advisory locking RAII wrapper `PlatformFileSystem.withFileLock` in `Sources/TTZipCore/Platform/PlatformFileSystem.swift`

---

## Phase 2: High-Speed Synthetic Generator & Out-of-Tree Cache Manager

- [x] T003 [P] [US1] Implement `SyntheticXmlCorpusGenerator` with seed-indexed $O(1)$ memory chunk synthesis in `Sources/TTZipCore/Benchmark/SyntheticXmlCorpusGenerator.swift`
- [x] T004 [P] [US2] Implement `EnwikFixtureCacheManager` with multi-mirror download, in-process self-decompression, and file locking in `Sources/TTZipCore/Benchmark/EnwikFixtureCacheManager.swift`

---

## Phase 3: Component Unit Tests

- [x] T005 [P] [US1] Implement unit tests for synthetic generator throughput, SHA-256 parity, and repetition pattern correctness in `Tests/TTZipTests/SyntheticXmlCorpusGeneratorTests.swift`
- [x] T006 [P] [US2] Implement unit tests for fixture cache manager, lock concurrency, and mirror fallback in `Tests/TTZipTests/EnwikFixtureCacheManagerTests.swift`

---

## Phase 4: Extreme Ratio Benchmark Suite & Memory Ceiling Gating

- [x] T007 [US1] Implement `ExtremeRatioBenchmarkSuiteTests` evaluating LZMA2, ZSTD Ultra, and BZIP2 with byte-level verification and Peak RSS memory ceiling assertions in `Tests/TTZipTests/ExtremeRatioBenchmarkSuiteTests.swift`

---

## Phase 5: Verification & Quality Regression

- [x] T008 [US3] Run full test suite validation (`swift test` and `TTZIP_RUN_BENCHMARKS=1 swift test --filter ExtremeRatioBenchmarkSuiteTests`) to assert zero regression across the codebase in `Tests/TTZipTests/`
