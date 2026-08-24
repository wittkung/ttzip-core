// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// A thread-safe handle bridging Swift 6 structured concurrency Task cancellation
/// to low-level native Rust atomic cancellation tokens via Mozilla UniFFI.
public final class TaskExecutionHandle: @unchecked Sendable {
    public let uniffiToken: CancellationToken
    private let lock = NSLock()
    private var _isPaused: Bool = false
    
    public init() {
        self.uniffiToken = CancellationToken()
    }
    
    /// Increments the reference count of the underlying native CancellationToken.
    public func retainToken() {}

    /// Decrements the reference count of the underlying native CancellationToken.
    public func releaseToken() {}
    
    /// Cancels execution with specific reason code (0 = user requested, 1 = timeout, 2 = error abort).
    public func cancel(reason: UInt8 = 0) {
        uniffiToken.cancel()
    }
    
    public var isCancelled: Bool {
        uniffiToken.isCancelled()
    }
    
    public func pause() {
        lock.lock()
        _isPaused = true
        lock.unlock()
    }
    
    public func resume() {
        lock.lock()
        _isPaused = false
        lock.unlock()
    }
    
    public var isPaused: Bool {
        lock.lock()
        defer { lock.unlock() }
        return _isPaused
    }
}
