// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import os

/// Unified console, system diagnostics, and Apple Unified Logging service (`TTLogger`).
public final class TTLogger: @unchecked Sendable {
    public enum Level: Int, Comparable, Sendable {
        case debug = 0
        case info = 1
        case warning = 2
        case error = 3
        case quiet = 4

        public static func < (lhs: Level, rhs: Level) -> Bool {
            return lhs.rawValue < rhs.rawValue
        }
    }

    public enum Category: String, Sendable, CaseIterable {
        case general
        case archive
        case preview
        case device
        case kernel
        case hang
    }

    public struct LogEntry: Sendable {
        public let timestamp: Date
        public let level: Level
        public let category: Category
        public let message: String
        public let file: String
        public let line: UInt

        public init(
            timestamp: Date = Date(),
            level: Level,
            category: Category = .general,
            message: String,
            file: String,
            line: UInt
        ) {
            self.timestamp = timestamp
            self.level = level
            self.category = category
            self.message = message
            self.file = file
            self.line = line
        }
    }

    public static let shared = TTLogger()

    private struct State: Sendable {
        var level: Level
        var isCapturingForTest: Bool
        var logBuffer: [LogEntry]
    }

    private let state: OSAllocatedUnfairLock<State>
    private let maxBufferSize: Int = 2000
    private let isConsolePrintForced: Bool
    private let isTestEnvironment: Bool
    private let osLoggers: [Category: os.Logger]

    public var level: Level {
        get {
            state.withLock { $0.level }
        }
        set {
            state.withLock { $0.level = newValue }
        }
    }

    private init() {
        let env = ProcessInfo.processInfo.environment
        let initialLevel: Level
        var testCapturing = false

        if let envLevelStr = env["TTZIP_LOG_LEVEL"]?.lowercased() {
            switch envLevelStr {
            case "debug": initialLevel = .debug
            case "info": initialLevel = .info
            case "warning", "warn": initialLevel = .warning
            case "error": initialLevel = .error
            case "quiet", "off": initialLevel = .quiet
            default: initialLevel = .info
            }
        } else if env["XCTestConfigurationFilePath"] != nil || NSClassFromString("XCTestCase") != nil {
            initialLevel = .quiet
            testCapturing = true
        } else {
            initialLevel = .info
        }

        self.isTestEnvironment = (env["XCTestConfigurationFilePath"] != nil || NSClassFromString("XCTestCase") != nil)
        self.isConsolePrintForced = (env["TTZIP_DEBUG_CONSOLE"] == "1")

        self.state = OSAllocatedUnfairLock(
            initialState: State(
                level: initialLevel,
                isCapturingForTest: testCapturing,
                logBuffer: []
            )
        )

        var loggers: [Category: os.Logger] = [:]
        for cat in Category.allCases {
            loggers[cat] = os.Logger(subsystem: "com.metastudyline.ttzip", category: cat.rawValue)
        }
        self.osLoggers = loggers
    }

    // MARK: - Core Logging

    public func log(
        level: Level,
        category: Category = .general,
        message: String,
        file: String = #file,
        line: UInt = #line
    ) {
        let now = Date()
        let fileName = (file as NSString).lastPathComponent
        let entry = LogEntry(
            timestamp: now,
            level: level,
            category: category,
            message: message,
            file: fileName,
            line: line
        )

        let (currentLevel, shouldPrintConsole) = state.withLock { s -> (Level, Bool) in
            if s.isCapturingForTest {
                if s.logBuffer.count >= maxBufferSize {
                    s.logBuffer.removeFirst(100)
                }
                s.logBuffer.append(entry)
            }
            let isLevelActive = (level >= s.level && s.level != .quiet)
            let printConsole = isLevelActive && (isConsolePrintForced || isTestEnvironment || isatty(STDOUT_FILENO) != 0)
            return (s.level, printConsole)
        }

        guard currentLevel != .quiet else { return }

        // 1. Dispatch to Apple Unified Logging (os.Logger)
        let osLogger = osLoggers[category] ?? os.Logger(subsystem: "com.metastudyline.ttzip", category: category.rawValue)
        switch level {
        case .debug:
            osLogger.debug("\(message, privacy: .public)")
        case .info:
            osLogger.info("\(message, privacy: .public)")
        case .warning:
            osLogger.warning("\(message, privacy: .public)")
        case .error:
            osLogger.error("\(message, privacy: .public)")
        case .quiet:
            break
        }

        // 2. Format and dispatch to TTLogFileWriter
        let levelName: String
        switch level {
        case .debug: levelName = "DEBUG"
        case .info: levelName = "INFO"
        case .warning: levelName = "WARN"
        case .error: levelName = "ERROR"
        case .quiet: levelName = "QUIET"
        }

        let timeStr = now.formatted(.iso8601)
        let fileLogLine = "[\(timeStr)] [\(levelName)] [\(category.rawValue)] [\(fileName):\(line)] \(message)\n"
        TTLogFileWriter.shared.write(fileLogLine)

        // 3. Optional terminal print for development / testing
        if shouldPrintConsole {
            let consoleLine = "[\(fileName):\(line)] [\(category.rawValue)] \(message)"
            print(consoleLine)
            fflush(stdout)
        }
    }

    // MARK: - Test Log Capture & Fail-Dump API

    public static func startTestCapture() {
        let instance = shared
        instance.state.withLock { s in
            s.isCapturingForTest = true
            s.logBuffer.removeAll(keepingCapacity: true)
        }
    }

    public static func clearTestCapture() {
        let instance = shared
        instance.state.withLock { s in
            s.logBuffer.removeAll(keepingCapacity: true)
        }
    }

    public static func dumpCapturedLogsOnFailure(testName: String = "Test") {
        let instance = shared
        let buffer = instance.state.withLock { $0.logBuffer }
        guard !buffer.isEmpty else { return }

        print("\n==========================================================================================")
        print("🚨 [TTLogger Log Dump on Failure] Test '\(testName)' execution trace (\(buffer.count) entries)")
        print("==========================================================================================")
        for entry in buffer {
            let lvlStr: String
            switch entry.level {
            case .debug: lvlStr = "DEBUG"
            case .info: lvlStr = "INFO"
            case .warning: lvlStr = "WARN"
            case .error: lvlStr = "ERROR"
            case .quiet: lvlStr = "QUIET"
            }
            print(" [\(lvlStr)] [\(entry.category.rawValue)] [\(entry.file):\(entry.line)] \(entry.message)")
        }
        print("==========================================================================================\n")
        fflush(stdout)
    }

    // MARK: - Static Helper Shorthands

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
}

