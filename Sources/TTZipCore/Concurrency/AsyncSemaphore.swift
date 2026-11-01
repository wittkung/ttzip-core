// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import os

/// Cancellation-aware semaphore; waiter registration and cancellation share one lock.
public final class AsyncSemaphore: @unchecked Sendable {
    package struct WaiterHooks: Sendable {
        let beforeRegistration: (@Sendable () -> Void)?
        let didRegister: (@Sendable () -> Void)?
        let didCancel: (@Sendable () -> Void)?

        package init(
            beforeRegistration: (@Sendable () -> Void)? = nil,
            didRegister: (@Sendable () -> Void)? = nil,
            didCancel: (@Sendable () -> Void)? = nil
        ) {
            self.beforeRegistration = beforeRegistration
            self.didRegister = didRegister
            self.didCancel = didCancel
        }
    }

    /// Every access is protected by this semaphore's state lock, including cancellation.
    private final class WaiterCancellation: @unchecked Sendable {
        var isCancelled = false
    }

    private struct State {
        var value: Int
        var waiters: [UUID: CheckedContinuation<Bool, Never>] = [:]
        var waiterOrder: [UUID] = []
    }

    private let state: OSAllocatedUnfairLock<State>
    private let waiterHooks: WaiterHooks

    public convenience init(value: Int) {
        self.init(value: value, waiterHooks: WaiterHooks())
    }

    package init(value: Int, waiterHooks: WaiterHooks) {
        self.state = OSAllocatedUnfairLock(initialState: State(value: max(1, value)))
        self.waiterHooks = waiterHooks
    }

    public func wait() async throws {
        try Task.checkCancellation()
        let waiterID = UUID()
        let immediate = state.withLock { state -> Bool in
            guard state.value > 0 else { return false }
            state.value -= 1
            return true
        }
        if immediate { return }

        let cancellation = WaiterCancellation()
        let acquired = await withTaskCancellationHandler {
            await withCheckedContinuation { (continuation: CheckedContinuation<Bool, Never>) in
                waiterHooks.beforeRegistration?()
                let registered = state.withLock { state -> Bool in
                    guard !cancellation.isCancelled else {
                        continuation.resume(returning: false)
                        return false
                    }
                    if state.value > 0 {
                        state.value -= 1
                        continuation.resume(returning: true)
                        return false
                    }
                    state.waiters[waiterID] = continuation
                    state.waiterOrder.append(waiterID)
                    return true
                }
                if registered { waiterHooks.didRegister?() }
            }
        } onCancel: {
            let continuation = state.withLock { state in
                cancellation.isCancelled = true
                state.waiterOrder.removeAll { $0 == waiterID }
                return state.waiters.removeValue(forKey: waiterID)
            }
            continuation?.resume(returning: false)
            waiterHooks.didCancel?()
        }
        guard acquired else { throw CancellationError() }
        if Task.isCancelled {
            signal()
            throw CancellationError()
        }
    }

    public func signal() {
        let continuation = state.withLock { state -> CheckedContinuation<Bool, Never>? in
            while !state.waiterOrder.isEmpty {
                let id = state.waiterOrder.removeFirst()
                if let waiter = state.waiters.removeValue(forKey: id) { return waiter }
            }
            state.value += 1
            return nil
        }
        continuation?.resume(returning: true)
    }

    package var currentAvailablePermits: Int { state.withLock { $0.value } }
    package var pendingWaiterCount: Int { state.withLock { $0.waiters.count } }
}
