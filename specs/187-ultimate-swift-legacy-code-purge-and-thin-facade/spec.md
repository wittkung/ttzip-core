# Feature Specification: 187-ultimate-swift-legacy-code-purge-and-thin-facade

## 1. Executive Summary & Strategic Motivation
This is the ultimate architectural purification milestone of the TTZip project:
1. **Purge 200+ redundant legacy Swift files**:
   - Delete legacy container implementations: `Zip/`, `SevenZip/`, `Zstd/`, `Snappy/`, `Tar/`.
   - Delete obsolete Swift design pattern scaffolding: `TemplateMethod/`, `StatePattern/`, `MediatorPattern/`, `MementoPattern/`, `ChainOfResponsibility/`, `Decorators/`, `Flyweights/`, `Observers/`, `Adaptive/`, `DependencyInjection/`, `IteratorPattern/`.
   - Delete redundant computation services: `FolderStatsCalculator`, `ArchiveDiskPreallocator`, `ArchiveEntropyEvaluator`, `SmartCodecSelector`.
2. **Consolidate TTZipCore into ~30 Ultra-Thin Facades**:
   - `ArchiveWriter`, `ArchiveExtractor`, `ArchiveReader`, `ArchiveRepairEngine`, `ArchiveIntegrityChecker`, `PasswordVaultManager`, `PasswordRecoveryEngine`, `SplitVolumeEngine`, `QuickLookPreviewEngine`.
   - All operations delegate 100% directly to Safe Rust microkernel C-ABI (`ttzip_rust_*`).
3. **Streamline Tests & Validate 0 Regressions**:
   - Delete tests associated with removed scaffolding.
   - Retain full E2E, standards compliance, differential oracle, and UI integration tests.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Ultra-Fast Clean SPM Build
- **Given** building `TTZipCore` on macOS
- **When** compiling via `swift build`
- **Then** SPM compiles ~30 lean files in $<1.0\text{s}$ with zero warnings and zero legacy bloat.

### User Scenario 2: Pure Rust Single Source of Truth
- **Given** developers building on Linux or Windows
- **When** compiling `rust/ttzip-glue` or `rust/ttzip-tui`
- **Then** 100% of logic is in Rust with zero dependency on Swift.

---

## 3. Success Metrics
1. **Source Code Purge**: Delete ~200 redundant Swift files (~30,000 LOC eliminated).
2. **Lean Swift Skin**: TTZipCore reduced to $< 35$ files, 100% $< 350\text{ LOC}$.
3. **Zero Regression**: 100% pass rate on `cargo test`, `swift test`, and `./scripts/run_local_ci_gate.sh`.
