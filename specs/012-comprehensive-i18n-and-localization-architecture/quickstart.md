# Quickstart & Verification Guide: 012 Comprehensive i18n and Localization Architecture Overhaul

- **Feature Directory**: `specs/012-comprehensive-i18n-and-localization-architecture`
- **Created**: 2026-08-25

---

## 1. Prerequisites

- macOS 14.0+ (Sonoma or Sequoia)
- Xcode 15.0+ / Swift 6.0 Toolchain
- `jq` (for contract validation)

---

## 2. Validation Scenarios

### Scenario 1: Comprehensive Catalog Completeness & Anti-Pseudo Guard
Run the automated security test suite to verify 100% key coverage, $< 3\%$ English copy ratio in foreign catalogs, and positional format specifiers:
```bash
swift test --filter TTZipLocalizationSecurityTests
```
*Expected Outcome*: All 7 language catalogs pass key parity, anti-copy-paste rules, and format specifier consistency checks without failure.

### Scenario 2: Dynamic In-App Language Switching (< 10ms)
1. Launch TTZip:
   ```bash
   ./apple/scripts/bundle_app.sh --open
   ```
2. Open **Settings (⌘,)** -> **General** -> **Language**.
3. Switch dynamically between:
   - English -> Deutsch -> Français -> Español -> 日本語 -> 繁體中文 -> 简体中文
4. Verify:
   - All active windows, sidebars, omnibars, inspectors, and status bars update immediately.
   - AppKit top-level menu bar (`File`, `Edit`, `View`, `Window`, `Help`) translates dynamically without restart and never freezes.
   - Right-click contextual menus in Finder Miller Columns render in the selected language.

### Scenario 3: Cross-Process FinderSync Context Menu Synchronization
1. Open a Finder window and select any archive file (e.g. `test.7z`).
2. Right-click and inspect the TTZip contextual submenu.
3. Switch language in TTZip Settings to `日本語`.
4. Right-click again in Finder:
   *Expected Outcome*: Menu items immediately display in Japanese (e.g. `⚡️ ここに展開`, `🔍 診断と検査`) without restarting Finder.

### Scenario 4: Quick Look HTML Preview Localization
1. In Finder or TTZip Explorer, select an archive and press `Spacebar`.
2. Inspect the Quick Look window:
   *Expected Outcome*: Table headers (`名前`, `サイズ` in Japanese / `Name`, `Größe` in German), item count, and compression statistics render in the active language.

### Scenario 5: AST Zero-Hardcoding Linter Gate
Run the static AST analyzer across all SwiftUI views:
```bash
swift run --package-path core ttzip-ast-linter --path apple/Sources/TTZipApp
```
*Expected Outcome*: Zero unwhitelisted human-readable string literals detected.
