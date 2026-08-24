// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation

/// A thread-safe, heap-allocated reference wrapper for bridging Swift closures
/// across C-ABI void* context pointers without triggering pointer decay, stack escape,
/// or undefined behavior due to Swift closure memory layouts.
public final class ClosureBox<T: Sendable>: @unchecked Sendable {
    public let closure: T

    @inline(__always)
    public init(_ closure: T) {
        self.closure = closure
    }
}
