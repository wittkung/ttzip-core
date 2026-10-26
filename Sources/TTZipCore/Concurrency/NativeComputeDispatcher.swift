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

/// Thread-safe cooperative async semaphore for strictly bounding concurrent native compute tasks.
public final class AsyncSemaphore: @unchecked Sendable {
    private struct State {
        var value: Int
        var waiters: [UUID: CheckedContinuation<Bool, Never>] = [:]
        var waiterOrder: [UUID] = []
    }
    
    private let state: OSAllocatedUnfairLock<State>
    
    public init(value: Int) {
        self.state = OSAllocatedUnfairLock(initialState: State(value: max(1, value)))
    }
    
    /// Acquires a concurrency token, cooperatively suspending if all tokens are occupied.
    public func wait() async throws {
        try Task.checkCancellation()
        
        let waiterId = UUID()
        let immediateAcquired: Bool = state.withLock { s in
            if s.value > 0 {
                s.value -= 1
                return true
            }
            return false
        }
        
        if immediateAcquired {
            return
        }
        
        let acquired = await withTaskCancellationHandler {
            await withCheckedContinuation { (continuation: CheckedContinuation<Bool, Never>) in
                state.withLock { s in
                    if s.value > 0 {
                        s.value -= 1
                        continuation.resume(returning: true)
                    } else {
                        s.waiters[waiterId] = continuation
                        s.waiterOrder.append(waiterId)
                    }
                }
            }
        } onCancel: {
            let cont: CheckedContinuation<Bool, Never>? = state.withLock { s in
                s.waiterOrder.removeAll { $0 == waiterId }
                return s.waiters.removeValue(forKey: waiterId)
            }
            cont?.resume(returning: false)
        }
        
        if !acquired {
            throw CancellationError()
        }
        
        if Task.isCancelled {
            signal()
            throw CancellationError()
        }
    }
    
    /// Releases a concurrency token and transfers it to the next waiting task.
    public func signal() {
        let toResume: CheckedContinuation<Bool, Never>? = state.withLock { s in
            while !s.waiterOrder.isEmpty {
                let nextId = s.waiterOrder.removeFirst()
                if let cont = s.waiters.removeValue(forKey: nextId) {
                    return cont
                }
            }
            s.value += 1
            return nil
        }
        toResume?.resume(returning: true)
    }
}

/// Thread-safe one-shot continuation resumption coordinator preventing double-resume
/// while guaranteeing instantaneous continuation resumption upon cooperative task cancellation.
final class ComputeResumptionCoordinator<T: Sendable>: @unchecked Sendable {
    private enum State {
        case idle
        case attached(CheckedContinuation<T, Error>)
        case completed
    }

    private let state = OSAllocatedUnfairLock<State>(initialState: .idle)

    func attach(_ continuation: CheckedContinuation<T, Error>) {
        let contToCancel: CheckedContinuation<T, Error>? = state.withLock { s in
            switch s {
            case .idle:
                s = .attached(continuation)
                return nil
            case .completed:
                return continuation
            case .attached:
                return nil
            }
        }
        contToCancel?.resume(throwing: CancellationError())
    }

    func cancel() {
        let contToResume: CheckedContinuation<T, Error>? = state.withLock { s in
            switch s {
            case .idle:
                s = .completed
                return nil
            case .attached(let continuation):
                s = .completed
                return continuation
            case .completed:
                return nil
            }
        }
        contToResume?.resume(throwing: CancellationError())
    }

    func resume(returning value: T) {
        let contToResume: CheckedContinuation<T, Error>? = state.withLock { s in
            if case let .attached(continuation) = s {
                s = .completed
                return continuation
            }
            return nil
        }
        contToResume?.resume(returning: value)
    }

