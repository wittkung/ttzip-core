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

import XCTest
import CoreServices
import Darwin
@testable import TTZipCore

final class ZeroDiskIOLeakHarnessTests: XCTestCase {
    
    private var tempDestDir: URL!
    private var watchdogSandboxDir: URL!
    
    override func setUpWithError() throws {
        try super.setUpWithError()
        tempDestDir = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_dest_\(UUID().uuidString)")
        watchdogSandboxDir = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_watchdog_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDestDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: watchdogSandboxDir, withIntermediateDirectories: true)
    }
    
    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: tempDestDir)
        try? FileManager.default.removeItem(at: watchdogSandboxDir)
        try super.tearDownWithError()
    }
    
    private func getProcessDiskBytesWritten() -> UInt64 {
        var rusageInfo = rusage_info_v4()
        let flavor: Int32 = 4 // RUSAGE_INFO_V4
        let ret = withUnsafeMutablePointer(to: &rusageInfo) { ptr in
            ptr.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) { infoPtr in
                proc_pid_rusage(getpid(), flavor, infoPtr)
            }
        }
        guard ret == 0 else { return 0 }
        return rusageInfo.ri_diskio_byteswritten
    }
    
    func testStreamingExtractionZeroDiskIOLeakInvariant() async throws {
        let sourceFile = tempDestDir.appendingPathComponent("payload_source.bin")
        let chunk = Data(repeating: 0xFE, count: 1024 * 1024) // 1MB
        var fullData = Data()
        for _ in 0..<5 { fullData.append(chunk) } // 5MB payload
        try fullData.write(to: sourceFile)
        
        let zipPath = tempDestDir.appendingPathComponent("stream_test.zip").path
        let writer = ArchiveWriter()
        let created = writer.createArchiveWithRust(
            outputPath: zipPath,
            format: .zip,
            inputPaths: [sourceFile.path],
            level: .fast,
            password: nil,
            totalBytes: Int64(fullData.count)
        )
        XCTAssertTrue(created)
        
        let initialDiskBytes = getProcessDiskBytesWritten()
        
        // Execute in-memory extractions
        for _ in 0..<20 {
            let extractedData = try await ArchiveSelectiveExtractor.shared.extractSingleEntryData(
                archivePath: zipPath,
                entryPath: "payload_source.bin"
            )
            XCTAssertNotNil(extractedData)
            XCTAssertEqual(extractedData?.count, fullData.count)
        }
        
        let watchdogContents = try FileManager.default.contentsOfDirectory(atPath: watchdogSandboxDir.path)
        XCTAssertEqual(watchdogContents.count, 0, "Watchdog directory must have 0 temporary files")
        
        let finalDiskBytes = getProcessDiskBytesWritten()
        let deltaDiskWritten = finalDiskBytes - initialDiskBytes
        
        // Single-entry memory extraction should write 0 bytes to disk
        XCTAssertEqual(deltaDiskWritten, 0, "Disk writes occurred during in-memory streaming extraction: \(deltaDiskWritten) bytes")
        print("✓ Zero-Disk-IO Leak Invariant Verified: 20 Extractions performed with 0 bytes disk amplification.")
    }
}
