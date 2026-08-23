# Research Notes: Swift-Layer GCD Elimination & Concurrency Bridge

## Research Item R001: Swift `@Sendable (Int) -> Void` to C Function Pointer Bridge

### Decision
Implement `ConcurrencyBridge.parallelFor` in `Sources/TTZipCore/ConcurrencyBridge.swift` by boxing the `@Sendable (Int) -> Void` closure in an immutable `final class ParallelForBox: @unchecked Sendable`. The box is converted to an unmanaged opaque pointer via `Unmanaged.passUnretained(box).toOpaque()` and shielded with `withExtendedLifetime(box)` across the synchronous C call `ttzip_parallel_for`. Inside the C static callback `@convention(c) (Int, UnsafeMutableRawPointer?) -> Void`, the context pointer is cast back via `Unmanaged<ParallelForBox>.fromOpaque(userData).takeUnretainedValue()` without atomic retain/release overhead. Fast paths handle `count == 0` (no-op) and `count == 1` (direct synchronous execution on the calling thread).

### Rationale
- **Zero Per-Iteration Overhead**: `takeUnretainedValue()` executes a single bitcast without generating `swift_retain` or `swift_release` instructions on the closure context across concurrent worker threads, eliminating CPU cache line bouncing and atomic bus contention.
- **Strict Temporal Safety**: Because `ttzip_parallel_for` uses a synchronous barrier (`ttzip_threadpool_wait_all`) before returning, the `ParallelForBox` on the caller thread stack is guaranteed to strictly outlive every concurrent worker thread invocation.
- **100% Cross-Platform**: Interacts solely with the C11 `ttzip_threadpool` and standard C ABI, eliminating Apple `libdispatch` lock-in.

### Alternatives Considered
1. **Swift `withDiscardingTaskGroup`**: Requires an `async` context and allocates individual `Task` structures and scheduling frames on the heap, adding latency to microsecond-level block compression loops.
2. **Direct stack pointer dereferencing via `withoutActuallyEscaping` + `UnsafePointer.pointee`**: Invoking `box.pointee` across multiple concurrent threads causes Swift ARC to emit atomic retains on the captured closure buffer on every iteration, destroying multi-core scalability.

### Source
- `Sources/CTTZipBridge/include/ttzip_threadpool.h`
- `Sources/CTTZipBridge/ttzip_threadpool.c`
- `Sources/TTZipCore/Zip/NativeZipEngine.swift` (Unmanaged bridge precedent in TTZip)

---

## Research Item R002: Deadlock-Free Native Synchronous Dual-Dispatch for Template Methods

### Decision
Eliminate the double-inversion async-to-sync bridging pattern (`Task.detached` + `DispatchSemaphore.wait()`) across `BaseArchiveEngineTemplate`, `ZipArchiveEngineTemplate`, `SevenZipArchiveEngineTemplate`, and `TarArchiveEngineTemplate`. Instead, execute underlying synchronous C routines directly in `executeCoreAlgorithm(context:)`. For inspection workflows, invoke synchronous C inspection primitives directly (`ttzip_inspect_archive_v2` / `ArchiveReader.inspectSync`). Remove the dead `executeStrategyBridgeSync` helper in `ArchiveEngineStrategy.swift`.

### Rationale
- **Deadlock Prevention**: Calling `DispatchSemaphore.wait()` on a thread in Swift's fixed-size cooperative thread pool blocks the underlying OS thread. When concurrent callers exhaust the pool, newly spawned `Task.detached` jobs are starved and can never signal the semaphore, resulting in permanent deadlocks (SE-0296, SE-0304).
- **Predictable Stack Execution**: TTZip's core archive creation and extraction engines are in-process C libraries. Executing them directly on the calling thread eliminates thread context switches, heap allocations, and kernel parking latency.

### Alternatives Considered
1. **`NSCondition` / `pthread_cond_wait`**: Suffers from the exact same fatal flaw as `DispatchSemaphore` by blocking cooperative worker threads.
2. **`withCheckedContinuation`**: Continuations only work from synchronous callbacks to `async` callers; they cannot transform an asynchronous call into a synchronous return without blocking.

### Source
- `Sources/TTZipCore/TemplateMethod/BaseArchiveEngineTemplate.swift`
- `Sources/TTZipCore/ArchiveEngineStrategy.swift`
- Swift Evolution SE-0296 & SE-0304 (Cooperative Thread Pool Invariants)
