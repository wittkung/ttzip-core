// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import os
import TTLogKit

/// High-precision cross-language trace span bridging Swift and Rust microkernel observability.
public final class TTZipTraceSpan: Sendable {
    public let name: String
    public let spanId: UInt64
    public let category: TTLogCategory
    private let startNanos: UInt64

    private static let nextId = OSAllocatedUnfairLock(initialState: UInt64(1))

    private init(name: String, category: TTLogCategory) {
        self.name = name
        self.category = category
        self.spanId = Self.nextId.withLock { id in
            let current = id
            id &+= 1
            return current
        }
        self.startNanos = DispatchTime.now().uptimeNanoseconds

        uniffiLogDirect(
            level: 0,
            target: "swift::span",
            message: "[SPAN:BEGIN] name=\"\(name)\" id=\(spanId)",
            file: "TTZipTraceSpan.swift",
            line: #line
        )
    }

    /// Begins a structured trace span.
    public static func begin(_ name: String, category: TTLogCategory = .kernel) -> TTZipTraceSpan {
        return TTZipTraceSpan(name: name, category: category)
    }

    /// Ends the span, computing duration and emitting performance telemetry.
    public func finish(status: String = "OK", extra: String? = nil) {
        let elapsedNanos = DispatchTime.now().uptimeNanoseconds - startNanos
        let elapsedMs = Double(elapsedNanos) / 1_000_000.0

        let extraDetail = extra.map { " \($0)" } ?? ""
        let message = "[SPAN:END] name=\"\(name)\" id=\(spanId) elapsed_ms=\(String(format: "%.3f", elapsedMs)) status=\(status)\(extraDetail)"

        uniffiLogDirect(
            level: 0,
            target: "swift::span",
            message: message,
            file: "TTZipTraceSpan.swift",
            line: #line
        )

        TTLogEngine.shared.log(
            level: .debug,
            category: category,
            message: message,
            file: "TTZipTraceSpan",
            line: #line
        )
    }

    /// Scoped execution helper guaranteeing span completion.
    @discardableResult
    public static func withSpan<T>(
        _ name: String,
        category: TTLogCategory = .kernel,
        block: () throws -> T
    ) rethrows -> T {
        let span = begin(name, category: category)
        do {
            let result = try block()
            span.finish(status: "OK")
            return result
        } catch {
            span.finish(status: "ERROR", extra: "error=\(error)")
            throw error
        }
    }

    /// Async scoped execution helper.
    @discardableResult
    public static func withSpanAsync<T>(
        _ name: String,
        category: TTLogCategory = .kernel,
        block: () async throws -> T
    ) async rethrows -> T {
        let span = begin(name, category: category)
        do {
            let result = try await block()
            span.finish(status: "OK")
            return result
        } catch {
            span.finish(status: "ERROR", extra: "error=\(error)")
            throw error
        }
    }
}
