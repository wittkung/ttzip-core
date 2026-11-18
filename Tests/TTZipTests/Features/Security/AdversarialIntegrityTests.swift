// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
@testable import TTZipCore

final class AdversarialIntegrityTests: XCTestCase {
    private var tempDir: URL!

    override func setUpWithError() throws {
        try super.setUpWithError()
        tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_adv_test_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: tempDir)
        try super.tearDownWithError()
    }

    /// Tests that true stream-discarding integrity verification accurately detects corrupted blocks
    /// and flags CRC mismatch errors instead of fake counting.
    func testMutatedZipIntegrityDetection() async throws {
        let zipPath = tempDir.appendingPathComponent("test_archive.zip").path
        let sampleFile = tempDir.appendingPathComponent("payload.txt").path
        let originalContent = "Hello TTZip Secure Archiving System Verification Stream."
        try originalContent.write(toFile: sampleFile, atomically: true, encoding: .utf8)

        let writer = ArchiveWriter()
        try writer.createArchiveSync(
            outputPath: zipPath,
            format: .zip,
            level: .normal,
            inputPaths: [sampleFile]
        )

        // 1. Assert pristine integrity passes
        let checker = ArchiveIntegrityChecker()
        let pristineReport = try await checker.checkArchiveIntegrity(archivePath: zipPath)
        XCTAssertEqual(pristineReport.overallStatus, IntegrityStatus.passed)
        XCTAssertEqual(pristineReport.corruptedEntriesCount, 0)

        // 2. Corrupt bytes in the archive
        var data = try Data(contentsOf: URL(fileURLWithPath: zipPath))
        guard data.count > 50 else {
            XCTFail("Zip payload too small to mutate")
            return
        }
        for offset in (data.count - 40)..<(data.count - 20) {
            data[offset] ^= 0xFF
        }
        try data.write(to: URL(fileURLWithPath: zipPath))

        // 3. Assert mutated archive is flagged as corrupted
        let mutatedReport = try await checker.checkArchiveIntegrity(archivePath: zipPath)
        XCTAssertEqual(mutatedReport.overallStatus, IntegrityStatus.corrupted)
        XCTAssertGreaterThan(mutatedReport.corruptedEntriesCount, 0)
    }

    /// SEC-05: Tests that extraction failure does NOT delete existing files in target folder.
    func testExtractionFailurePreservesExistingDirectoryFiles() async throws {
        let targetDir = tempDir.appendingPathComponent("user_documents")
        try FileManager.default.createDirectory(at: targetDir, withIntermediateDirectories: true)

        let existingPreciousFile = targetDir.appendingPathComponent("important_tax_document.pdf")
        try "Precious Tax Data".write(to: existingPreciousFile, atomically: true, encoding: .utf8)

        let corruptZip = tempDir.appendingPathComponent("broken.zip")
        try Data([0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, 0xFF, 0xFF]).write(to: corruptZip)

        let extractor = ArchiveExtractor()
        do {
            try await extractor.extract(
                archivePath: corruptZip.path,
                destinationDir: targetDir.path,
                options: .defaultClean
            )
            XCTFail("Extraction of broken archive should throw")
        } catch {
            // Expected error
        }

        // SEC-05 Assert: Precious existing file must STILL exist!
        XCTAssertTrue(FileManager.default.fileExists(atPath: existingPreciousFile.path))
        let content = try String(contentsOf: existingPreciousFile, encoding: .utf8)
        XCTAssertEqual(content, "Precious Tax Data")
    }

    // MARK: - 3. Zip Slip & Path Traversal Adversarial Defense

    func testZipSlipPathTraversalDefense() async throws {
        let destinationDir = tempDir.appendingPathComponent("safe_extract_target")
        try FileManager.default.createDirectory(at: destinationDir, withIntermediateDirectories: true)

        let outsideSecretFile = tempDir.appendingPathComponent("escaped_secret.txt")
        let deepOutsideSecretFile = tempDir.deletingLastPathComponent().appendingPathComponent("pwned.txt")

        let maliciousPayloads = [
            "../escaped_secret.txt",
            "../../pwned.txt",
            "..\\win_escaped.txt",
            "/tmp/rooted_exploit.txt"
        ]

        let extractor = ArchiveExtractor()

        for (idx, attackEntry) in maliciousPayloads.enumerated() {
            let zipURL = tempDir.appendingPathComponent("exploit_\(idx).zip")
            let maliciousData = createSyntheticZip(withEntryName: attackEntry, content: "ATTACKER_PWNED_DATA")
            try maliciousData.write(to: zipURL)

            do {
                try await extractor.extract(
                    archivePath: zipURL.path,
                    destinationDir: destinationDir.path
                )
                // If it didn't throw, we must guarantee zero files escaped outside destinationDir
            } catch {
                // Throws security violation or extraction failure - expected safe defense behavior
            }

            // Invariant Gate: Zero files must ever escape or be written outside destination directory
            XCTAssertFalse(
                FileManager.default.fileExists(atPath: outsideSecretFile.path),
                "Zip Slip exploit succeeded: '\(attackEntry)' escaped to parent directory!"
            )
            XCTAssertFalse(
                FileManager.default.fileExists(atPath: deepOutsideSecretFile.path),
                "Zip Slip exploit succeeded: '\(attackEntry)' escaped to root/ancestor directory!"
            )
        }
    }

    // MARK: - Synthetic Malicious Zip Archive Builder

    private func createSyntheticZip(withEntryName entryName: String, content: String) -> Data {
        let entryNameBytes = Array(entryName.utf8)
        let contentBytes = Array(content.utf8)

        var data = Data()

        // 1. Local File Header
        data.append(contentsOf: [0x50, 0x4B, 0x03, 0x04]) // PK\x03\x04
        data.append(contentsOf: [0x14, 0x00]) // Version needed (2.0)
        data.append(contentsOf: [0x00, 0x00]) // General purpose bit flag
        data.append(contentsOf: [0x00, 0x00]) // Compression method (0 = stored)
        data.append(contentsOf: [0x00, 0x00]) // Mod time
        data.append(contentsOf: [0x00, 0x00]) // Mod date

        // CRC-32
        var crc: UInt32 = 0xFFFF_FFFF
        for byte in contentBytes {
            let idx = (crc ^ UInt32(byte)) & 0xFF
            var tableEntry = idx
            for _ in 0..<8 {
                tableEntry = (tableEntry & 1 != 0) ? (tableEntry >> 1) ^ 0xEDB88320 : (tableEntry >> 1)
            }
            crc = (crc >> 8) ^ tableEntry
        }
        crc = ~crc

        var crcLE = crc.littleEndian
        withUnsafeBytes(of: &crcLE) { data.append(contentsOf: $0) }

        var sizeLE = UInt32(contentBytes.count).littleEndian
        withUnsafeBytes(of: &sizeLE) { data.append(contentsOf: $0) } // Compressed size
        withUnsafeBytes(of: &sizeLE) { data.append(contentsOf: $0) } // Uncompressed size

        var nameLenLE = UInt16(entryNameBytes.count).littleEndian
        withUnsafeBytes(of: &nameLenLE) { data.append(contentsOf: $0) } // Name len
        data.append(contentsOf: [0x00, 0x00]) // Extra field len

        data.append(contentsOf: entryNameBytes)
        let localHeaderOffset = 0
        data.append(contentsOf: contentBytes)

        // 2. Central Directory Header
        let cdOffset = UInt32(data.count)
        data.append(contentsOf: [0x50, 0x4B, 0x01, 0x02]) // PK\x01\x02
        data.append(contentsOf: [0x14, 0x00]) // Version made by
        data.append(contentsOf: [0x14, 0x00]) // Version needed
        data.append(contentsOf: [0x00, 0x00]) // Flag
        data.append(contentsOf: [0x00, 0x00]) // Compression method
        data.append(contentsOf: [0x00, 0x00]) // Mod time
        data.append(contentsOf: [0x00, 0x00]) // Mod date
        withUnsafeBytes(of: &crcLE) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: &sizeLE) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: &sizeLE) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: &nameLenLE) { data.append(contentsOf: $0) }
        data.append(contentsOf: [0x00, 0x00]) // Extra field len
        data.append(contentsOf: [0x00, 0x00]) // Comment len
        data.append(contentsOf: [0x00, 0x00]) // Disk start
        data.append(contentsOf: [0x00, 0x00]) // Internal attr
        data.append(contentsOf: [0x00, 0x00, 0x00, 0x00]) // External attr
        var localHeaderOffsetLE = UInt32(localHeaderOffset).littleEndian
        withUnsafeBytes(of: &localHeaderOffsetLE) { data.append(contentsOf: $0) }
        data.append(contentsOf: entryNameBytes)

        // 3. End of Central Directory (EOCD)
        let cdSize = UInt32(data.count) - cdOffset
        data.append(contentsOf: [0x50, 0x4B, 0x05, 0x06]) // PK\x05\x06
        data.append(contentsOf: [0x00, 0x00]) // Disk num
        data.append(contentsOf: [0x00, 0x00]) // CD disk start
        data.append(contentsOf: [0x01, 0x00]) // Entries on disk (1)
        data.append(contentsOf: [0x01, 0x00]) // Total entries (1)
        var cdSizeLE = cdSize.littleEndian
        withUnsafeBytes(of: &cdSizeLE) { data.append(contentsOf: $0) }
        var cdOffsetLE = cdOffset.littleEndian
        withUnsafeBytes(of: &cdOffsetLE) { data.append(contentsOf: $0) }
        data.append(contentsOf: [0x00, 0x00]) // Comment len

        return data
    }

    /// Verifies that propagateQuarantine actively skips symbolic links, preventing quarantine attributes
    /// from escaping extraction directory and tainting external host files.
    func testPropagateQuarantineSkipsSymbolicLinks() throws {
        let archivePath = tempDir.appendingPathComponent("quarantine_source.zip").path
        try "fake_zip_bytes".write(toFile: archivePath, atomically: true, encoding: .utf8)

        let qValue = "0083;65432100;Safari;ABCDEF-1234".utf8CString
        let setRes = qValue.withUnsafeBufferPointer { ptr in
            setxattr(archivePath, "com.apple.quarantine", ptr.baseAddress!, ptr.count, 0, XATTR_NOFOLLOW)
        }
        guard setRes == 0 else {
            return
        }

        let extractDir = tempDir.appendingPathComponent("extracted").path
        let externalDir = tempDir.appendingPathComponent("external_host").path
        try FileManager.default.createDirectory(atPath: extractDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(atPath: externalDir, withIntermediateDirectories: true)

        let regularFile = (extractDir as NSString).appendingPathComponent("file.txt")
        try "regular_content".write(toFile: regularFile, atomically: true, encoding: .utf8)

        let externalFile = (externalDir as NSString).appendingPathComponent("host_secret.txt")
        try "secret_content".write(toFile: externalFile, atomically: true, encoding: .utf8)

        let symlinkPath = (extractDir as NSString).appendingPathComponent("link_to_external")
        try FileManager.default.createSymbolicLink(atPath: symlinkPath, withDestinationPath: externalFile)

        ArchiveExtractor.propagateQuarantine(from: archivePath, to: extractDir)

        // 1. Regular file receives quarantine attribute
        let regSz = getxattr(regularFile, "com.apple.quarantine", nil, 0, 0, XATTR_NOFOLLOW)
        XCTAssertGreaterThan(regSz, 0, "Regular file should receive quarantine attribute")

        // 2. External host file pointed by symlink MUST NOT receive quarantine attribute
        let extSz = getxattr(externalFile, "com.apple.quarantine", nil, 0, 0, XATTR_NOFOLLOW)
        XCTAssertEqual(extSz, -1, "External target file must not be tainted by quarantine propagation")
    }

    /// Verifies that both direct parent symlinks (`dir_link -> ..`) and chained symlinks (`a/b -> ..`, `a/c -> b/..`)
    /// are rejected without overwriting `harmless_marker.txt` outside `destinationDir`, and quarantined archives
    /// never silently downgrade on security errors.
    func testSymlinkParentAndChainedEscapePreservesOutsideMarkerAndRefusesFallback() async throws {
        let outsideMarker = tempDir.appendingPathComponent("harmless_marker.txt")
        try "SAFE_ORIGINAL_CONTENT".write(to: outsideMarker, atomically: true, encoding: .utf8)

        let destinationDir = tempDir.appendingPathComponent("sandbox_dest")
        try FileManager.default.createDirectory(at: destinationDir, withIntermediateDirectories: true)

        let directAttackZip = tempDir.appendingPathComponent("direct_symlink_attack.zip")
        try createSyntheticMultiEntryZip(entries: [
            (name: "dir_link", content: "..", unixMode: 0o120777),
            (name: "dir_link/harmless_marker.txt", content: "PWNED_DIRECT", unixMode: 0o100644)
        ]).write(to: directAttackZip)

        let chainedAttackZip = tempDir.appendingPathComponent("chained_symlink_attack.zip")
        try createSyntheticMultiEntryZip(entries: [
            (name: "a/b", content: "..", unixMode: 0o120777),
            (name: "a/c", content: "b/..", unixMode: 0o120777),
            (name: "a/c/harmless_marker.txt", content: "PWNED_CHAINED", unixMode: 0o100644)
        ]).write(to: chainedAttackZip)

        // Tag chainedAttackZip with com.apple.quarantine to exercise quarantined extraction refusal-to-downgrade path
        let qValue = "0083;65432100;Safari;ABCDEF-1234".utf8CString
        _ = qValue.withUnsafeBufferPointer { ptr in
            setxattr(chainedAttackZip.path, "com.apple.quarantine", ptr.baseAddress!, ptr.count, 0, XATTR_NOFOLLOW)
        }

        let extractor = ArchiveExtractor()
        for attackArchive in [directAttackZip, chainedAttackZip] {
            var didThrow = false
            do {
                try await extractor.extract(
                    archivePath: attackArchive.path,
                    destinationDir: destinationDir.path
                )
            } catch {
                didThrow = true
            }
            XCTAssertTrue(didThrow, "Extractor must reject symlink escape archive \(attackArchive.lastPathComponent)")
            let markerContent = try String(contentsOf: outsideMarker, encoding: .utf8)
            XCTAssertEqual(markerContent, "SAFE_ORIGINAL_CONTENT")
        }
    }

    private func createSyntheticMultiEntryZip(
        entries: [(name: String, content: String, unixMode: UInt32)]
    ) -> Data {
        var data = Data()
        var cd = Data()

        for entry in entries {
            let lfhOffset = UInt32(data.count)
            let nameBytes = Array(entry.name.utf8)
            let contentBytes = Array(entry.content.utf8)

            var crc: UInt32 = 0xFFFF_FFFF
            for byte in contentBytes {
                var tableEntry = (crc ^ UInt32(byte)) & 0xFF
                for _ in 0..<8 {
                    tableEntry = (tableEntry & 1 != 0) ? (tableEntry >> 1) ^ 0xEDB88320 : (tableEntry >> 1)
                }
                crc = (crc >> 8) ^ tableEntry
            }
            crc = ~crc

            var crcLE = crc.littleEndian
            var sizeLE = UInt32(contentBytes.count).littleEndian
            var nameLenLE = UInt16(nameBytes.count).littleEndian

            data.append(contentsOf: [0x50, 0x4B, 0x03, 0x04, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
            withUnsafeBytes(of: &crcLE) { data.append(contentsOf: $0) }
            withUnsafeBytes(of: &sizeLE) { data.append(contentsOf: $0) }
            withUnsafeBytes(of: &sizeLE) { data.append(contentsOf: $0) }
            withUnsafeBytes(of: &nameLenLE) { data.append(contentsOf: $0) }
            data.append(contentsOf: [0x00, 0x00])
            data.append(contentsOf: nameBytes)
            data.append(contentsOf: contentBytes)

            cd.append(contentsOf: [0x50, 0x4B, 0x01, 0x02, 0x14, 0x03, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
            withUnsafeBytes(of: &crcLE) { cd.append(contentsOf: $0) }
            withUnsafeBytes(of: &sizeLE) { cd.append(contentsOf: $0) }
            withUnsafeBytes(of: &sizeLE) { cd.append(contentsOf: $0) }
            withUnsafeBytes(of: &nameLenLE) { cd.append(contentsOf: $0) }
            cd.append(contentsOf: [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
            var extAttrLE = (entry.unixMode << 16).littleEndian
            withUnsafeBytes(of: &extAttrLE) { cd.append(contentsOf: $0) }
            var lfhOffsetLE = lfhOffset.littleEndian
            withUnsafeBytes(of: &lfhOffsetLE) { cd.append(contentsOf: $0) }
            cd.append(contentsOf: nameBytes)
        }

        let cdOffset = UInt32(data.count)
        let cdSize = UInt32(cd.count)
        data.append(cd)

        var countLE = UInt16(entries.count).littleEndian
        var cdSizeLE = cdSize.littleEndian
        var cdOffsetLE = cdOffset.littleEndian
        data.append(contentsOf: [0x50, 0x4B, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00])
        withUnsafeBytes(of: &countLE) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: &countLE) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: &cdSizeLE) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: &cdOffsetLE) { data.append(contentsOf: $0) }
        data.append(contentsOf: [0x00, 0x00])
        return data
    }
}

