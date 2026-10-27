// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
@testable import TTZipCore

extension TTZipAssertions {
    /// Asserts that the underlying executed engine matches the exact expected pure Rust engine tag.
    public static func assertEngineExecution(
        _ provenance: EngineDispatchProvenance,
        expected: EngineExecutionTag,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertEqual(
            provenance.engineTag,
            expected,
            "❌ [Engine Mismatch] Expected '\(expected.rawValue)' but actual executed engine was '\(provenance.engineTag.rawValue)'. Fallback: \(provenance.isFallback), Reason: \(provenance.fallbackReason ?? "None")",
            file: file,
            line: line
        )
    }

    /// Asserts that no operation silently fell back to legacy C wrappers or CLI processes.
    public static func assertNoFallback(
        _ provenance: EngineDispatchProvenance,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertFalse(
            provenance.isFallback,
            "❌ [Unexpected Engine Fallback] Operation fell back to '\(provenance.engineTag.rawValue)'. Reason: \(provenance.fallbackReason ?? "Unknown")",
            file: file,
            line: line
        )
        XCTAssertTrue(
            provenance.engineTag.isPureRust,
            "❌ [Impure Engine] Engine '\(provenance.engineTag.rawValue)' is not a Pure Rust engine.",
            file: file,
            line: line
        )
    }
}
