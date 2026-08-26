# Tasks: 全面国际化、CLI 标准化与测试体系专业化构建 (Task Breakdown)

**Feature**: `068-i18n-cli-test-standardization`  
**Date**: 2026-08-17  
**Status**: Completed  
**Plan**: [plan.md](./plan.md) | **Spec**: [spec.md](./spec.md)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization, core data models and formatting utilities

- [x] T001 [P] Create language enumeration and metadata in `Sources/TTZipCore/Localization/AppLanguage.swift`
- [x] T002 [P] Create type-safe localization keys and namespaces in `Sources/TTZipCore/Localization/LocaleKey.swift`
- [x] T003 [P] Create unit formatters in `Sources/TTZipCore/Localization/Formatters/ByteSizeFormatter.swift`, `ThroughputFormatter.swift`, `PluralRuleEngine.swift`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core localization engine, static catalogs and testing infrastructure that MUST be complete before user stories

- [x] T004 Implement `TTZipLocalizationManager` with thread-safe cascading fallback and POSIX env parsing in `Sources/TTZipCore/Localization/TTZipLocalizationManager.swift`
- [x] T005 [P] Create 7 language catalog dictionaries in `Sources/TTZipCore/Localization/Catalogs/` (`LocaleCatalog+En.swift`, `LocaleCatalog+ZhHans.swift`, `LocaleCatalog+ZhHant.swift`, `LocaleCatalog+Ja.swift`, `LocaleCatalog+De.swift`, `LocaleCatalog+Fr.swift`, `LocaleCatalog+Es.swift`)
- [x] T006 [P] Create test framework infrastructure (`TestTier.swift`, `TestReportModel.swift`, `JUnitReportBuilder.swift`) in `Sources/TTZipCore/Testing/`
- [x] T007 [P] Create error localization bridge in `Sources/TTZipCore/Localization/Extensions/ArchiveError+L10n.swift`

---

## Phase 3: User Story 1 - 多语言与无缝国际化体验 (Priority: P1) 🎯 MVP

**Goal**: Deliver full i18n support across 7 languages, dynamic SwiftUI language switching and 100% key parity validation

**Independent Test**: `swift test --filter LocalizationIntegrityTests` passes with 100% key parity and placeholder format matching.

### Tests for User Story 1
- [x] T008 [P] [US1] Create `LocalizationIntegrityTests.swift` in `Tests/TTZipTests/LocalizationIntegrityTests.swift` (100% key parity & placeholder type check)

### Implementation for User Story 1
- [x] T009 [P] [US1] Implement `AppLanguageStore.swift` in `Sources/TTZipApp/Services/AppLanguageStore.swift` with SwiftUI dynamic observable binding
- [x] T010 [US1] Refactor `Sources/TTZipApp/Views/` (SettingsView, CompressModalView, ExtractModalView, PasswordVaultView, etc.) to use `LocaleKey`
- [x] T011 [US1] Verify GUI dynamic switching and run `LocalizationIntegrityTests` to assert zero missing keys

---

## Phase 4: User Story 2 - 企业级标准化的专业命令行交互 (Priority: P1)

**Goal**: Deliver POSIX/GNU compliant CLI with TTY adaptive rendering, 60Hz throttling, `<sysexits.h>` exit codes, stream pipes and NDJSON streaming

**Independent Test**: `swift test --filter CLIPOSIXStandardTests` passes with verified exit codes, pipe streams and TTY/NDJSON modes.

### Tests for User Story 2
- [x] T012 [P] [US2] Create `CLIPOSIXStandardTests.swift` in `Tests/TTZipTests/CLIPOSIXStandardTests.swift` (Testing exit codes, pipes, dry-run, json mode)

### Implementation for User Story 2
- [x] T013 [P] [US2] Create `CLIExitCode.swift` in `Sources/TTZipCore/CLI/CLIExitCode.swift` (POSIX `<sysexits.h>` mapping)
- [x] T014 [P] [US2] Create `TerminalRenderEngine.swift` in `Sources/TTZipCore/CLI/TerminalRenderEngine.swift` (TTY detection, `ioctl` width adaptive, 60Hz throttling, Unicode progress bar)
- [x] T015 [P] [US2] Create `StreamPipeAdapter.swift` in `Sources/TTZipCore/CLI/StreamPipeAdapter.swift` (`-` stdin/stdout streaming & APFS 2-tier spooling)
- [x] T016 [P] [US2] Create `CLICommandSpec.swift` in `Sources/TTZipCore/CLI/CLICommandSpec.swift` (Declarative spec, Shell completions generator for Bash/Zsh/Fish & Man page)
- [x] T017 [US2] Create `POSIXCLIArgumentParser.swift` in `Sources/TTZipCore/CLI/POSIXCLIArgumentParser.swift` (compact flags, `--` delimiter, `--key=value`)
- [x] T018 [US2] Refactor `CLICommandRouter.swift` in `Sources/TTZipCLI/CLICommandRouter.swift` to route POSIX commands, i18n, exit codes, and NDJSON streaming

---

## Phase 5: User Story 3 - 全维度多层次标准化测试体系 (Priority: P1)

**Goal**: Deliver 6-Tier test classification, JUnit XML / JSON / Markdown reporting engine and strict historical peak performance gate verification

**Independent Test**: `swift test --filter TestTierClassificationTests` and `swift test --filter XCTestPerformanceMeasureTests` pass.

### Tests for User Story 3
- [x] T019 [P] [US3] Create `TestTierClassificationTests.swift` in `Tests/TTZipTests/TestTierClassificationTests.swift` (Validating Tier 0-5 classification)
- [x] T020 [US3] Refactor `TestCommand.swift` & `TestReportGenerator.swift` in `Sources/TTZipCLI/TestCommand.swift` and `Sources/TTZipCLI/TestReportGenerator.swift` to support `--tier`, `--format`, JUnit XML, JSON, Markdown export
- [x] T021 [US3] Add strict historical performance floor assert validation in Tier 3 performance gates
- [x] T022 [US3] Verify all tests via `swift test` and `swift test --filter LocalizationIntegrityTests` / `CLIPOSIXStandardTests` / `TestTierClassificationTests`

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, quickstart validation, and full regression verification

- [x] T023 [P] Update documentation in `docs/` and `README.md` for POSIX CLI, i18n locales, and Test Tiers
- [x] T024 Validate quickstart scenarios per `specs/068-i18n-cli-test-standardization/quickstart.md`
- [x] T025 Run full CI test suite and verify 100% pass rate
