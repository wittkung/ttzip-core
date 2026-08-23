# Quickstart & Verification Guide: Full Frontend Bilingual Localization

## Overview
This guide provides executable verification scenarios to validate the complete frontend Chinese & English bilingual localization in TTZip.

---

## Verification Scenarios

### Scenario 1: Localization Catalog Key Parity & Zero Missing Keys
Verify that 100% of all `L10n.allRawKeys` are populated across all 7 language catalogs without missing keys or orphan entries.

- **Command**:
  ```bash
  swift test --filter LocalizationIntegrityTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'LocalizationIntegrityTests' passed.
  Executed 5 tests, with 0 failures (0 unexpected) in 0.045 seconds.
  ```
- **Failure Diagnostic**:
  - If a test fails with `Missing key [key.name] in LocaleCatalogZhHans`, locate the missing key in `Sources/TTZipCore/Localization/LocaleKey.swift` and add the translated string entry to `Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+ZhHans.swift`.
  - If a test fails with `Orphan key detected`, remove the unused key from the corresponding catalog dictionary.

---

### Scenario 2: GUI Reactive Localization & Language Switching
Verify that switching languages dynamically via `AppLocalizationState` updates all view models and catalogs within < 10ms.

- **Command**:
  ```bash
  swift test --filter GUILocalizationTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'GUILocalizationTests' passed.
  Executed 6 tests, with 0 failures (0 unexpected) in 0.082 seconds.
  ```
- **Failure Diagnostic**:
  - If language switching fails to update, ensure `AppLocalizationState.shared.setLanguage(...)` correctly triggers `self.objectWillChange.send()` and mutates `TTZipLocalizationManager.shared.currentLanguage`.

---

### Scenario 3: Zero Hardcoded Chinese/English Literals Codebase Audit
Execute static codebase audit scanning for hardcoded UI text in `Sources/TTZipApp/Views/`.

- **Command**:
  ```bash
  swift test --filter HardcodedStringAuditTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'HardcodedStringAuditTests' passed.
  Found 0 hardcoded user-facing literals across 67 SwiftUI view files.
  ```
- **Failure Diagnostic**:
  - If hardcoded strings are detected, replace the string literal with `l10n.t(L10n.<Domain>.<key>)` and add the corresponding key to `LocaleKey.swift`.

---

### Scenario 4: Full App Build & Regression Suite Gate
Verify that the entire TTZip compilation and test suite (525+ tests) completes with zero warnings and zero regressions.

- **Command**:
  ```bash
  swift test
  ```
- **Expected Output**:
  ```text
  Executed 530+ tests, with 0 failures (0 unexpected).
  ```
- **Failure Diagnostic**:
  - Check for any compiler type errors in `Sources/TTZipCore` or `Sources/TTZipApp`.
