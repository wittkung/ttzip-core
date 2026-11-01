// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import os

public enum ExecutionQoSProfile: Sendable {
    case interactive
    case userInitiated
    case utility
    case background
    
    public var taskPriority: TaskPriority {
        switch self {
        case .interactive: return .high
        case .userInitiated: return .medium
        case .utility: return .low
        case .background: return .background
        }
    }
    
    public var dispatchQoS: DispatchQoS {
        switch self {
        case .interactive: return .userInteractive
        case .userInitiated: return .userInitiated
        case .utility: return .utility
        case .background: return .background
        }
    }
}

/// Defines whether the caller unbinds early upon cooperative cancellation (pure query)
/// or waits for the underlying worker closure and its cleanup to finish (side-effecting work).
/// In both modes, semaphore permits and worker pool loads remain held until the worker exits.
public enum CancellationCompletionPolicy: Sendable {
    case waitForWorkerCompletion
    case detachCallerOnCancel
}

/// Single-arbitration state machine governing permit/load handoff between the dispatch entry
/// and the worker queue block. Guarantees that permits and worker loads are released
/// at most once and never while the worker block is still queued or executing.
private final class DispatchOwnershipArbitrator: @unchecked Sendable {
    private enum Phase {
        case acquiredNotEnqueued
        case enqueuedOrRunning
        case released
    }

    private let phase = OSAllocatedUnfairLock<Phase>(initialState: .acquiredNotEnqueued)
    private let onRelease: @Sendable () -> Void

    init(onRelease: @escaping @Sendable () -> Void) {
        self.onRelease = onRelease
    }

    /// Transitions ownership from the caller entry to the queued worker block.
    /// Returns `true` if the worker block now owns release responsibility.
    func markEnqueued() -> Bool {
        phase.withLock { current in
            guard current == .acquiredNotEnqueued else { return false }
            current = .enqueuedOrRunning
            return true
        }
    }

    /// Releases permit and worker load if and only if the work was never enqueued.
    func releaseIfNotEnqueued() {
        let shouldRelease = phase.withLock { current -> Bool in
            guard current == .acquiredNotEnqueued else { return false }
            current = .released
            return true
        }
        if shouldRelease {
            onRelease()
        }
    }

    /// Releases permit and worker load from the worker block upon completion or skip.
    func releaseFromWorker() {
        let shouldRelease = phase.withLock { current -> Bool in
            guard current != .released else { return false }
            current = .released
            return true
        }
        if shouldRelease {
            onRelease()
        }
    }
}

/// Thread-safe one-shot continuation resumption coordinator preventing double-resume.
final class ComputeResumptionCoordinator<T: Sendable>: @unchecked Sendable {
    private enum State {
        case idle
        case attached(CheckedContinuation<T, Error>)
        case pendingResult(Result<T, Error>)
        case completed
    }

    private let state = OSAllocatedUnfairLock<State>(initialState: .idle)

    func attach(_ continuation: CheckedContinuation<T, Error>) {
        let immediateResult: Result<T, Error>? = state.withLock { s in
            switch s {
            case .idle:
                s = .attached(continuation)
                return nil
            case .pendingResult(let res):
                s = .completed
                return res
            case .completed:
                return .failure(CancellationError())
            case .attached:
                return nil
            }
        }
        if let immediateResult {
            continuation.resume(with: immediateResult)
        }
    }

    var hasResult: Bool {
        state.withLock { state in
            switch state {
            case .pendingResult, .completed: return true
            case .idle, .attached: return false
            }
        }
    }

    func cancel() {
        resume(throwing: CancellationError())
    }

    func resume(returning value: T) {
        let contToResume: CheckedContinuation<T, Error>? = state.withLock { s in
            switch s {
            case .idle:
                s = .pendingResult(.success(value))
                return nil
            case .attached(let continuation):
                s = .completed
                return continuation
            case .pendingResult, .completed:
                return nil
            }
        }
        contToResume?.resume(returning: value)
    }

    func resume(throwing error: Error) {
        let contToResume: CheckedContinuation<T, Error>? = state.withLock { s in
            switch s {
            case .idle:
                s = .pendingResult(.failure(error))
                return nil
            case .attached(let continuation):
                s = .completed
                return continuation
            case .pendingResult, .completed:
                return nil
            }
        }
        contToResume?.resume(throwing: error)
    }
}