    func resume(throwing error: Error) {
        let contToResume: CheckedContinuation<T, Error>? = state.withLock { s in
            if case let .attached(continuation) = s {
                s = .completed
                return continuation
            }
            return nil
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
    
    public init(maxConcurrency: Int = NativeComputeDispatcher.defaultWorkerPoolSize()) {
        let boundedSize = min(16, max(2, maxConcurrency))
        self.maxConcurrency = boundedSize
        self.semaphore = AsyncSemaphore(value: boundedSize)
        
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
    
    // MARK: - Compute Dispatch (CPU-bound)
    
    /// Dispatches intensive C/Rust compute workload across the fixed-size serial worker queue pool
    /// aligned to hardware P-cores, using load-balanced round-robin distribution with atomic cancellation flag support
    /// and strict concurrency bounds.
    public func dispatchCompute<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable (_ isCancelled: OSAllocatedUnfairLock<Bool>) throws -> T
    ) async throws -> T {
        try Task.checkCancellation()

        try await semaphore.wait()

        let effectiveHandle = cancellationHandle ?? TaskExecutionHandle()
        let isCancelledFlag = OSAllocatedUnfairLock(initialState: false)
        let coordinator = ComputeResumptionCoordinator<T>()
        let semaphoreSignalled = OSAllocatedUnfairLock(initialState: false)

        let releaseSemaphore: @Sendable () -> Void = { [weak semaphore] in
            let shouldSignal = semaphoreSignalled.withLock { signalled in
                if !signalled {
                    signalled = true
                    return true
                }
                return false
            }
            if shouldSignal {
                semaphore?.signal()
            }
        }

        let (workerIndex, targetQueue) = acquireLeastLoadedWorker()
        let workerReleased = OSAllocatedUnfairLock(initialState: false)
        let releaseWorkerLoad: @Sendable () -> Void = { [weak self] in
            let shouldRelease = workerReleased.withLock { released in
                if !released {
                    released = true
                    return true
                }
                return false
            }
            if shouldRelease {
                self?.releaseWorker(at: workerIndex)
            }
        }

        defer {
            releaseWorkerLoad()
            releaseSemaphore()
        }

        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                coordinator.attach(continuation)

                targetQueue.async(qos: qos.dispatchQoS) {
                    defer {
                        releaseWorkerLoad()
                        releaseSemaphore()
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
            coordinator.cancel()
        }
    }

    /// Dispatches intensive C/Rust compute workload with direct UniFFI CancellationToken cancellation token.
    public func dispatchCompute<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable (_ token: CancellationToken) throws -> T
    ) async throws -> T {
        let effectiveHandle = cancellationHandle ?? TaskExecutionHandle()
        return try await dispatchCompute(qos: qos, cancellationHandle: effectiveHandle) { (_: OSAllocatedUnfairLock<Bool>) in
            try work(effectiveHandle.uniffiToken)
        }
    }

    /// Dispatches compute block with standard 0-argument closure.
    public func dispatchCompute<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try await dispatchCompute(qos: qos, cancellationHandle: cancellationHandle) { (_: OSAllocatedUnfairLock<Bool>) in
            try work()
        }
    }

    // MARK: - I/O Dispatch (Disk-bound)

    /// Dispatches disk-bound I/O workload on the dedicated serial I/O queue,
    /// completely isolated from CPU-bound compute workers to prevent thread contention.
    public func dispatchIO<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
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
            coordinator.cancel()
        }
    }

    /// Dispatches disk-bound I/O workload with direct UniFFI CancellationToken.
    public func dispatchIO<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable (_ token: CancellationToken) throws -> T
    ) async throws -> T {
        let effectiveHandle = cancellationHandle ?? TaskExecutionHandle()
        return try await dispatchIO(cancellationHandle: effectiveHandle) { (_: OSAllocatedUnfairLock<Bool>) in
            try work(effectiveHandle.uniffiToken)
        }
    }

    /// Dispatches disk-bound I/O workload with standard 0-argument closure.
    public func dispatchIO<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try await dispatchIO(cancellationHandle: cancellationHandle) { (_: OSAllocatedUnfairLock<Bool>) in
            try work()
        }
    }

    // MARK: - High Priority Dispatch (Interactive)

    /// Dispatches time-critical or interactive native compute workload on the dedicated high-priority queue,
    /// prioritizing UI-facing inspection, archive header decoding, and password verification.
    public func dispatchHighPriority<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
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
            coordinator.cancel()
        }
    }

    /// Dispatches high-priority workload with direct UniFFI CancellationToken.
    public func dispatchHighPriority<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable (_ token: CancellationToken) throws -> T
    ) async throws -> T {
        let effectiveHandle = cancellationHandle ?? TaskExecutionHandle()
        return try await dispatchHighPriority(cancellationHandle: effectiveHandle) { (_: OSAllocatedUnfairLock<Bool>) in
            try work(effectiveHandle.uniffiToken)
        }
    }

    /// Dispatches high-priority workload with standard 0-argument closure.
    public func dispatchHighPriority<T: Sendable>(
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try await dispatchHighPriority(cancellationHandle: cancellationHandle) { (_: OSAllocatedUnfairLock<Bool>) in
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

