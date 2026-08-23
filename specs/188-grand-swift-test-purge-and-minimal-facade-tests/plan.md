# Implementation Plan: 188-grand-swift-test-purge-and-minimal-facade-tests

## Technical Context
- **Objective**: Purge 70+ redundant Swift test files from `Tests/TTZipTests/`, establish a minimal high-level test suite, and align CI gates.

---

## Constitution Check
- [x] **Safe Rust Single Oracle**: 100% of engine invariants covered in `rust/ttzip-glue/tests/`.
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **Ultra-Fast Local Gate**: Local CI passes in $<10\text{s}$.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《Swift 冗余测试清单与清理方案》: Completed.
- R002 [SUBAGENT:research] 《极简 Swift 公共 API 集成测试收敛》: Completed.

---

## Phase 1: Component Change List

### 1. Minimal Swift Facade Integration Test Suite
- Create `Tests/TTZipTests/TTZipCoreIntegrationTests.swift` covering `ArchiveWriter`, `ArchiveExtractor`, `ArchiveReader`, `SplitVolumeEngine`, and `PasswordVaultManager`.

### 2. Purge Redundant Swift Tests
- Delete 70+ low-level test files in `Tests/TTZipTests/`.
- Retain only:
  - `TTZipCoreIntegrationTests.swift`
  - `CLICommandE2ETests.swift`
  - `CLIPOSIXStandardTests.swift`
  - `QuickLookPreviewTests.swift`
  - `AppStorePackageAuditTests.swift`
  - Helper fixtures (`IsolatedTempSandbox.swift`, `TestFileGenerator.swift`, `SilesiaFixtureLoader.swift`)

### 3. Local CI Gate Streamlining
- Update `./scripts/run_local_ci_gate.sh` to execute the lean Swift suite and full Rust suite.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` on all Rust crates.
2. `swift test` ensuring all remaining tests pass with 0 failures and 0 warnings.
3. `./scripts/run_local_ci_gate.sh` full CI validation.
