// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
import CTTZipBridge

/// A thread-safe handle bridging Swift 6 structured concurrency Task cancellation
/// to low-level native Rust C-ABI atomic cancellation tokens.
public final class TaskExecutionHandle: @unchecked Sendable {
    private let rawToken: OpaquePointer?
    private let lock = NSLock()
    private var _isCancelled: Bool = false
    private var _isPaused: Bool = false
    
    public init() {
        self.rawToken = ttzip_rust_cancellation_token_new()
    }
    
    deinit {
        if let token = rawToken {
            ttzip_rust_cancellation_token_free(token)
        }
    }
    
    public var tokenPointer: OpaquePointer? {
        rawToken
    }

    /// Increments the reference count of the underlying native CancellationToken.
    public func retainToken() {
        if let token = rawToken {
            ttzip_rust_cancellation_token_retain(token)
        }
    }

    /// Decrements the reference count of the underlying native CancellationToken.
    public func releaseToken() {
        if let token = rawToken {
            ttzip_rust_cancellation_token_free(token)
        }
    }
    
    /// Cancels execution with specific reason code (0 = user requested, 1 = timeout, 2 = error abort).
    public func cancel(reason: UInt8 = 0) {
        lock.lock()
        _isCancelled = true
        _isPaused = false
        lock.unlock()
        
        if let token = rawToken {
            ttzip_rust_cancellation_token_cancel(token, reason)
        }
    }
    
    public var isCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        if _isCancelled { return true }
        if let token = rawToken {
            return ttzip_rust_cancellation_token_is_cancelled(token)
        }
        return false
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
