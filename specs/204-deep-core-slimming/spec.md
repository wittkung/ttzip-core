# Spec: Deep Swift Core Slimming Phase 2 (OOP Strategy Hierarchy Purge)

**Feature**: `204-deep-core-slimming`  
**Classification**: `[Lean SDD]` (Codebase slimming, dead OOP hierarchy purge, redundant abstractions removal)  
**Status**: `COMPLETED`  

---

## 1. Context & Objectives

Following the sinking of core archiving algorithms, compression engines, password recovery routines, and in-place editing into Rust C-ABI, the Swift codebase retained obsolete OOP abstraction layers, complex abstract factory providers, and duplicate strategy protocol hierarchies.

This feature executes Phase 2 of the deep architectural slimming:
1. **Purge Dead OOP Strategy Hierarchies**: Eliminate `ArchiveRepairStrategyProtocol`, `CharsetDetectionStrategyProtocol`, `CompressionStrategyFactory`, `CompressionStrategyProtocol`, and `PasswordRecoveryStrategyProtocol`.
2. **Eliminate Redundant Factories & Traversal Classes**: Remove `ArchiveEngineFamilyFactory.swift`, `ArchiveEngineStrategy.swift`, `ArchiveComponentTraversals.swift`, and `CompressCommandBuilder.swift`.
3. **Consolidate Thin Facades**: Refactor `ArchiveEngineFactory`, `ArchiveOperationPipeline`, `ArchivePipelineBuilder`, `ArchiveComponentProtocol`, and `CharsetDetector` into ultra-thin, direct native facades.
4. **Enforce Zero-Regression Gates**: Validate with 4-stage local CI gate (`lint_loc_gate.sh`, `swift test`, `ttzip-bench gate`, `run_rust_tests.sh`).

---

## 2. Deleted Files (9 Redundant Files Purged)

- `Sources/TTZipCore/ArchiveComponentTraversals.swift` (225 LOC)
- `Sources/TTZipCore/ArchiveEngineFamilyFactory.swift` (253 LOC)
- `Sources/TTZipCore/ArchiveEngineStrategy.swift` (192 LOC)
- `Sources/TTZipCore/Commands/CompressCommandBuilder.swift` (103 LOC)
- `Sources/TTZipCore/Strategies/ArchiveRepairStrategyProtocol.swift` (333 LOC)
- `Sources/TTZipCore/Strategies/CharsetDetectionStrategyProtocol.swift` (170 LOC)
- `Sources/TTZipCore/Strategies/CompressionStrategyFactory.swift` (164 LOC)
- `Sources/TTZipCore/Strategies/CompressionStrategyProtocol.swift` (323 LOC)
- `Sources/TTZipCore/Strategies/PasswordRecoveryStrategyProtocol.swift` (342 LOC)

---

## 3. Verification & Metrics

- **Net LOC Reduction**: -2,028 LOC (`25 files changed, 274 insertions(+), 2302 deletions(-)`).
- **Single-File LOC Defense Gate**: 641 files scanned, 100% $\le 800\text{ LOC}$.
- **Swift Test Suite**: 138/138 tests passing.
- **Rust Industrial Test Suite**: 42 unit + 9 proptest + 4 fuzzing targets passing.
- **Deflate-Bench 50-Point Matrix Gate**: 100% passed.
