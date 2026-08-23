# Implementation Plan: Swift-Layer GCD Elimination & Concurrency Bridge

## 1. Technical Context

Following the complete removal of Apple GCD (`dispatch_*`) from `CTTZipBridge`, `TTZipCore` contains 34 residual occurrences of GCD across 16 Swift files.
- Language & Compiler: Swift 6.0 (`swift-tools-version: 6.0`).
- Target OS: macOS 14.0+ / Apple Silicon & Intel x86_64, portable to POSIX & Windows.
- Core Integration: `ConcurrencyBridge.swift` wraps `ttzip_parallel_for`, `ttzip_thread_budget_get`, and `ttzip_mem_budget_query`.

## 2. Constitution Check

- **Zero-Cost Abstraction on Hot Paths**: `ConcurrencyBridge.parallelFor` allocates exactly 1 32-byte class box per entire loop (0 bytes on single iteration) and performs 0 allocations per iteration.
- **Stream-First & Zero Kernel Zeroing**: Maintained across all block writers.
- **No Shared Locks in Parallel Closures**: `parallelFor` passes unretained context with no mutex or semaphore in chunk iterations.
- **Logging Discipline**: All logs remain strictly through `TTLogger`.

## 3. Phase 0: Research & Architectural Decoupling

- - R001 [SUBAGENT:research] 《Swift `@Sendable (Int) -> Void` to C Function Pointer Bridge》: Tunneling Swift closure context into C `ttzip_parallel_for` using `Unmanaged.passUnretained` and `withExtendedLifetime` with zero per-iteration ARC overhead.
- - R002 [SUBAGENT:research] 《Deadlock-Free Native Synchronous Dual-Dispatch for Template Methods》: Replacing `DispatchSemaphore` with direct synchronous execution of underlying C engine routines, eliminating thread pool starvation and deadlocks.

## 4. Phase 1: Data Model, Contracts & Quickstart

- `data-model.md`: Definitions for `ConcurrencyBridge`, `ParallelForBox`, `ThreadBudget`, `MemoryBudget`, and `ArchiveProgressObserverProtocol`.
- `contracts/concurrency-bridge-schema.json`: JSON Schema draft-07 contract for thread budget, memory queries, and parallel iteration dispatch.
- `quickstart.md`: Verification commands and diagnostic scripts for validating zero-GCD and full matrix test execution.

## 5. Component Breakdown & Planned Changes

### [NEW] `Sources/TTZipCore/ConcurrencyBridge.swift`
- Exposes `ConcurrencyBridge.parallelFor(count:pool:_:)` and `(iterations:pool:_:)`.
- Exposes `ConcurrencyBridge.ThreadBudget` and `ConcurrencyBridge.MemoryBudget`.

### [MODIFY] Swift Multi-Core Call Sites (12 occurrences -> `ConcurrencyBridge.parallelFor`)
- `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift` (L108)
- `Sources/TTZipCore/Zip/ZipBlockParallelCompressor.swift` (L47)
- `Sources/TTZipCore/Zip/ZipBlockParallelDecompressor.swift` (L45)
- `Sources/TTZipCore/Zip/ZipMemoryEngine.swift` (L31, L81)
- `Sources/TTZipCore/Zip/ZipParallelExtractor.swift` (L90)
- `Sources/TTZipCore/Zip/ZipParallelWriter.swift` (L47)
- `Sources/TTZipCore/Zip/ZipStoreStreamWriter.swift` (L99, L150, L183)
- `Sources/TTZipCore/SevenZip/SevenZipBlockParallelDecompressor.swift` (L40)
- `Sources/TTZipCore/SevenZip/SevenZipCryptoEngine.swift` (L102)
- `Sources/TTZipCore/HashCalculator.swift` (L49)
- `Sources/TTZipCore/Benchmark/MultiCoreBreakdownRunner.swift` (L70, L85)

### [MODIFY] Template Methods & Synchronous Execution (6 occurrences -> Native Dual-Dispatch)
- `Sources/TTZipCore/TemplateMethod/BaseArchiveEngineTemplate.swift` (L137)
- `Sources/TTZipCore/TemplateMethod/ZipArchiveEngineTemplate.swift` (L188)
- `Sources/TTZipCore/TemplateMethod/SevenZipArchiveEngineTemplate.swift` (L113)
- `Sources/TTZipCore/TemplateMethod/TarArchiveEngineTemplate.swift` (L198)
- `Sources/TTZipCore/ArchiveEngineStrategy.swift` (L87)
- `Sources/TTZipCore/Benchmark/EnwikFixtureCacheManager.swift` (L166)

### [MODIFY] Actors & MainActor Dispatch (7 occurrences)
- `Sources/TTZipCore/Benchmark/BenchmarkSpeedCache.swift` (L40 -> `actor BenchmarkSpeedCache`)
- `Sources/TTZipCore/MediatorPattern/ArchiveAppMediator.swift` (L76, L97, L114 -> `@MainActor`)
- `Sources/TTZipCore/PasswordVaultManager.swift` (L114 -> `MainActor.run`)
- `Sources/TTZipCore/SubprocessExecutor.swift` (L58 -> `Task.detached`)

### [MODIFY] Observer Pattern Decoupling (9 occurrences -> `@Sendable`)
- `Sources/TTZipCore/Observers/ArchiveEventCenter.swift`
- `Sources/TTZipCore/Observers/ArchiveObserverProtocols.swift`
- `Sources/TTZipCore/Observers/ArchiveProgressBroadcaster.swift`
- `Sources/TTZipCore/Observers/TaskCancellationObserver.swift`
- `Sources/TTZipCore/Observers/WeakObserverWrapper.swift`
