# Research & Technical Decisions: Streamlining Redundant Swift C-Wrapper Tests

**Feature**: `155-155-streamline-redundant`  
**Date**: 2026-08-20  
**Status**: Completed  

---

## 1. Dual-Engine Test Suite Decoupling & Pruning Policy

### Decision
Adopt a **Selective Microkernel Pruning Strategy** to cleanly decouple testing responsibilities:
- **Pure C-Wrapper Test Suites Pruned from `Tests/TTZipTests/`**:
  1. `ZipSlipDefenseTests.swift` (100% superseded by `tests/c/test_security_zipslip.c`)
  2. `SingleCoreDeflateOracleTests.swift` (100% superseded by `tests/c/test_deflate_zopfli.c`)
  3. `SevenZipHeaderParserTests.swift` (100% superseded by `tests/c/test_7z_lzma2.c`)
  4. `BranchlessDecompTests.swift` (100% superseded by `tests/c/test_deflate_zopfli.c`)
  5. `StreamingDecompressorDualSymbolLutTests.swift` (100% superseded by `tests/c/test_deflate_zopfli.c`)
  6. `SwarOptimizationBenchmarkTests.swift` (100% superseded by `tests/c/test_magic_sniff.c` & `test_strnatcmp.c`)
  7. `CRC32PmullDifferentialTests.swift` (100% superseded by `tests/c/test_crc_neon.c`)

- **Essential Swift Architecture Suites Retained in `Tests/TTZipTests/` and `Tests/TTZipAppTests/`**:
  - All 16 GoF Design Pattern suites (`AdapterPatternTests`, `BridgePatternTests`, `ChainOfResponsibilityTests`, `CompositePatternTests`, `DecoratorPatternTests`, `FacadePatternTests`, `FlyweightPatternTests`, `InterpreterPatternTests`, `IteratorPatternTests`, `ObserverPatternTests`, `ProducerConsumerPatternTests`, `ProxyPatternTests`, `ReadWriteLockPatternTests`, `RepositoryPatternTests`, `StrategyPatternTests`, `TemplateMethodPatternTests`, `WorkerPoolPatternTests`).
  - Swift Concurrency Bridge & Memory Limits (`ConcurrencyBridgeTests.swift`, `HardwareChecksumTests.swift`, `CRC64HardwareTests.swift`, `InPlaceHuffmanTests.swift`, `ContextMemoryPoolTests.swift`, `MmapBufferHandleTests.swift`).
  - Swift High-Level Archiving & End-to-End Containers (`SevenZipBridgeTests.swift`, `TarVariantEdgeCasesTests.swift`, `FastLZMA2Tests.swift`, `ArchiveWriterTests.swift`, `ArchiveReaderTests.swift`, `ArchiveExtractorTests.swift`, `ArchiveBuilderTests.swift`).
  - All AppKit UI, ViewModel, and GUI Localization tests in `Tests/TTZipAppTests/`.

### Rationale
- **Zero Loss of Invariant Coverage**: Every mathematical assertion, PMULL vector, 7z varint boundary, and Zip-Slip traversal check is fully and continuously tested by `tests/c/` in < 4ms.
- **SwiftPM Compilation & Test Acceleration**: Removing 7 heavy C-wrapper test files saves multiple seconds of Swift compilation and link overhead during `swift test`.
- **Architectural Clarity**: Eliminates confusing duplication where developers had to maintain identical test vectors in both Swift and C.

### Alternatives Considered
- **Keep All (Status Quo)**: Rejected due to maintenance duplication, slower CI cycles, and unclear test ownership.
- **Aggressive Pruning (Delete all tests touching C)**: Rejected because it would break verification of Swift adapter protocols, type safe models, and GoF patterns.
- **Selective Microkernel Pruning (Selected)**: Preserves 100% invariant coverage, speeds up CI, and clarifies dual-engine boundaries.

### Source
- `tests/c/test_*.c`
- `Tests/TTZipTests/` audit by Test Suite Audit Subagent (`43fb1041-17a1-45c1-b37a-f9d468de928f`)
- `CMakeLists.txt` & `scripts/local-ci.sh`
