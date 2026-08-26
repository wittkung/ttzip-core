# Implementation Plan: 183-archive-dispatch-decoupling-and-protocol-modularization

## Technical Context
- **Objective**: Decompose all remaining oversized files in `TTZipCore` to guarantee 100% adherence to `< 350 LOC` and Single Responsibility Principle (SRP).

---

## Constitution Check
- [x] **Safe Architecture**: 100% of files $< 350\text{ LOC}$.
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **Zero Regression**: 100% pass rate across test suite.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《归档引擎分发与调度路由解耦》: Completed.
- R002 [SUBAGENT:research] 《压缩策略与组件协议接口隔离》: Completed.

---

## Phase 1: Component Change List
- **`Sources/TTZipCore/Bridge/ArchiveEngineBridge.swift`**: Split format mapping.
- **`Sources/TTZipCore/ArchiveWriter+Dispatch.swift`**: Split Zip and Tar/7z dispatchers.
- **`Sources/TTZipCore/Strategies/CompressionStrategyProtocol.swift`**: Split strategy interfaces.
- **`Sources/TTZipCore/ArchiveComponentProtocol.swift`**: Split component interfaces.
- **`Sources/TTZipCore/Testing/TestTerminalRenderer.swift`**: Split ANSI colorizer from results formatter.
- **`Sources/TTZipCore/Commands/CompressCommand.swift`**: Split argument options from execution logic.

---

## Phase 2: Verification Plan
1. `swift test` ensuring all 893+ tests pass with 0 failures and 0 warnings.
2. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
