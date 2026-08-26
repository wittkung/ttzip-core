# Quickstart: Unified Path and Search Address Bar (一体化路径与搜索地址栏)

**Feature Branch**: `098-unified-path-search-bar`
**Created**: 2026-08-18

---

## 1. Prerequisites & Environment Setup

Ensure the TTZip workspace is clean and compiles without warnings:

```bash
swift build
```

---

## 2. Automated Validation Scenarios

### Scenario 1: POSIX Path Sanitizer & Expansion Unit Suite
Validates that tilde paths (`~/Downloads`), shell-escaped paths (`/My\ Folder`), relative paths (`../sub`), and `file://` URLs are accurately normalized.

- **Command**:
  ```bash
  swift test --filter POSIXPathSanitizerTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'POSIXPathSanitizerTests' passed.
  Executed 12 tests, with 0 failures (0 unexpected) in 0.015 seconds.
  ```
- **Failure Diagnostic**:
  - If tests fail on tilde expansion, verify `(path as NSString).expandingTildeInPath` or `NSHomeDirectory()` behavior.
  - If tests fail on shell unescaping, inspect the backslash scanner state machine in `POSIXPathSanitizer.swift`.

---

### Scenario 2: High-Speed Asynchronous Autocompletion & Micro-Caching
Validates that directory prefix lookups return within $\le 15\text{ ms}$, LRU cache hits return in $< 0.5\text{ ms}$, and task cancellation prevents stale dropdown flashes.

- **Command**:
  ```bash
  swift test --filter AsyncPathAutocompletionTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'AsyncPathAutocompletionTests' passed.
  Executed 10 tests, with 0 failures (0 unexpected) in 0.045 seconds.
  ```
- **Failure Diagnostic**:
  - If tests fail on latency, check that directory scanning runs on `Task.detached(priority: .userInitiated)` rather than `@MainActor`.
  - If tests fail on stale results, ensure `completionTask?.cancel()` is dispatched immediately on each query update.

---

### Scenario 3: Destination Dispatcher & Sandbox Permission Probing
Validates that directories update `currentDirectory`, archives trigger `openArchiveAsFolder`, and non-existent paths produce structured error feedback without throwing exceptions.

- **Command**:
  ```bash
  swift test --filter DestinationDispatcherTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'DestinationDispatcherTests' passed.
  Executed 8 tests, with 0 failures (0 unexpected) in 0.022 seconds.
  ```
- **Failure Diagnostic**:
  - If archive detection fails, verify `ArchiveCompressionFormat.isArchiveExtension` integration in `DestinationDispatcher.swift`.
  - If permission handling fails, check `RootFolderAccessManager.shared.ensureAccess` return flags.

---

### Scenario 4: End-to-End Frontend Regression & UI Gate
Validates that the omnibar integration does not degrade directory tree construction or UI frame rates.

- **Command**:
  ```bash
  swift test --filter FrontendPerformanceGateTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'FrontendPerformanceGateTests' passed.
  Executed 15 tests, with 0 failures (0 unexpected) in 0.320 seconds.
  ```
- **Failure Diagnostic**:
  - If 50,000-node directory tree construction exceeds 250ms, verify that breadcrumb rendering does not trigger redundant filesystem scans.

---

## 3. Manual Interactive Verification on macOS

1. **Launch App**:
   ```bash
   swift run TTZipApp
   ```
2. **Path Navigation (`⌘L` / `⇧⌘G`)**:
   - Press `⌘L` or `⇧⌘G`. Assert the top address bar highlights in gold and selects the entire current path.
   - Type `~/Downloads` and press `Return`. Assert the explorer view navigates to the user's Downloads folder.
3. **Autocomplete Dropdown**:
   - Type `/usr/` in the address bar. Assert a Liquid Glass dropdown displays matching folders (`/usr/bin`, `/usr/lib`, `/usr/local`).
   - Press `↓` and `Return`. Assert navigation jumps into the selected folder.
4. **Archive Direct Opening**:
   - Type the path to an existing `.zip` or `.7z` file (e.g. `~/Downloads/test.zip`) and press `Return`. Assert TTZip enters Archive Explorer mode.
5. **Spotlight Search Dual-Mode**:
   - Type a search query without slashes (e.g. `report`). Assert the leading icon turns green with magnifying glass and displays Spotlight search results.
