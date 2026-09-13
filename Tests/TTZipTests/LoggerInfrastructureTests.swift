// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import XCTest
@testable import TTZipCore

final class LoggerInfrastructureTests: XCTestCase {
    private var tempDirectory: URL!

    override func setUpWithError() throws {
        try super.setUpWithError()
        tempDirectory = FileManager.default.temporaryDirectory.appendingPathComponent("TTZipLogTests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        if let tempDirectory = tempDirectory {
            try? FileManager.default.removeItem(at: tempDirectory)
        }
        try super.tearDownWithError()
    }

    func testLogFileWriterIsolatedWriteAndFlush() throws {
        let writer = TTLogFileWriter(customLogDirectory: tempDirectory)
        let sampleMessage = "2026-09-14 00:00:00.000 [INFO] [general] TTZip test log entry\n"

        writer.write(sampleMessage)
        writer.flushSync()

        let logPath = writer.logFilePath
        XCTAssertTrue(FileManager.default.fileExists(atPath: logPath))

        let content = try String(contentsOfFile: logPath, encoding: .utf8)
        XCTAssertTrue(content.contains("TTZip test log entry"))
    }

    func testLogFileWriterConcurrentWrites() throws {
        let writer = TTLogFileWriter(customLogDirectory: tempDirectory)
        let group = DispatchGroup()
        let iterations = 100

        for i in 0..<iterations {
            group.enter()
            DispatchQueue.global().async {
                writer.write("Concurrent message #\(i)\n")
                group.leave()
            }
        }

        group.wait()
        writer.flushSync()

        let logPath = writer.logFilePath
        let content = try String(contentsOfFile: logPath, encoding: .utf8)
        let lines = content.components(separatedBy: "\n").filter { !$0.isEmpty }
        XCTAssertEqual(lines.count, iterations)
    }

    func testLoggerCategoryAndLevelSemantics() {
        XCTAssertTrue(TTLogger.Level.debug < TTLogger.Level.info)
        XCTAssertTrue(TTLogger.Level.info < TTLogger.Level.warning)
        XCTAssertTrue(TTLogger.Level.warning < TTLogger.Level.error)
        XCTAssertTrue(TTLogger.Level.error < TTLogger.Level.quiet)

        XCTAssertEqual(TTLogger.Category.allCases.count, 6)
        XCTAssertTrue(TTLogger.Category.allCases.contains(.general))
        XCTAssertTrue(TTLogger.Category.allCases.contains(.archive))
        XCTAssertTrue(TTLogger.Category.allCases.contains(.preview))
        XCTAssertTrue(TTLogger.Category.allCases.contains(.device))
        XCTAssertTrue(TTLogger.Category.allCases.contains(.kernel))
        XCTAssertTrue(TTLogger.Category.allCases.contains(.hang))

        // Ensure logging APIs do not crash across all categories and levels
        TTLogger.debug("Testing preview debug dispatch", category: .preview)
        TTLogger.info("Testing kernel info dispatch", category: .kernel)
        TTLogger.warning("Testing device warning dispatch", category: .device)
        TTLogger.error("Testing hang error dispatch", category: .hang)
    }
}