/// Dedicated compute dispatch coordinator isolated from the Swift 6 cooperative thread pool,
/// backed by a fixed-size pool of dedicated serial worker queues aligned to hardware P-cores (performance cores).
///
/// Bounding active OS threads prevents thread pool explosion, Rayon context switch storms,
/// and CPU starvation of the MainActor/UI thread.
public final class NativeComputeDispatcher: @unchecked Sendable {
    public static let shared = NativeComputeDispatcher()
    
    /// Determines the optimal worker pool size aligned to physical performance cores (P-cores)
    /// or active processor cores, strictly bounded within [2, 16].
    public static func defaultWorkerPoolSize() -> Int {
        #if canImport(Darwin)
        var pCores: Int32 = 0
        var size = MemoryLayout<Int32>.size
        if sysctlbyname("hw.perflevel0.physicalcpu", &pCores, &size, nil, 0) == 0, pCores > 0 {
            return min(16, max(2, Int(pCores)))
        }
        #endif
        let fallback = ProcessInfo.processInfo.activeProcessorCount
        return min(16, max(2, fallback))
    }
    
    /// Maximum concurrent native compute tasks bounded by hardware P-core processor count.
    public let maxConcurrency: Int
    private let semaphore: AsyncSemaphore
    
    /// Dedicated fixed-size pool of serial worker queues.
    /// Each queue is strictly serial, guaranteeing at most 1 OS thread per worker,
    /// preventing thread explosion, lock contention, and UI starvation.
    private let workerQueues: [DispatchQueue]
    
    /// Dedicated I/O serial queue for disk-bound operations, isolated from compute workers.
    private let ioQueue: DispatchQueue
    
    /// Dedicated high-priority serial queue for interactive UI and metadata queries.
    private let highPriorityQueue: DispatchQueue
    
    /// Load tracking and round-robin coordinator for worker pool distribution.
    private struct WorkerPoolState {
        var activeLoads: [Int]
        var roundRobinCursor: Int
    }
    private let poolState: OSAllocatedUnfairLock<WorkerPoolState>

    /// Backward-compatible accessor for an underlying worker queue from the pool.
    public var computeQueue: DispatchQueue {
        acquireLeastLoadedWorker().queue
    }
    
    private let onResourcesReleased: (@Sendable () -> Void)?
    private let onCancellationHandled: (@Sendable (Bool) -> Void)?
    private let beforeEnqueue: (@Sendable () throws -> Void)?

    public convenience init(maxConcurrency: Int = NativeComputeDispatcher.defaultWorkerPoolSize()) {
        self.init(maxConcurrency: maxConcurrency, waiterHooks: .init())
    }

    package init(
        maxConcurrency: Int,
        waiterHooks: AsyncSemaphore.WaiterHooks,
        onResourcesReleased: (@Sendable () -> Void)? = nil,
        onCancellationHandled: (@Sendable (Bool) -> Void)? = nil,
        beforeEnqueue: (@Sendable () throws -> Void)? = nil
    ) {
        let boundedSize = min(16, max(2, maxConcurrency))
        self.maxConcurrency = boundedSize
        self.semaphore = AsyncSemaphore(value: boundedSize, waiterHooks: waiterHooks)
        self.onResourcesReleased = onResourcesReleased
        self.onCancellationHandled = onCancellationHandled
        self.beforeEnqueue = beforeEnqueue
        
        self.workerQueues = (0..<boundedSize).map { index in
            DispatchQueue(
                label: "com.metastudyline.ttzip.compute-worker.\(index)",
                qos: .userInitiated,
                autoreleaseFrequency: .workItem
            )
        }
        
        self.ioQueue = DispatchQueue(
            label: "com.metastudyline.ttzip.io-worker",
            qos: .utility,
            autoreleaseFrequency: .workItem
        )
        
        self.highPriorityQueue = DispatchQueue(
            label: "com.metastudyline.ttzip.high-priority-worker",
            qos: .userInteractive,
            autoreleaseFrequency: .workItem
        )
        
        self.poolState = OSAllocatedUnfairLock(initialState: WorkerPoolState(
            activeLoads: Array(repeating: 0, count: boundedSize),
            roundRobinCursor: 0
        ))
    }
    
