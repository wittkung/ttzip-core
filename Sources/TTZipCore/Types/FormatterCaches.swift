// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

/// Shared byte count formatted string provider with caching.
public final class ByteCountFormatterCache: @unchecked Sendable {
    public static let shared = ByteCountFormatterCache()

    private let lock = NSLock()
    private var stringCache: [Int64: String] = [:]

    private let formatter: ByteCountFormatter = {
        let fmt = ByteCountFormatter()
        fmt.allowedUnits = [.useAll]
        fmt.countStyle = .file
        return fmt
    }()

    private let maxCacheSize = 20_000

    private init() {
        for bytes in Int64(0)...Int64(1024) {
            stringCache[bytes] = formatter.string(fromByteCount: bytes)
        }
    }

    public func string(fromByteCount bytes: Int64) -> String {
        let targetBytes = max(0, bytes)

        lock.lock()
        defer { lock.unlock() }
        if let cached = stringCache[targetBytes] {
            return cached
        }

        let formatted = formatter.string(fromByteCount: targetBytes)
        if stringCache.count < maxCacheSize {
            stringCache[targetBytes] = formatted
        }
        return formatted
    }

    public static func string(fromByteCount byteCount: Int64) -> String {
        return shared.string(fromByteCount: byteCount)
    }

    public func clearCache() {
        lock.lock()
        defer { lock.unlock() }
        stringCache.removeAll(keepingCapacity: false)
        for bytes in Int64(0)...Int64(1024) {
            stringCache[bytes] = formatter.string(fromByteCount: bytes)
        }
    }
}

public typealias ByteCountFormatterFlyweight = ByteCountFormatterCache

// MARK: - Date Formatter Cache

/// Thread-safe global DateFormatter cache avoiding redundant instance allocations
/// and eliminating cross-thread data races via Thread-Local Storage (TLS).
public final class DateFormatterCache: Sendable {
    public static let shared = DateFormatterCache()

    private final class ThreadLocalContainer {
        var formatters: [String: DateFormatter] = [:]
        lazy var shortDateTimeFormatter: DateFormatter = {
            let fmt = DateFormatter()
            fmt.dateStyle = .short
            fmt.timeStyle = .short
            return fmt
        }()
    }

    private static let threadLocalKey = "com.metastudyline.ttzip.DateFormatterCache.TLS"

    private init() {}

    private var localContainer: ThreadLocalContainer {
        let dict = Thread.current.threadDictionary
        if let container = dict[Self.threadLocalKey] as? ThreadLocalContainer {
            return container
        }
        let newContainer = ThreadLocalContainer()
        dict[Self.threadLocalKey] = newContainer
        return newContainer
    }

    /// Formats a Date using short date and short time style on the current thread.
    public func string(fromShortDateTime date: Date) -> String {
        return localContainer.shortDateTimeFormatter.string(from: date)
    }

    /// Formats a Date into a string using the specified format pattern on the current thread.
    public func string(from date: Date, format: String) -> String {
        return getLocalFormatter(for: format).string(from: date)
    }

    /// Parses a date string using the specified format pattern on the current thread.
    public func date(from string: String, format: String) -> Date? {
        return getLocalFormatter(for: format).date(from: string)
    }

    /// Returns a thread-isolated DateFormatter instance configured with the specified format pattern.
    /// A defensive copy is returned so callers cannot mutate the thread-local cached instance.
    public func formatter(for format: String) -> DateFormatter {
        let cached = getLocalFormatter(for: format)
        return (cached.copy() as? DateFormatter) ?? cached
    }

    private func getLocalFormatter(for format: String) -> DateFormatter {
        let container = localContainer
        if let existing = container.formatters[format] {
            return existing
        }
        let formatter = DateFormatter()
        formatter.dateFormat = format
        container.formatters[format] = formatter
        return formatter
    }
}
