# Tasks: 081-professional-competitor-gap-audit

**Input**: Design documents from `/specs/081-professional-competitor-gap-audit/`
**Prerequisites**: `spec.md`, `plan.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md`

## Format: `[ID] [P?] [Story] Description`
- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g. US1, US2, US3, US4, US5)

---

## Phase 1: Setup & Core Data Models

**Purpose**: Establish domain models and contracts matching `data-model.md` and JSON schemas in `contracts/`.

- [x] T001 [P] [US1] Create `SplitVolumeConfig.swift` model in `Sources/TTZipCore/Split/SplitVolumeConfig.swift` conforming to `contracts/split-volume-config.json`
- [x] T002 [P] [US2] Create `RecoveryRecordPayload.swift` model in `Sources/TTZipCore/Security/RecoveryRecordPayload.swift` conforming to `contracts/recovery-record-payload.json`
- [x] T003 [P] [US3] Create `ArchiveSearchModels.swift` in `Sources/TTZipCore/Search/ArchiveSearchModels.swift` conforming to `contracts/archive-search-query.json`
- [x] T004 [P] [US5] Create `HardwareBenchmarkMetric.swift` in `Sources/TTZipCore/Benchmark/HardwareBenchmarkMetric.swift` conforming to `contracts/hardware-benchmark-metric.json`
- [x] T005 [P] Create model verification test suite `CompetitorGapModelTests.swift` in `Tests/TTZipTests/CompetitorGapModelTests.swift`

---

## Phase 2: User Story 1 - 多格式多卷归档创建与自适应分卷分割 (Priority: P1)

**Purpose**: Implement in-stream zero-copy split volume sinker (`MultiVolumeStreamSink`) and PKZIP/7Z/TAR multi-part output.

- [x] T006 [US1] Implement `MultiVolumeStreamSink.swift` in `Sources/TTZipCore/Split/MultiVolumeStreamSink.swift` with exact byte boundary tracking and automatic `.001`/`.z01` file rollover
- [x] T007 [US1] Update `ArchiveWriter.swift` and `ArchiveWriter+Dispatch.swift` to route split archive writes directly into `MultiVolumeStreamSink` without monolithic intermediate disk caching
- [x] T008 [US1] Integrate multi-volume size presets (CD 700MB, DVD 4.7GB, FAT32 4GB, Email 25MB/100MB, Custom) into `CompressModalView.swift` and `PresetEditorCardView.swift`
- [x] T009 [P] [US1] Create split volume test suite `SplitVolumeCreationTests.swift` in `Tests/TTZipTests/SplitVolumeCreationTests.swift`

---

## Phase 3: User Story 2 - 前向纠错恢复记录与冗余包保护 (Priority: P1)

**Purpose**: Implement Cauchy Reed-Solomon $\text{GF}(2^{16})$ FEC encoder/decoder, `TTZR` header trailer, and auto-repair integration.

- [x] T010 [US2] Implement `ReedSolomonFEC.swift` in `Sources/TTZipCore/Security/ReedSolomonFEC.swift` with ARM NEON SIMD table lookups and Cauchy matrix inversion
- [x] T011 [US2] Implement `ArchiveRecoveryRecordEngine.swift` in `Sources/TTZipCore/Security/ArchiveRecoveryRecordEngine.swift` for embedding transparent `TTZR`/`TTRC` recovery blocks into TAR, 7Z, and ZIP
- [x] T012 [US2] Update `ArchiveRepairEngine.swift` and `ArchiveIntegrityChecker.swift` to automatically detect recovery records and execute self-healing reconstruction on corrupted sectors
- [x] T013 [US2] Expose "Add Recovery Record (1%~10%)" slider and status indicator in `CompressModalView.swift`
- [x] T014 [P] [US2] Create recovery record and error injection test suite `ReedSolomonRecoveryRecordTests.swift` in `Tests/TTZipTests/ReedSolomonRecoveryRecordTests.swift`

---

## Phase 4: User Story 3 - 归档内穿透式瞬时全文搜索与选择性提取 (Priority: P2)

**Purpose**: Implement sub-15ms contiguous columnar index (`ArchiveSearchIndex`) and format-aware selective stream extraction.

- [x] T015 [US3] Implement `ArchiveSearchIndex.swift` in `Sources/TTZipCore/Search/ArchiveSearchIndex.swift` with contiguous UTF-8 byte buffer and SIMD substring matching
- [x] T016 [US3] Implement `ArchiveSelectiveExtractor.swift` in `Sources/TTZipCore/ArchiveSelectiveExtractor.swift` supporting direct ZIP random seeks, 7Z solid block skipping, and streaming skip
- [x] T017 [US3] Connect `ArchiveSearchIndex` to `ArchiveTreeStore.swift` and `ArchiveExplorerView.swift` for live 30Hz keystroke search and "Extract Selected" action
- [x] T018 [P] [US3] Create in-archive search benchmark test suite `InArchiveSearchEngineTests.swift` in `Tests/TTZipTests/InArchiveSearchEngineTests.swift`

---

## Phase 5: User Story 4 - 密码保险库 Touch ID 生物识别解锁与 7Z 头部文件名加密 (Priority: P2)

**Purpose**: Implement macOS LocalAuthentication Touch ID biometrics and 7Z AES-256 encrypted header (`-mhe=on`).

- [x] T019 [US4] Implement `TouchIDAuthenticator.swift` in `Sources/TTZipCore/Security/TouchIDAuthenticator.swift` with `LAContext` biometrics and Secure Enclave keychain binding
- [x] T020 [US4] Update `PasswordVaultManager.swift` and `PasswordVaultViewModel.swift` with Touch ID prompt and graceful fallback to system master password
- [x] T021 [US4] Ensure 7Z writer emits `kEncodedHeader` (0x17) when "Encrypt File Names" is toggled in `CompressModalView.swift`
- [x] T022 [P] [US4] Create biometric vault and encrypted header test suite `TouchIDAndHeaderEncryptionTests.swift` in `Tests/TTZipTests/TouchIDAndHeaderEncryptionTests.swift`

---

## Phase 6: User Story 5 - GUI 原生多核能效基准测试与实时硬件仪表盘 (Priority: P3)

**Purpose**: Implement 7-Zip aligned MIPS calculation, Mach thread CPU sampling, and SwiftUI 30Hz telemetry dashboard.

- [x] T023 [US5] Implement `MIPSHardwareBenchmarkEngine.swift` in `Sources/TTZipCore/Benchmark/MIPSHardwareBenchmarkEngine.swift` with 7-Zip `CBenchProps` formulas and Mach thread CPU telemetry
- [x] T024 [US5] Implement `BenchmarkDashboardView.swift` and `BenchmarkDashboardViewModel.swift` in `Sources/TTZipApp/Views/` with real-time speed dial, MIPS rating, and CPU load graphs
- [x] T025 [P] [US5] Create MIPS benchmark test suite `MIPSBenchmarkEngineTests.swift` in `Tests/TTZipTests/MIPSBenchmarkEngineTests.swift`

---

## Phase 7: Verification & Convergence

**Purpose**: Full-suite regression and schema consistency analysis.

- [x] T026 Run full automated regression test suite (`swift test`) verifying zero regressions across all 80+ test files
- [x] T027 Execute `speckit-analyze` to verify 100% artifact consistency across spec, plan, contracts, and implementations
