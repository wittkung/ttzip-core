# Quickstart & Verification Guide: 073-media-preview-audit-tui-engine-acceleration

This document outlines manual and automated verification procedures for Desktop Media Preview hardening, CLI Interactive TUI Mode, and Core SIMD Decompression Acceleration.

---

### Scenario 1: Desktop Media Preview Memory & Lifecycle Audit

- **Command**:
  ```bash
  swift test --filter MediaPreviewAuditTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'MediaPreviewAuditTests' passed
  Executed 4 tests, with 0 failures in ~0.35 seconds
  ```
- **Failure Diagnostic**:
  - If image downsampling fails, check `CGImageSourceCreateThumbnailAtIndex` max pixel clamp in `ImageIOThumbnailCache.swift`.
  - If AVPlayer teardown fails, ensure `player.replaceCurrentItem(with: nil)` is called in `.onDisappear`.

---

### Scenario 2: Interactive Terminal TUI Mode Navigation & Selective Extraction

- **Command**:
  ```bash
  swift test --filter InteractiveTUITests
  ```
- **Expected Output**:
  ```text
  Test Suite 'InteractiveTUITests' passed
  Executed 4 tests, with 0 failures in ~0.20 seconds
  ```
- **Failure Diagnostic**:
  - If terminal mode assertion fails, verify `tcgetattr`/`tcsetattr` RAII teardown in `TerminalRawModeManager.swift`.
  - If key sequence parsing fails, check `TUIKeyParser.parseKey` multi-byte escape buffer handling.

---

### Scenario 3: Core LZ4/ZSTD Streaming SIMD Decompression

- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'XCTestPerformanceMeasureTests' passed
  Executed 13 tests, with 0 failures in ~1.10 seconds
  ```
- **Failure Diagnostic**:
  - If LZ4/ZSTD throughput drops below historical floors, verify 64KB page-alignment in `posix_memalign` and PMULL inline unrolling.

---

### Scenario 4: Local 6-Stage Automated CI Regression Gate

- **Command**:
  ```bash
  ./scripts/run_local_ci_gate.sh --json reports/ci_gate_073.json
  ```
- **Expected Output**:
  ```text
  Total: 6 Passed, 0 Failed
  Exported JSON gate report to reports/ci_gate_073.json
  ✅ Local CI/CD Gate Passed! 100% compliant and ready.
  ```
- **Failure Diagnostic**:
  - Inspect `/tmp/ci_gate_*.log` and JSON report for individual failing stage diagnostics.
