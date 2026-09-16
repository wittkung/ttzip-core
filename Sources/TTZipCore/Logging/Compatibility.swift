// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import os
@_exported import TTLogKit

/// Backward compatibility alias for legacy file logging instances.
public typealias TTLogFileWriter = TTLogFileSink

extension TTLogCategory: @retroactive CaseIterable {
    /// Preserves legacy enum case iteration for existing test assertions.
    public static let allCases: [TTLogCategory] = [
        .general,
        .archive,
        .preview,
        .device,
        .kernel,
        .hang
    ]
}

/// Backward compatibility logging facade wrapping `TTLogEngine`.
public final class TTLogger: @unchecked Sendable {
    public static let shared = TTLogger()

    public typealias Level = TTLogLevel
    public typealias Category = TTLogCategory
    public typealias LogEntry = TTLogRecord

    private static let testCaptureSink = TTLogCaptureSink()
    private static let isCaptureAttached = OSAllocatedUnfairLock(initialState: false)

    /// Active minimum logging level on shared engine.
    public var level: TTLogLevel {
        get {
            TTLogEngine.shared.level
        }
        set {
            TTLogEngine.shared.level = newValue
            _ = enableMicrokernelLoggingBridge(minLevel: newValue)
        }
    }

    private init() {
        _ = TTZipCore.enableMicrokernelLoggingBridge(minLevel: TTLogEngine.shared.level)
    }

    /// Bridges microkernel logs into the shared logger engine.
    @discardableResult
    public func enableMicrokernelLoggingBridge(minLevel: TTLogLevel? = nil) -> Bool {
        let targetLevel = minLevel ?? self.level
        return TTZipCore.enableMicrokernelLoggingBridge(minLevel: targetLevel)
    }

    /// Logs an entry to the shared log engine.
    public func log(
        level: Level,
        category: Category = .general,
        message: String,
        file: String = #file,
        line: UInt = #line
    ) {
        TTLogEngine.shared.log(
            level: level,
            category: category,
            message: message,
            file: file,
            line: line
        )
    }

    // MARK: - Static Shorthands

    @inline(__always)
    public static func debug(
        _ message: @autoclosure () -> String,
        category: Category = .general,
        file: String = #file,
        line: UInt = #line
    ) {
        shared.log(level: .debug, category: category, message: message(), file: file, line: line)
    }

    @inline(__always)
    public static func info(
        _ message: @autoclosure () -> String,
        category: Category = .general,
        file: String = #file,
        line: UInt = #line
    ) {
        shared.log(level: .info, category: category, message: message(), file: file, line: line)
    }

    @inline(__always)
    public static func warning(
        _ message: @autoclosure () -> String,
        category: Category = .general,
        file: String = #file,
        line: UInt = #line
    ) {
        shared.log(level: .warning, category: category, message: message(), file: file, line: line)
    }

    @inline(__always)
    public static func error(
        _ message: @autoclosure () -> String,
        category: Category = .general,
        file: String = #file,
        line: UInt = #line
    ) {
        shared.log(level: .error, category: category, message: message(), file: file, line: line)
    }

    // MARK: - Sanitizer Shorthand

    public static func sanitize(_ message: String) -> String {
        return TTLogSanitizer.sanitize(message)
    }

    public func sanitize(_ message: String) -> String {
        return TTLogSanitizer.sanitize(message)
    }

    // MARK: - Test Capture Compatibility

    public static func startTestCapture() {
        isCaptureAttached.withLock { attached in
            if !attached {
                TTLogEngine.shared.addSink(testCaptureSink)
                attached = true
            }
        }
        testCaptureSink.clear()
    }

    public static func clearTestCapture() {
        testCaptureSink.clear()
    }

    public static var capturedLogs: [TTLogRecord] {
        return testCaptureSink.records
    }

    public static func dumpCapturedLogsOnFailure(testName: String = "Test") {
        testCaptureSink.dumpOnFailure(testName: testName)
    }
}
