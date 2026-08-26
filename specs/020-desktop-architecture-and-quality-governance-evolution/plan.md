# Implementation Plan: 020 Desktop Architecture Evolution & Full-Chain Quality Governance

- **Feature Directory**: `specs/020-desktop-architecture-and-quality-governance-evolution`
- **Classification**: `[Full SDD]`
- **Status**: `Implementation & RCA Verification Complete`
- **Created**: 2026-08-26
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Technical Context & Architectural Invariants

### 1.1 Architecture Topology

```
                              ┌──────────────────────────────────┐
                              │    AppIntentDispatcher (Router)  │
                              └─────────────────┬────────────────┘
                                                │ Routes to KeyWindow / Matching Path
                        ┌───────────────────────┴───────────────────────┐
                        ▼                                               ▼
         ┌──────────────────────────────┐                ┌──────────────────────────────┐
         │ ArchiveSessionContext (Doc A)│                │ ArchiveSessionContext (Doc B)│
         │ - Isolated VFS Tree Cache    │                │ - Isolated VFS Tree Cache    │
         │ - Dedicated Password State   │                │ - Dedicated Password State   │
         │ - Tabbing Mode: .preferred   │                │ - Tabbing Mode: .preferred   │
         └──────────────┬───────────────┘                └──────────────┬───────────────┘
                        │ Dispatches tasks                              │ Dispatches tasks
                        ▼                                               ▼
         ┌──────────────────────────────────────────────────────────────────────────────┐
         │                  ArchiveOperationsQueueCenter (Global Shared)                 │
         │  - ArchiveTaskCoordinator (Cooperative CancellationToken / Pause / Resume)  │
         │  - Monotonic Batch Progress Telemetry & Real-Time Throughput (MB/s)          │
         │  - DockProgressManager (30Hz Throttled macOS Dock Progress Ring)             │
         │  - AppErrorReporter (Structured Diagnostic Codes & Non-Destructive Sheets)   │
         └──────────────────────────────────────┬───────────────────────────────────────┘
                                                │ UniFFI Rust Rayon Threadpool
                                                ▼
         ┌──────────────────────────────────────────────────────────────────────────────┐
         │            TTZip Rust Core Engine (Zip / SevenZ / Tar / Streaming)           │
         │         Atomic cancellation checks per 64KB~1MB chunk (<100ms latency)       │
         │                 RAII Incomplete Output Rollback Guard                        │
         └──────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Constitution Check
- **100% Mozilla UniFFI Standard**: Swift interacts with Rust strictly via UniFFI `CancellationToken`, `create_archive_stream`, and `extract_archive_stream`.
- **Swift 6 Strict Concurrency**: `@MainActor` on all UI ViewModels and session contexts; `Sendable` on all error payloads and task execution handles.
- **Single-File LOC Limit ($\le 800$ LOC)**: All 429 source files are under 800 LOC.
- **Release-by-Default & Zero Warnings**: `-warnings-as-errors` enforced across SPM configurations.

---

## 2. Seven Core Engineering Pillars (RCA Decisions)

1. **P1: Cooperative Cancellation & Rollback Guard**:
   - `ArchiveTaskCoordinator` + UniFFI `CancellationToken` in `ArchiveWriter` & `ArchiveExtractor`.
   - Chunk-level cancellation checks in Rust loops; elimination of 7z fallback penetration; automatic rollback of incomplete `.zip` artifacts on cancellation.
2. **P2: Monotonic Operations Queue & Telemetry Hub**:
   - `ArchiveOperationsQueueCenter` + `DockProgressManager` with monotonic batch progress calculation and FSM terminal state lock.
3. **P3: Multi-Session Document Architecture & Tab Merging**:
   - `ArchiveSessionContext` per Window/Tab; session-partitioned VFS cache keys (`\(sessionId):\(entryHash)`); elimination of `NotificationCenter` modal broadcasts.
4. **P4: Deep VFS Tree Navigation & Ancestor Auto-Expansion**:
   - `NativeArchiveOutlineView.findAncestorChain` with prefix pruning ($O(D \cdot B)$) and top-down parent expansion.
5. **P5: Memory-Budgeted Media Previews**:
   - `DownsampledImageLoader` (ImageIO thumbnailing at 2048px), eliminating unbounded bitmap allocations.
6. **P6: Universal Error Diagnosis & Recovery**:
   - `AppErrorReporter` with structured diagnostic codes (`ERR_CORRUPT_HEADER`, `ERR_CRC_MISMATCH`, etc.) and `ErrorPresentationSheetView`.
7. **P7: Engineering Governance & Meta-Tooling**:
   - `lint_loc_gate.py` with explicit `--dir` and `--min-files 10` assertion; `lint_repo_hygiene.sh` with SPM target-to-disk symmetry check.

---

## 3. Verification & CI Gates

| Validation Gate | Script / Command | Target Threshold |
| :--- | :--- | :--- |
| **Core Test Suite** | `swift test` (core/) | 100% Pass (64/64 tests) |
| **Apple Test Suite** | `swift test` (apple/) | 100% Pass (152/152 tests) |
| **Total Test Suite** | `core/` + `apple/` | 100% Pass (216/216 tests) |
| **LOC Defense Gate** | `bash apple/scripts/lint_loc_gate.sh` | 0 files > 800 LOC across 429 files |
| **Repository Hygiene Gate** | `bash scripts/lint_repo_hygiene.sh` | 0 orphaned folders, 0 unreferenced files |
| **Contracts Linter** | `bash specs/.../contracts/lint-contracts.sh` | 100% contracts valid |
