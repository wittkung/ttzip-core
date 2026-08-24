// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation

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
    
    /// Dispatches intensive C/Rust compute workload on an isolated GCD concurrent queue with cancellation support.
    public func dispatchCompute<T: Sendable>(
        qos: ExecutionQoSProfile = .userInitiated,
        cancellationHandle: TaskExecutionHandle? = nil,
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try Task.checkCancellation()

        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                computeQueue.async(qos: qos.dispatchQoS) {
                    if Task.isCancelled || cancellationHandle?.isCancelled == true {
                        continuation.resume(throwing: CancellationError())
                        return
                    }
                    do {
                        let val = try work()
                        continuation.resume(returning: val)
                    } catch {
                        continuation.resume(throwing: error)
                    }
                }
            }
        } onCancel: {
            cancellationHandle?.cancel()
        }
    }
}