    /// Selects the least-loaded serial worker queue, breaking ties using round-robin distribution.
    private func acquireLeastLoadedWorker() -> (index: Int, queue: DispatchQueue) {
        poolState.withLock { s in
            let count = s.activeLoads.count
            var minIndex = s.roundRobinCursor % count
            var minLoad = s.activeLoads[minIndex]
            
            for offset in 0..<count {
                let candidate = (s.roundRobinCursor + offset) % count
                if s.activeLoads[candidate] < minLoad {
                    minLoad = s.activeLoads[candidate]
                    minIndex = candidate
                }
            }
            
            s.activeLoads[minIndex] += 1
            s.roundRobinCursor = (s.roundRobinCursor + 1) % count
            return (minIndex, workerQueues[minIndex])
        }
    }
    
    /// Decrements the active load counter for the specified worker queue.
    private func releaseWorker(at index: Int) {
        poolState.withLock { s in
            if s.activeLoads[index] > 0 {
                s.activeLoads[index] -= 1
            }
        }
    }

    package var currentActiveLoads: [Int] {
        poolState.withLock { $0.activeLoads }
    }

    package var currentAvailablePermits: Int {
        semaphore.currentAvailablePermits
    }
    
    // MARK: - Compute Dispatch (CPU-bound)
    
