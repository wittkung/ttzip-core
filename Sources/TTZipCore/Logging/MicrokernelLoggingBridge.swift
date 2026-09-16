// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import os
import TTLogKit

/// UniFFI log callback adapter routing Rust microkernel log records into TTLogEngine.
internal final class MicrokernelLogHandler: UniFfiLogCallback, @unchecked Sendable {
    nonisolated func log(level: UInt32, target: String, message: String, file: String, line: UInt32) {
        let mappedLevel: TTLogLevel
        switch level {
        case 0:
            mappedLevel = .debug
        case 1:
            mappedLevel = .info
        case 2:
            mappedLevel = .warning
        case 3:
            mappedLevel = .error
        default:
            mappedLevel = .info
        }
        let formattedMessage = target.isEmpty ? message : "[\(target)] \(message)"
        TTLogEngine.shared.log(
            level: mappedLevel,
            category: .kernel,
            message: formattedMessage,
            file: file.isEmpty ? "kernel" : file,
            line: UInt(line)
        )
    }
}

private let isBridgeConfigured = OSAllocatedUnfairLock(initialState: false)

/// Enables the UniFFI logging bridge between the Rust microkernel and TTLogEngine.
///
/// Routes microkernel log records (level, target, message, file, line) into `TTLogEngine.shared.log(category: .kernel, ...)`.
/// This function is thread-safe and idempotent.
@discardableResult
public func enableMicrokernelLoggingBridge(minLevel: TTLogLevel = .debug) -> Bool {
    let minLevelValue: UInt32
    switch minLevel {
    case .debug:
        minLevelValue = 0
    case .info:
        minLevelValue = 1
    case .warning:
        minLevelValue = 2
    case .error, .quiet:
        minLevelValue = 3
    }

    do {
        try uniffiSetLogger(callback: MicrokernelLogHandler(), minLevel: minLevelValue)
        isBridgeConfigured.withLock { $0 = true }
        return true
    } catch {
        isBridgeConfigured.withLock { $0 = false }
        TTLogEngine.shared.log(
            level: .error,
            category: .kernel,
            message: "Failed to install UniFFI microkernel logger callback: \(error)"
        )
        return false
    }
}
