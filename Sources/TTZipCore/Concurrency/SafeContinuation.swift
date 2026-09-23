// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation
import os

/// Thread-safe wrapper around `CheckedContinuation` that guarantees strictly-once resumption.
/// Concurrent or redundant resume attempts are atomically discarded, preventing `SIGTRAP` runtime crashes.
public final class SafeContinuation<T: Sendable, E: Error>: @unchecked Sendable {
    private let state: OSAllocatedUnfairLock<Bool>
    private let continuation: CheckedContinuation<T, E>

    public init(_ continuation: CheckedContinuation<T, E>) {
        self.state = OSAllocatedUnfairLock(initialState: false)
        self.continuation = continuation
    }

    /// Resumes the continuation returning `value`, if it has not already been resumed.
    /// - Returns: `true` if this invocation won the race and resumed the continuation; otherwise `false`.
    @discardableResult
    public func resume(returning value: T) -> Bool {
        let won = state.withLock { resumed in
            if resumed { return false }
            resumed = true
            return true
        }
        if won {
            continuation.resume(returning: value)
        }
        return won
    }

    /// Resumes the continuation throwing `error`, if it has not already been resumed.
    /// - Returns: `true` if this invocation won the race and resumed the continuation; otherwise `false`.
    @discardableResult
    public func resume(throwing error: E) -> Bool {
        let won = state.withLock { resumed in
            if resumed { return false }
            resumed = true
            return true
        }
        if won {
            continuation.resume(throwing: error)
        }
        return won
    }

    /// Resumes the continuation with a `Result`, if it has not already been resumed.
    @discardableResult
    public func resume(with result: Result<T, E>) -> Bool {
        switch result {
        case .success(let value):
            return resume(returning: value)
        case .failure(let error):
            return resume(throwing: error)
        }
    }

    /// Checks if the continuation has already been resumed.
    public var isResumed: Bool {
        state.withLock { $0 }
    }
}

private final class TimeoutContinuationContext<T: Sendable>: @unchecked Sendable {
    let safeContinuation = OSAllocatedUnfairLock<SafeContinuation<T, Never>?>(initialState: nil)
    let timerTask = OSAllocatedUnfairLock<Task<Void, Never>?>(initialState: nil)
}

/// Executes an async block with continuation, enforcing a strict timeout and cooperative task cancellation.
/// Guarantees that the continuation resumes exactly once with either the produced value or `fallbackOnTimeout`.
public func withSafeTimeoutContinuation<T: Sendable>(
    seconds: TimeInterval,
    fallbackOnTimeout: T,
    operation: String = "AsyncOperation",
    _ body: @escaping @Sendable (SafeContinuation<T, Never>) -> Void
) async -> T {
    if Task.isCancelled {
        return fallbackOnTimeout
    }

    let ctx = TimeoutContinuationContext<T>()

    return await withTaskCancellationHandler {
        await withCheckedContinuation { (rawContinuation: CheckedContinuation<T, Never>) in
            let safe = SafeContinuation(rawContinuation)
            ctx.safeContinuation.withLock { $0 = safe }

            let timer = Task {
                try? await Task.sleep(nanoseconds: UInt64(max(0.001, seconds) * 1_000_000_000))
                _ = safe.resume(returning: fallbackOnTimeout)
            }
            ctx.timerTask.withLock { $0 = timer }

            body(safe)
        }
    } onCancel: {
        ctx.timerTask.withLock { $0?.cancel() }
        _ = ctx.safeContinuation.withLock { $0 }?.resume(returning: fallbackOnTimeout)
    }
}
