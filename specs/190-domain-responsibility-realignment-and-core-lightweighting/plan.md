# Implementation Plan: 190-domain-responsibility-realignment-and-core-lightweighting

## Technical Context
- **Objective**: Purge and realign 75+ files from `Sources/TTZipCore/` to achieve true core lightweighting ($< 50$ files total in `TTZipCore`).

---

## Constitution Check
- [x] **Target Isolation**: Benchmark logic in `TTZipBench`, CLI in `TTZipCLI`, Micro-kernel in `rust/`.
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **Clean Swift 6 Concurrency**: Zero legacy POSIX mutex wrappers.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《Benchmark 领域隔离方案》: Completed.
- R002 [SUBAGENT:research] 《Swift TUI 与并发模式彻底淘汰》: Completed.

---

## Phase 1: Benchmark Domain Realignment
- Move required benchmark runner types to `Sources/TTZipBench/`.
- Purge `Sources/TTZipCore/Benchmark/` (48 files).

## Phase 2: Swift TUI & Concurrency Patterns Purge
- Delete `Sources/TTZipCore/CLI/TUI/` (6 files).
- Delete `Sources/TTZipCore/ConcurrencyPatterns/` (20 files).
- Delete `Sources/TTZipCore/Security/MalformedStreamFuzzEngine.swift` (1 file).

## Phase 3: Verification Plan
1. `swift build` and `swift test` pass cleanly.
2. `cargo test --workspace` passes cleanly.
3. `./scripts/run_local_ci_gate.sh` passes 100%.
