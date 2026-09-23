// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

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

/// Dedicated compute dispatch queue isolated from Swift 6 cooperative thread pool.
public final class NativeComputeDispatcher: @unchecked Sendable {
    public static let shared = NativeComputeDispatcher()
    
    private let computeQueue = DispatchQueue(
        label: "org.ttzip.native.compute",
        qos: .userInitiated,
        attributes: .concurrent
    )
    
    private init() {}
    
    /// Dispatches intensive C/Rust compute workload on an isolated GCD concurrent queue with atomic cancellation flag support.
    public func dispatchCompute<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable (_ isCancelled: OSAllocatedUnfairLock<Bool>) throws -> T
    ) async throws -> T {
        try Task.checkCancellation()

        let effectiveHandle = cancellationHandle ?? TaskExecutionHandle()
        let isCancelledFlag = OSAllocatedUnfairLock(initialState: false)

        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                computeQueue.async(qos: qos.dispatchQoS) {
                    if Task.isCancelled || effectiveHandle.isCancelled || isCancelledFlag.withLock({ $0 }) {
                        continuation.resume(throwing: CancellationError())
                        return
                    }
                    do {
                        let val = try work(isCancelledFlag)
                        continuation.resume(returning: val)
                    } catch {
                        continuation.resume(throwing: error)
                    }
                }
            }
        } onCancel: {
            isCancelledFlag.withLock { $0 = true }
            effectiveHandle.cancel()
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
}
