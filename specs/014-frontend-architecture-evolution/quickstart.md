# Quickstart Validation Guide: 014 Frontend Architecture Evolution

- **Feature**: `014-frontend-architecture-evolution`
- **Created**: 2026-08-25
- **Status**: Ready for Validation

---

## 1. Prerequisites & Build Commands

Ensure the Swift and Rust build environments are intact:
```bash
# Verify Rust engine compilation and UniFFI binding export
cd core/rust && cargo build --release

# Run Swift full test suite with strict concurrency checks
cd ../.. && swift test --package-path apple
```

---

## 2. Automated Validation Scenarios

### Scenario 1: State Isolation & 60Hz Re-render Gate
- **Purpose**: Verify that `TaskExecutionState.progressValue` updates at 60Hz do not trigger body invalidations in navigation, sidebar, or explorer.
- **Verification Command**:
  ```bash
  swift test --package-path apple --filter FrontendPerformanceMetricsTests
  ```
- **Expected Outcome**:
  - `RootReRenderCount == 0` during high-frequency simulated task progress stream.

---

### Scenario 2: Batch I/O Scanning Throughput Benchmark
- **Purpose**: Measure performance of `DiskDirectoryScannerActor` on folders with 5,000 simulated files.
- **Verification Command**:
  ```bash
  swift test --package-path apple --filter DiskDirectoryScannerTests
  ```
- **Expected Outcome**:
  - 5,000-file directory scan completes in $< 25\text{ ms}$ on Apple Silicon (zero individual `attributesOfItem` syscalls).

---

### Scenario 3: Archive Hierarchy Session Cache $O(1)$ Traversal
- **Purpose**: Verify that subpath column expansion in a 100,000-entry archive hits `ArchiveHierarchySessionCache` without re-running `inspectArchive`.
- **Verification Command**:
  ```bash
  swift test --package-path apple --filter ArchiveHierarchySessionCacheTests
  ```
- **Expected Outcome**:
  - `cache.getOrFetchSession` returns cached session in $< 0.5\text{ ms}$ with 0 disk reads.

---

### Scenario 4: Syntax Highlighting Latency & Background Tokenizer
- **Purpose**: Validate that typing in a 10,000-line code preview keeps main thread keystroke latency $< 8\text{ ms}$.
- **Verification Command**:
  ```bash
  swift test --package-path apple --filter BackgroundSyntaxTokenizerTests
  ```
- **Expected Outcome**:
  - All tokenization executes off-main-thread; regex patterns are precompiled with 0 dynamic regex creations.

---

### Scenario 5: Full Localization & Theme Token Alignment
- **Purpose**: Verify zero hardcoded string remnants and 100% Design Token adherence.
- **Verification Command**:
  ```bash
  python3 apple/scripts/lint_loc_gate.py
  ```
- **Expected Outcome**:
  - All files adhere to $\le 800$ LOC threshold; 0 unlocalized hardcoded strings in explorer/alerts.
