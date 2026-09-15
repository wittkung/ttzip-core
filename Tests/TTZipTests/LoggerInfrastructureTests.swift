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

    func testLogSanitizerSensitivePatterns() {
        // 1. Password redaction variants
        let p1 = TTLogSanitizer.sanitize("Extracting archive with password=MySecret123 and user=alice")
        XCTAssertEqual(p1, "Extracting archive with password=[REDACTED] and user=alice")

        let p2 = TTLogSanitizer.sanitize("Config dump: {\"password\": \"VaultSecretPass!\", \"retries\": 3}")
        XCTAssertEqual(p2, "Config dump: {\"password\": [REDACTED], \"retries\": 3}")

        let p3 = TTLogSanitizer.sanitize("Archive pwd='QuickPassword99' valid")
        XCTAssertEqual(p3, "Archive pwd=[REDACTED] valid")

        // 2. Token redaction variants
        let t1 = TTLogSanitizer.sanitize("Request failed: token=tok_live_891238912389123")
        XCTAssertEqual(t1, "Request failed: token=[REDACTED]")

        let t2 = TTLogSanitizer.sanitize("Lark auth response: tenant_access_token=t-g1049abcdef and expires_in=7200")
        XCTAssertEqual(t2, "Lark auth response: tenant_access_token=[REDACTED] and expires_in=7200")

        let t3 = TTLogSanitizer.sanitize("HTTP header: Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9")
        XCTAssertEqual(t3, "HTTP header: Authorization: Bearer [REDACTED]")

        // 3. Embedded URL credentials
        let u1 = TTLogSanitizer.sanitize("Connecting to https://admin:SuperSecret456@remote.cloud.ttzip/archive.tar.gz")
        XCTAssertEqual(u1, "Connecting to https://admin:[REDACTED]@remote.cloud.ttzip/archive.tar.gz")

        // 4. Query string parameters
        let q1 = TTLogSanitizer.sanitize("Fetching https://api.endpoint.org/download?file=demo.zip&password=Pass123&token=t456")
        XCTAssertEqual(q1, "Fetching https://api.endpoint.org/download?file=demo.zip&password=[REDACTED]&token=[REDACTED]")

        // 5. Labeled keys and secrets
        let k1 = TTLogSanitizer.sanitize("Crypto session key: api_key=sk_test_51Mzxyz1234567890")
        XCTAssertEqual(k1, "Crypto session key: api_key=[REDACTED]")

        let k2 = TTLogSanitizer.sanitize("Initializing vault with master_key: 0123456789abcdef0123456789abcdef")
        XCTAssertEqual(k2, "Initializing vault with master_key: [REDACTED]")

        // 6. PEM Private Key block
        let pemSample = """
        Loading server key:
        -----BEGIN RSA PRIVATE KEY-----
        MIIEowIBAAKCAQEA0Y3wVq2D...
        -----END RSA PRIVATE KEY-----
        Done
        """
        let pemRedacted = TTLogSanitizer.sanitize(pemSample)
        XCTAssertTrue(pemRedacted.contains("[REDACTED_PRIVATE_KEY]"))
        XCTAssertFalse(pemRedacted.contains("MIIEowIBAAKCAQEA0Y3wVq2D"))
    }

    func testLogSanitizerFastPathPreservesNonSensitiveMessages() {
        let normal1 = "VFS node /Users/admin/Documents/report.pdf indexed successfully."
        XCTAssertEqual(TTLogSanitizer.sanitize(normal1), normal1)

        let normal2 = "Parallel chunk compression completed: 8 threads, 4194304 bytes."
        XCTAssertEqual(TTLogSanitizer.sanitize(normal2), normal2)

        let normal3 = "Archive header CRC32: 0xEDB88320 matches entry catalog."
        XCTAssertEqual(TTLogSanitizer.sanitize(normal3), normal3)
    }

    func testLoggerSanitizationInLivePipeline() {
        TTLogger.startTestCapture()
        defer { TTLogger.clearTestCapture() }

        TTLogger.shared.level = .debug
        TTLogger.info("Unzipping encrypted payload with password=TopSecretArchivePassword")

        let logs = TTLogger.capturedLogs
        XCTAssertFalse(logs.isEmpty, "Logs must be captured during test capture mode")
        let lastLog = logs.last!
        XCTAssertTrue(lastLog.message.contains("password=[REDACTED]"))
        XCTAssertFalse(lastLog.message.contains("TopSecretArchivePassword"))
    }

    func testMicrokernelLoggingBridgeDirectEmission() {
        TTLogger.startTestCapture()
        defer { TTLogger.clearTestCapture() }

        TTLogger.shared.level = .debug
        let success = TTLogger.shared.enableMicrokernelLoggingBridge(minLevel: .debug)
        XCTAssertTrue(success, "Microkernel bridge registration must succeed")

        let target = "test_kernel"
        let msg = "Microkernel diagnostics pass with token=KernelInternalSecretToken"
        let file = "kernel_vfs.rs"
        let line: Int32 = 88

        uniffiLogDirect(level: 1, target: target, message: msg, file: file, line: UInt32(line))

        let logs = TTLogger.capturedLogs
        let kernelLogs = logs.filter { $0.category == .kernel }
        XCTAssertFalse(kernelLogs.isEmpty, "Kernel logs must be routed to category .kernel")

        let targetEntry = kernelLogs.first { $0.message.contains("Microkernel diagnostics pass") }
        XCTAssertNotNil(targetEntry, "Expected microkernel log entry to be received")

        if let entry = targetEntry {
            XCTAssertEqual(entry.level, .info)
            XCTAssertTrue(entry.message.contains("[test_kernel]"))
            XCTAssertTrue(entry.message.contains("token=[REDACTED]"))
            XCTAssertFalse(entry.message.contains("KernelInternalSecretToken"))
        }
    }
}
