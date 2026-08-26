# Quickstart & Verification Guide: 020 Desktop Architecture Evolution & Quality Governance

- **Feature Directory**: `specs/020-desktop-architecture-and-quality-governance-evolution`
- **Classification**: `[Full SDD]`
- **Status**: `Specified`
- **Created**: 2026-08-26
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Prerequisites & Environment Setup

Ensure the development environment has the required toolchains:
- Xcode 16.0+ (Swift 6.0)
- Rust 1.80.0+ (with `aarch64-apple-darwin` target)
- macOS 14.0+ (Sonoma or Sequoia on Apple Silicon)

---

## 2. End-to-End Verification Scenarios

### Scenario 1: Cooperative Cancellation Responsiveness
1. Create a large test archive (1GB+):
   ```bash
   swift test --filter CooperativeCancellationLatencyTests
   ```
2. Trigger compression, immediately issue a cancel command after 100ms.
3. Verify that disk activity stops within $< 100\text{ms}$ and partial files are cleaned up.

### Scenario 2: Global Operations Queue Synchronization
1. Enqueue 3 background compression jobs.
2. Open `OperationsQueueView` sheet and verify that all 3 active jobs appear with live throughput and progress.
3. Verify Dock icon progress bar mirrors aggregate percentage.

### Scenario 3: Media Preview Downsampling Memory Cap
1. Inspect an archive containing high-resolution 4K/8K images.
2. Measure resident memory footprint before and after preview generation.
3. Verify peak memory delta remains $< 35\text{MB}$.

### Scenario 4: Repository Hygiene & LOC Gate
1. Run repository hygiene validator:
   ```bash
   ./scripts/lint_repo_hygiene.sh
   ```
2. Run single-file LOC gate:
   ```bash
   ./apple/scripts/lint_loc_gate.sh
   ```
3. Verify 0 violations and 0 compiler warnings under `swift test`.
