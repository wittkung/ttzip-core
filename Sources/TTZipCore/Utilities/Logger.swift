// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import os

/// Industrial zero-trust log sanitizer and sensitive credential redaction engine (`TTLogSanitizer`).
public enum TTLogSanitizer: Sendable {
    public static let redactedMarker = "[REDACTED]"
    public static let redactedKeyMarker = "[REDACTED_PRIVATE_KEY]"

    // Fast rejection scan keywords (lowercased substrings)
    private static let triggerKeywords: [String] = [
        "pass", "pwd", "token", "key", "secret", "bearer", "cred", "auth", "://", "-----begin"
    ]

    private final class RegexRule: @unchecked Sendable {
        let regex: NSRegularExpression
        let template: String

        init(regex: NSRegularExpression, template: String) {
            self.regex = regex
            self.template = template
        }
    }

    private static let rules: [RegexRule] = {
        var list: [RegexRule] = []

        // 1. PEM Private Keys
        if let pemRegex = try? NSRegularExpression(
            pattern: "-----BEGIN [A-Z ]*PRIVATE KEY-----[\\s\\S]*?-----END [A-Z ]*PRIVATE KEY-----",
            options: []
        ) {
            list.append(RegexRule(regex: pemRegex, template: redactedKeyMarker))
        }

        // 2. HTTP Authorization Header / Bearer tokens
        if let bearerRegex = try? NSRegularExpression(
            pattern: "(?i)\\b(Bearer\\s+)[A-Za-z0-9\\-._~+/]+=*",
            options: []
        ) {
            list.append(RegexRule(regex: bearerRegex, template: "$1" + redactedMarker))
        }

        // 3. URLs with embedded user:password credentials: https://user:pass@host
        if let urlCredsRegex = try? NSRegularExpression(
            pattern: "([a-zA-Z][a-zA-Z0-9+.-]*://[^:\\s/@]+):([^@\\s/]+)@",
            options: []
        ) {
            list.append(RegexRule(regex: urlCredsRegex, template: "$1:" + redactedMarker + "@"))
        }

        // 4. Key-Value pairs with sensitive names
        let sensitiveKeys = [
            "password", "passwd", "pwd", "passphrase",
            "secret", "app_secret", "client_secret", "shared_secret",
            "api_key", "apikey", "secret_key", "secretkey",
            "private_key", "privkey",
            "access_token", "refresh_token", "auth_token", "tenant_access_token", "user_access_token", "token",
            "credential", "credentials",
            "vault_key", "master_key", "symmetric_key", "aes_key", "encryption_key",
            "aes_hex", "key_hex", "vault_hex"
        ].joined(separator: "|")

        if let kvRegex = try? NSRegularExpression(
            pattern: "(?i)(?<=^|[^a-zA-Z0-9_])(\"?(" + sensitiveKeys + ")\"?\\s*(?:=|:|:=|=>)\\s*)(?:\"([^\"]*)\"|'([^']*)'|`([^`]*)`|([^\\s,;)\\]}\"'>&]+))",
            options: []
        ) {
            list.append(RegexRule(regex: kvRegex, template: "$1" + redactedMarker))
        }

        // 5. Query parameters: (?|&)password=... or (?|&)token=...
        if let queryRegex = try? NSRegularExpression(
            pattern: "(?i)([?&](?:" + sensitiveKeys + ")=)([^&\\s#]+)",
            options: []
        ) {
            list.append(RegexRule(regex: queryRegex, template: "$1" + redactedMarker))
        }

        return list
    }()

    /// Sanitizes message by redacting passwords, secrets, tokens, private keys, and credential URIs.
    public static func sanitize(_ message: String) -> String {
        guard !message.isEmpty else { return message }

        // Fast-path check: if none of the trigger keywords are present, return immediately.
        let lower = message.lowercased()
        var hasTrigger = false
        for kw in triggerKeywords {
            if lower.contains(kw) {
                hasTrigger = true
                break
            }
        }
        guard hasTrigger else {
            return message
        }

        var result = message
        for rule in rules {
            let range = NSRange(result.startIndex..<result.endIndex, in: result)
            result = rule.regex.stringByReplacingMatches(
                in: result,
                options: [],
                range: range,
                withTemplate: rule.template
            )
        }
        return result
    }
}

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

    private static let isBridgeConfigured = OSAllocatedUnfairLock(initialState: false)

    public var level: Level {
        get {
            state.withLock { $0.level }
        }
        set {
            state.withLock { $0.level = newValue }
            _ = enableMicrokernelLoggingBridge(minLevel: newValue)
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

        // Automatically bind microkernel logging bridge so standalone SDK & app receive kernel logs out-of-the-box
        _ = self.enableMicrokernelLoggingBridge(minLevel: initialLevel)
    }

    // MARK: - Microkernel Logging Bridge

    private final class MicrokernelLogHandler: UniFfiLogCallback, @unchecked Sendable {
        nonisolated func log(level: UInt32, target: String, message: String, file: String, line: UInt32) {
            let mappedLevel: TTLogger.Level
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
            TTLogger.shared.log(
                level: mappedLevel,
                category: .kernel,
                message: formattedMessage,
                file: file.isEmpty ? "kernel" : file,
                line: UInt(line)
            )
        }
    }

    /// Enables the UniFFI logging bridge between the Rust microkernel and TTLogger.
    ///
    /// Routes microkernel log records (level, target, message, file, line) into `TTLogger.shared.log(category: .kernel, ...)`.
    /// This method is thread-safe and idempotent.
    @discardableResult
    public func enableMicrokernelLoggingBridge(minLevel: Level? = nil) -> Bool {
        let currentLevel = minLevel ?? self.level
        let minLevelValue: UInt32
        switch currentLevel {
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
            Self.isBridgeConfigured.withLock { $0 = true }
            return true
        } catch {
            Self.isBridgeConfigured.withLock { $0 = false }
            return false
        }
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
        let sanitized = TTLogSanitizer.sanitize(message)
        let entry = LogEntry(
            timestamp: now,
            level: level,
            category: category,
            message: sanitized,
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

        // 1. Dispatch to Apple Unified Logging (os.Logger) with pre-sanitized privacy
        let osLogger = osLoggers[category] ?? os.Logger(subsystem: "com.metastudyline.ttzip", category: category.rawValue)
        switch level {
        case .debug:
            osLogger.debug("\(sanitized, privacy: .public)")
        case .info:
            osLogger.info("\(sanitized, privacy: .public)")
        case .warning:
            osLogger.warning("\(sanitized, privacy: .public)")
        case .error:
            osLogger.error("\(sanitized, privacy: .public)")
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
        let fileLogLine = "[\(timeStr)] [\(levelName)] [\(category.rawValue)] [\(fileName):\(line)] \(sanitized)\n"
        TTLogFileWriter.shared.write(fileLogLine)

        // 3. Optional terminal print for development / testing
        if shouldPrintConsole {
            let consoleLine = "[\(fileName):\(line)] [\(category.rawValue)] \(sanitized)"
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

    /// Access snapshot of currently captured in-memory test logs.
    public static var capturedLogs: [LogEntry] {
        return shared.state.withLock { $0.logBuffer }
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

    // MARK: - Sanitization Helpers

    /// Public static helper for sanitizing sensitive data (passwords, keys, tokens) from log messages.
    public static func sanitize(_ message: String) -> String {
        return TTLogSanitizer.sanitize(message)
    }

    /// Public instance helper for sanitizing sensitive data.
    public func sanitize(_ message: String) -> String {
        return TTLogSanitizer.sanitize(message)
    }
}