    /// Dispatches intensive C/Rust compute workload across the fixed-size serial worker queue pool
    /// aligned to hardware P-cores, using load-balanced round-robin distribution with atomic cancellation flag support
    /// and strict concurrency bounds.
    public func dispatchCompute<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        cancellationPolicy: CancellationCompletionPolicy = .detachCallerOnCancel,
        _ work: @escaping @Sendable (_ isCancelled: OSAllocatedUnfairLock<Bool>) throws -> T
    ) async throws -> T {
        try Task.checkCancellation()

        try await semaphore.wait()

        let effectiveHandle = cancellationHandle ?? TaskExecutionHandle()
        let isCancelledFlag = OSAllocatedUnfairLock(initialState: false)
        let coordinator = ComputeResumptionCoordinator<T>()
        let (workerIndex, targetQueue) = acquireLeastLoadedWorker()

        let arbitrator = DispatchOwnershipArbitrator { [weak self, weak semaphore] in
            self?.releaseWorker(at: workerIndex)
            semaphore?.signal()
            self?.onResourcesReleased?()
        }
        defer {
            arbitrator.releaseIfNotEnqueued()
        }

        if let hook = beforeEnqueue {
            try hook()
        }
        if Task.isCancelled || effectiveHandle.isCancelled {
            effectiveHandle.cancel()
            throw CancellationError()
        }

        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                coordinator.attach(continuation)

                guard arbitrator.markEnqueued() else {
                    coordinator.cancel()
                    return
                }

                targetQueue.async(qos: qos.dispatchQoS) {
                    defer {
                        arbitrator.releaseFromWorker()
                    }
                    if effectiveHandle.isCancelled || isCancelledFlag.withLock({ $0 }) {
                        coordinator.cancel()
                        return
                    }
                    do {
                        let val = try work(isCancelledFlag)
                        coordinator.resume(returning: val)
                    } catch {
                        coordinator.resume(throwing: error)
                    }
                }
            }
        } onCancel: {
            isCancelledFlag.withLock { $0 = true }
            effectiveHandle.cancel()
            if cancellationPolicy == .detachCallerOnCancel {
                coordinator.cancel()
            }
            onCancellationHandled?(coordinator.hasResult)
        }
    }

    /// Dispatches intensive C/Rust compute workload with direct UniFFI CancellationToken cancellation token.
    public func dispatchCompute<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        cancellationPolicy: CancellationCompletionPolicy = .detachCallerOnCancel,
        _ work: @escaping @Sendable (_ token: CancellationToken) throws -> T
    ) async throws -> T {
        let effectiveHandle = cancellationHandle ?? TaskExecutionHandle()
        return try await dispatchCompute(
            qos: qos,
            cancellationHandle: effectiveHandle,
            cancellationPolicy: cancellationPolicy
        ) { (_: OSAllocatedUnfairLock<Bool>) in
            try work(effectiveHandle.uniffiToken)
        }
    }

    /// Dispatches compute block with standard 0-argument closure.
    public func dispatchCompute<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        cancellationPolicy: CancellationCompletionPolicy = .detachCallerOnCancel,
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try await dispatchCompute(
            qos: qos,
            cancellationHandle: cancellationHandle,
            cancellationPolicy: cancellationPolicy
        ) { (_: OSAllocatedUnfairLock<Bool>) in
            try work()
        }
    }

    // MARK: - I/O Dispatch (Disk-bound)

    /// Dispatches disk-bound I/O workload on the dedicated serial I/O queue,
    /// completely isolated from CPU-bound compute workers to prevent thread contention.
    public func dispatchIO<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        cancellationPolicy: CancellationCompletionPolicy = .detachCallerOnCancel,
        _ work: @escaping @Sendable (_ isCancelled: OSAllocatedUnfairLock<Bool>) throws -> T
    ) async throws -> T {
        try Task.checkCancellation()

        let effectiveHandle = cancellationHandle ?? TaskExecutionHandle()
        let isCancelledFlag = OSAllocatedUnfairLock(initialState: false)
        let coordinator = ComputeResumptionCoordinator<T>()

        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                coordinator.attach(continuation)

                ioQueue.async {
                    if effectiveHandle.isCancelled || isCancelledFlag.withLock({ $0 }) {
                        coordinator.cancel()
                        return
                    }
                    do {
                        let val = try work(isCancelledFlag)
                        coordinator.resume(returning: val)
                    } catch {
                        coordinator.resume(throwing: error)
                    }
                }
            }
        } onCancel: {
            isCancelledFlag.withLock { $0 = true }
            effectiveHandle.cancel()
            if cancellationPolicy == .detachCallerOnCancel {
                coordinator.cancel()
            }
        }
    }

    /// Dispatches disk-bound I/O workload with direct UniFFI CancellationToken.
    public func dispatchIO<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        cancellationPolicy: CancellationCompletionPolicy = .detachCallerOnCancel,
        _ work: @escaping @Sendable (_ token: CancellationToken) throws -> T
    ) async throws -> T {
        let effectiveHandle = cancellationHandle ?? TaskExecutionHandle()
        return try await dispatchIO(
            cancellationHandle: effectiveHandle,
            cancellationPolicy: cancellationPolicy
        ) { (_: OSAllocatedUnfairLock<Bool>) in
            try work(effectiveHandle.uniffiToken)
        }
    }

    /// Dispatches disk-bound I/O workload with standard 0-argument closure.
    public func dispatchIO<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        cancellationPolicy: CancellationCompletionPolicy = .detachCallerOnCancel,
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try await dispatchIO(
            cancellationHandle: cancellationHandle,
            cancellationPolicy: cancellationPolicy
        ) { (_: OSAllocatedUnfairLock<Bool>) in
            try work()
        }
    }

    // MARK: - High Priority Dispatch (Interactive)

    /// Dispatches time-critical or interactive native compute workload on the dedicated high-priority queue,
    /// prioritizing UI-facing inspection, archive header decoding, and password verification.
    public func dispatchHighPriority<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        cancellationPolicy: CancellationCompletionPolicy = .detachCallerOnCancel,
        _ work: @escaping @Sendable (_ isCancelled: OSAllocatedUnfairLock<Bool>) throws -> T
    ) async throws -> T {
        try Task.checkCancellation()

        let effectiveHandle = cancellationHandle ?? TaskExecutionHandle()
        let isCancelledFlag = OSAllocatedUnfairLock(initialState: false)
        let coordinator = ComputeResumptionCoordinator<T>()

        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                coordinator.attach(continuation)

                highPriorityQueue.async {
                    if effectiveHandle.isCancelled || isCancelledFlag.withLock({ $0 }) {
                        coordinator.cancel()
                        return
                    }
                    do {
                        let val = try work(isCancelledFlag)
                        coordinator.resume(returning: val)
                    } catch {
                        coordinator.resume(throwing: error)
                    }
                }
            }
        } onCancel: {
            isCancelledFlag.withLock { $0 = true }
            effectiveHandle.cancel()
            if cancellationPolicy == .detachCallerOnCancel {
                coordinator.cancel()
            }
        }
    }

    /// Dispatches high-priority workload with direct UniFFI CancellationToken.
    public func dispatchHighPriority<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        cancellationPolicy: CancellationCompletionPolicy = .detachCallerOnCancel,
        _ work: @escaping @Sendable (_ token: CancellationToken) throws -> T
    ) async throws -> T {
        let effectiveHandle = cancellationHandle ?? TaskExecutionHandle()
        return try await dispatchHighPriority(
            cancellationHandle: effectiveHandle,
            cancellationPolicy: cancellationPolicy
        ) { (_: OSAllocatedUnfairLock<Bool>) in
            try work(effectiveHandle.uniffiToken)
        }
    }

    /// Dispatches high-priority workload with standard 0-argument closure.
    public func dispatchHighPriority<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        cancellationPolicy: CancellationCompletionPolicy = .detachCallerOnCancel,
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try await dispatchHighPriority(
            cancellationHandle: cancellationHandle,
            cancellationPolicy: cancellationPolicy
        ) { (_: OSAllocatedUnfairLock<Bool>) in
            try work()
        }
    }

    // MARK: - Blocking Execution Compatibility

    /// Executes an arbitrary blocking synchronous closure directly on a dedicated compute worker queue,
    /// strictly isolated from the Swift 6 cooperative thread pool.
    public func executeBlocking<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try await dispatchCompute(qos: qos, cancellationHandle: cancellationHandle) { (_: OSAllocatedUnfairLock<Bool>) in
            try work()
        }
    }

    /// Executes a synchronous blocking workload with cancellation flag inspection on a dedicated compute worker queue.
    public func executeBlocking<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable (_ isCancelled: OSAllocatedUnfairLock<Bool>) throws -> T
    ) async throws -> T {
        try await dispatchCompute(qos: qos, cancellationHandle: cancellationHandle, work)
    }

    /// Executes a synchronous blocking workload with direct UniFFI CancellationToken on a dedicated compute worker queue.
    public func executeBlocking<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable (_ token: CancellationToken) throws -> T
    ) async throws -> T {
        try await dispatchCompute(qos: qos, cancellationHandle: cancellationHandle, work)
    }

    // MARK: - Static Ergonomic Helpers

    /// Static convenience for dispatching compute on the shared compute dispatcher.
    public static func dispatchCompute<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try await shared.dispatchCompute(qos: qos, cancellationHandle: cancellationHandle, work)
    }

    /// Static convenience for dispatching compute with UniFFI cancellation token on the shared compute dispatcher.
    public static func dispatchCompute<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable (_ token: CancellationToken) throws -> T
    ) async throws -> T {
        try await shared.dispatchCompute(qos: qos, cancellationHandle: cancellationHandle, work)
    }

    /// Static convenience for dispatching compute with cancellation flag on the shared compute dispatcher.
    public static func dispatchCompute<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable (_ isCancelled: OSAllocatedUnfairLock<Bool>) throws -> T
    ) async throws -> T {
        try await shared.dispatchCompute(qos: qos, cancellationHandle: cancellationHandle, work)
    }

    /// Static convenience for dispatching I/O on the shared compute dispatcher.
    public static func dispatchIO<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try await shared.dispatchIO(cancellationHandle: cancellationHandle, work)
    }

    /// Static convenience for dispatching I/O with UniFFI cancellation token on the shared compute dispatcher.
    public static func dispatchIO<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable (_ token: CancellationToken) throws -> T
    ) async throws -> T {
        try await shared.dispatchIO(cancellationHandle: cancellationHandle, work)
    }

    /// Static convenience for dispatching high-priority work on the shared compute dispatcher.
    public static func dispatchHighPriority<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try await shared.dispatchHighPriority(cancellationHandle: cancellationHandle, work)
    }

    /// Static convenience for dispatching high-priority work with UniFFI cancellation token on the shared compute dispatcher.
    public static func dispatchHighPriority<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable (_ token: CancellationToken) throws -> T
    ) async throws -> T {
        try await shared.dispatchHighPriority(cancellationHandle: cancellationHandle, work)
    }

    /// Static convenience for executing blocking work on the shared compute dispatcher.
    public static func executeBlocking<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try await shared.executeBlocking(qos: qos, cancellationHandle: cancellationHandle, work)
    }

    /// Static convenience for executing blocking work with UniFFI cancellation token on the shared compute dispatcher.
    public static func executeBlocking<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable (_ token: CancellationToken) throws -> T
    ) async throws -> T {
        try await shared.executeBlocking(qos: qos, cancellationHandle: cancellationHandle, work)
    }
}

