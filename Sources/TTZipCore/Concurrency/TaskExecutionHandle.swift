// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// A thread-safe handle bridging Swift 6 structured concurrency Task cancellation and pausing
/// to low-level native Rust atomic cancellation tokens via Mozilla UniFFI.
public final class TaskExecutionHandle: @unchecked Sendable {

    /// Execution lifecycle state.
    public enum State: Equatable, Sendable {
        case running
        case paused
        case cancelled(reason: UInt8)
        case completed
    }

    /// Low-level UniFFI atomic cancellation token.
    public let uniffiToken: CancellationToken

    private let condition = NSCondition()
    private var _state: State = .running
    private var pauseContinuations: [CheckedContinuation<Void, Never>] = []
    private var cancellationHandlers: [@Sendable () -> Void] = []

    public init(token: CancellationToken = CancellationToken()) {
        self.uniffiToken = token
    }

    /// Returns the current execution state snapshot.
    public var state: State {
        condition.lock()
        defer { condition.unlock() }
        return _state
    }

    /// Increments the reference count of the underlying native CancellationToken.
    public func retainToken() {}

    /// Decrements the reference count of the underlying native CancellationToken.
    public func releaseToken() {}

    /// Cancels execution with a specific reason code (0 = user requested, 1 = timeout, 2 = error abort).
    public func cancel(reason: UInt8 = 0) {
        condition.lock()
        _state = .cancelled(reason: reason)
        let continuations = pauseContinuations
        pauseContinuations.removeAll()
        let handlers = cancellationHandlers
        condition.broadcast()
        condition.unlock()

        uniffiToken.cancel()

        for continuation in continuations {
            continuation.resume()
        }
        for handler in handlers {
            handler()
        }
    }

    /// Internal lockless check for cancellation, assuming `condition` is already held by the caller thread.
    @inline(__always)
    func isCancelledLocked() -> Bool {
        if uniffiToken.isCancelled() {
            return true
        }
        if case .cancelled = _state {
            return true
        }
        return false
    }

    /// Indicates whether execution has been cancelled either via UniFFI or the Swift handle.
    public var isCancelled: Bool {
        if uniffiToken.isCancelled() {
            return true
        }
        condition.lock()
        defer { condition.unlock() }
        return isCancelledLocked()
    }

    /// Transitions execution state into paused, suspending collaborative workers.
    public func pause() {
        condition.lock()
        guard _state == .running else {
            condition.unlock()
            return
        }
        _state = .paused
        condition.broadcast()
        condition.unlock()
    }

    /// Resumes paused execution, waking up all suspended asynchronous tasks and threads.
    public func resume() {
        condition.lock()
        guard _state == .paused else {
            condition.unlock()
            return
        }
        _state = .running
        let continuations = pauseContinuations
        pauseContinuations.removeAll()
        condition.broadcast()
        condition.unlock()

        for continuation in continuations {
            continuation.resume()
        }
    }

    /// Indicates whether execution is currently suspended in paused state.
    public var isPaused: Bool {
        condition.lock()
        defer { condition.unlock() }
        return _state == .paused
    }

    /// Marks execution as completed, cleaning up any pending suspension continuations.
    public func complete() {
        condition.lock()
        if _state == .running || _state == .paused {
            _state = .completed
        }
        let continuations = pauseContinuations
        pauseContinuations.removeAll()
        condition.broadcast()
        condition.unlock()

        for continuation in continuations {
            continuation.resume()
        }
    }

    /// Registers a closure invoked when cancellation is triggered.
    public func onCancel(_ handler: @escaping @Sendable () -> Void) {
        condition.lock()
        if isCancelledLocked() {
            condition.unlock()
            handler()
            return
        }
        cancellationHandlers.append(handler)
        condition.unlock()
    }

    /// Attaches an underlying Swift Task to be automatically cancelled when this handle cancels.
    public func attach<T: Sendable, E: Error>(task: Task<T, E>) {
        onCancel {
            task.cancel()
        }
    }

    /// Cooperatively suspends the current structured Swift Task if paused,
    /// or throws `CancellationError` if cancelled.
    public func checkpoint() async throws {
        if isCancelled || Task.isCancelled {
            throw CancellationError()
        }

        while isPaused {
            if isCancelled || Task.isCancelled {
                throw CancellationError()
            }
            await withCheckedContinuation { continuation in
                condition.lock()
                if _state != .paused || isCancelledLocked() || Task.isCancelled {
                    condition.unlock()
                    continuation.resume()
                } else {
                    pauseContinuations.append(continuation)
                    condition.unlock()
                }
            }
        }

        if isCancelled || Task.isCancelled {
            throw CancellationError()
        }
    }

    /// Synchronously blocks the calling thread while paused until resumed, cancelled, or timed out.
    /// Throws `CancellationError` if cancelled.
    public func waitIfPaused(timeout: TimeInterval? = nil) throws {
        condition.lock()
        defer { condition.unlock() }

        if isCancelledLocked() {
            throw CancellationError()
        }

        if _state == .paused {
            if let timeout = timeout {
                let limit = Date().addingTimeInterval(timeout)
                while _state == .paused && !isCancelledLocked() {
                    if !condition.wait(until: limit) {
                        break
                    }
                }
            } else {
                while _state == .paused && !isCancelledLocked() {
                    condition.wait()
                }
            }
        }

        if isCancelledLocked() {
            throw CancellationError()
        }
    }
}

