# Feature Specification: Swift-Layer GCD Elimination & Concurrency Bridge

## 1. Executive Summary & Problem Statement

Following the successful elimination of 100% Apple GCD dependencies (`dispatch/dispatch.h` / `dispatch_*`) in the C bridge layer (`CTTZipBridge`), the Swift layer (`TTZipCore`) still contains **34 occurrences of Apple GCD primitives across 16 Swift files**.

These include:
- `DispatchQueue.concurrentPerform` (12 occurrences in 8 files): Used for multi-core block parallel compression, decompression, and CRC computation.
- `DispatchSemaphore` (6 occurrences in 5 files): Used for synchronous blocking wait over asynchronous template closures.
- `DispatchQueue.main.async` (4 occurrences in 2 files): Used for UI-thread event dispatching.
- `DispatchQueue(label:)` (3 occurrences in 2 files): Used for serial queue synchronization.
- `DispatchQueue?` parameter/property declarations (9 occurrences in 5 files): Used in the Observer pattern for custom queue injection.

This feature establishes `ConcurrencyBridge.swift` in `TTZipCore` to wrap the cross-platform C11 `ttzip_threadpool` and native Swift Structured Concurrency (`async/await`, `actor`, `withCheckedContinuation`, `@MainActor`), completely eliminating Apple GCD lock-in across all 16 Swift files without sacrificing throughput or introducing race conditions.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Multi-Core Block Parallel Operations (US1)
- **As an** archive engine developer or user running multi-gigabyte compression/decompression,
- **I want** parallel chunk tasks to execute over `ConcurrencyBridge.parallelFor` (backed by `ttzip_parallel_for` and `ttzip_threadpool_shared()`),
- **So that** multi-core throughput matches or exceeds GCD `concurrentPerform` while remaining 100% portable to non-Apple platforms.

### User Scenario 2: Synchronous Template Execution & Continuation (US2)
- **As an** engine caller using template method pattern (`BaseArchiveEngineTemplate`, `ZipArchiveEngineTemplate`, `SevenZipArchiveEngineTemplate`, `TarArchiveEngineTemplate`),
- **I want** template execution to use structured async/await or `withCheckedContinuation` instead of blocking `DispatchSemaphore`,
- **So that** threads are not parked in OS wait queues and the system avoids thread-pool starvation.

### User Scenario 3: Actor Isolation & MainActor Dispatch (US3)
- **As a** UI or event mediator caller (`ArchiveAppMediator`, `PasswordVaultManager`, `BenchmarkSpeedCache`),
- **I want** state isolation and UI updates to use `@MainActor` and Swift `actor` isolation instead of `DispatchQueue.main.async` and concurrent `DispatchQueue`,
- **So that** thread-safety is checked at compile-time by the Swift 6 compiler.

### User Scenario 4: Observer Pattern Clean-up (US4)
- **As an** event broadcaster or observer (`ArchiveEventCenter`, `ArchiveProgressBroadcaster`, `WeakObserverWrapper`),
- **I want** callback handlers to use `@Sendable` closures and `@MainActor` instead of untyped `DispatchQueue?`,
- **So that** observer registration is type-safe, decoupled from Apple GCD, and concurrency-compliant.

---

## 3. Functional Requirements

- **FR-01**: `ConcurrencyBridge.swift` MUST provide `parallelFor(count:worker:)` wrapping `ttzip_parallel_for` with `@Sendable (Int) -> Void` closure support and zero heap allocations per item.
- **FR-02**: `ConcurrencyBridge.swift` MUST export `ThreadBudget.optimalThreadCount(for:)` wrapping `ttzip_thread_budget_get()`.
- **FR-03**: `ConcurrencyBridge.swift` MUST export `MemoryBudget.safeBudget` wrapping `ttzip_mem_budget_query()`.
- **FR-04**: All 12 occurrences of `DispatchQueue.concurrentPerform` in `ZipExtremeBlockWriter.swift`, `ZipBlockParallelCompressor.swift`, `ZipBlockParallelDecompressor.swift`, `ZipMemoryEngine.swift`, `ZipParallelExtractor.swift`, `ZipParallelWriter.swift`, `ZipStoreStreamWriter.swift`, `SevenZipBlockParallelDecompressor.swift`, `SevenZipCryptoEngine.swift`, `HashCalculator.swift`, and `MultiCoreBreakdownRunner.swift` MUST be replaced with `ConcurrencyBridge.parallelFor`.
- **FR-05**: All 6 occurrences of `DispatchSemaphore` in `ArchiveEngineStrategy.swift`, `BaseArchiveEngineTemplate.swift`, `SevenZipArchiveEngineTemplate.swift`, `TarArchiveEngineTemplate.swift`, `ZipArchiveEngineTemplate.swift`, and `EnwikFixtureCacheManager.swift` MUST be replaced with structured async calls or `withCheckedContinuation`.
- **FR-06**: All 4 occurrences of `DispatchQueue.main.async` in `ArchiveAppMediator.swift` and `PasswordVaultManager.swift` MUST be replaced with `MainActor.run` or `@MainActor` methods.
- **FR-07**: `BenchmarkSpeedCache.swift` MUST be refactored from serial `DispatchQueue` to a thread-safe Swift `actor`.
- **FR-08**: `SubprocessExecutor.swift` MUST use `Task.detached` / `Task` instead of `DispatchQueue.global().async`.
- **FR-09**: All Observer classes (`ArchiveEventCenter.swift`, `ArchiveObserverProtocols.swift`, `ArchiveProgressBroadcaster.swift`, `TaskCancellationObserver.swift`, `WeakObserverWrapper.swift`) MUST remove `DispatchQueue?` properties and replace them with `@Sendable` callback isolation.
- **FR-10**: `grep -rn "DispatchQueue\|DispatchSemaphore\|DispatchGroup" Sources/TTZipCore/` MUST return 0 occurrences (excluding comments or platform-required `FileWatcherEngine` run loops if guarded).
- **FR-11**: All 42+ unit, matrix, and diagnostic tests MUST pass with 0 failures and 0 performance regressions.

---

## 4. Success Criteria

1. **Zero GCD Count in TTZipCore**: 34 GCD calls reduced to 0.
2. **Bit-Exact Correctness**: Full test suite (`AllFormatsAndAdvancedParametersMatrixTests`, `AllFormatDiagnosticSuiteTests`, `Blosc2PluginRegistryTests`, `EntropyAdaptiveExtremeRoutingTests`) passes 100% green.
3. **Zero Performance Regression**: Parallel compression/decompression throughput remains within ±2% of baseline.
4. **Swift 6 Strict Concurrency Compliance**: Zero compiler warnings under Swift 6 concurrency checks.

---

## 5. Clarifications

- **Q1**: Should `FileWatcherEngine.swift` retain `DispatchQueue` for FSEvents integration?
  - **Decision**: `FileWatcherEngine.swift` is macOS-specific FSEvents wrapper; guard with `#if canImport(Darwin)` and keep minimal run-loop hook while isolating all core logic.
- **Q2**: How should `ttzip_parallel_for` handle Swift closure contexts?
  - **Decision**: Use an unmanaged pointer box `SendableClosureBox` passed through `void* ctx` to the C static function pointer worker `ttzip_parallel_for_c_bridge_worker`.
