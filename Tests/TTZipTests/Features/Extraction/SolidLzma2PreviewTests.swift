// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Darwin
import Foundation
import XCTest
import TTZipCore

final class SolidLzma2PreviewTests: XCTestCase {
    private let entry = "01-target.bin"
    private let expected = Data("TTZIP-654 native LZMA2 target\n".utf8)
    private let compressed = """
        N3q8ryccAARiZnZuNQAAAAAAAAB2AAAAAAAAAGgcwpPgARkALV0AOByIzfCeWQEUm+KRGR+0
        nIGTipGzbvyLa2X+SH4rjv1OQa3/azebet/lsEAAAAEEBgABCTUABwsBAAEhIQEADIEaAAgN
        AgmA/AoBjdVYgShE5bIAAAUCGQIAABE5ADAAMAAtAHAAcgBlAGYAaQB4AC4AYgBpAG4AAAAw
        ADEALQB0AGEAcgBnAGUAdAAuAGIAaQBuAAAAFQoBACCApIEggKSBAAA=
        """
    private let badEntryCrc = """
        N3q8ryccAATqC0oKNQAAAAAAAAB2AAAAAAAAAOnAQ5jgARkALV0AOByIzfCeWQEUm+KRGR+0
        nIGTipGzbvyLa2X+SH4rjv1OQa3/azebet/lsEAAAAEEBgABCTUABwsBAAEhIQEADIEaAAgN
        AgmA/AoBjdVYgSlE5bIAAAUCGQIAABE5ADAAMAAtAHAAcgBlAGYAaQB4AC4AYgBpAG4AAAAw
        ADEALQB0AGEAcgBnAGUAdAAuAGIAaQBuAAAAFQoBACCApIEggKSBAAA=
        """
    private let stored = """
        N3q8ryccAATbYeDpHgAAAAAAAABGAAAAAAAAALkTbAFUVFpJUC02NTQgbmF0aXZlIExaTUEy
        IHRhcmdldAoBBAYAAQkeAAcLAQABAQAMHgAICgEoROWyAAAFAREdADAAMQAtAHQAYQByAGcA
        ZQB0AC4AYgBpAG4AAAAVBgEAIICkgQAA
        """

    private func fixture(_ base64: String) throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("\(UUID().uuidString).7z")
        let data = try XCTUnwrap(Data(base64Encoded: base64, options: .ignoreUnknownCharacters))
        try data.write(to: url, options: .withoutOverwriting)
        addTeardownBlock { try FileManager.default.removeItem(at: url) }
        return url
    }

    func testReadsExactEntryBytes() async throws {
        let archive = try fixture(compressed)
        let bytes = try await ArchiveSelectiveExtractor.shared.extractSolidLzma2PreviewData(
            archivePath: archive.path,
            entryPath: entry
        )
        XCTAssertEqual(bytes, expected)
    }

    func testRootEntryDoesNotResolveToNestedEntryWithSameName() async throws {
        let archive = try fixture("""
            N3q8ryccAAQkSJ3DNwAAAAAAAACOAAAAAAAAACux4W7gAEkAL10ANxlKzh/yERtCwljHbNHa
            NtqG+1K6SpumzkOmD3T2mhlfhHnFuDcz6NJkvLweQAAAAQQGAAEJNwAHCwEAASEhAQAMSgAI
            DQIJJgoBcyJOFiN7F04AAAUCGQQAAAAAETkAYQAvAHoALQB0AGEAcgBnAGUAdAAuAGIAaQBu
            AAAAegAtAHQAYQByAGcAZQB0AC4AYgBpAG4AAAAZAgAAFBIBAOjC+v09dd0BWcb6/T113QEV
            CgEAIICkgSCApIEAAA==
            """)
        let bytes = try await ArchiveSelectiveExtractor.shared.extractSolidLzma2PreviewData(
            archivePath: archive.path,
            entryPath: "z-target.bin"
        )
        XCTAssertEqual(String(decoding: bytes, as: UTF8.self), "root payload for 7z solid targeting\n")
    }

    func testUnsupportedStoredEntryDoesNotRetryLegacyExtraction() async throws {
        let archive = try fixture(stored)
        let legacy = try await ArchiveSelectiveExtractor.shared.extractSingleEntryData(
            archivePath: archive.path,
            entryPath: entry
        )
        XCTAssertEqual(legacy, expected)
        do {
            _ = try await ArchiveSelectiveExtractor.shared.extractSolidLzma2PreviewData(
                archivePath: archive.path,
                entryPath: entry
            )
            XCTFail("Opt-in read must reject a stored entry even when the legacy cache can satisfy it")
        } catch SolidLzma2PreviewError.Unsupported {}
    }

    func testCorruptEntryCrcIsTerminal() async throws {
        let archive = try fixture(badEntryCrc)
        do {
            _ = try await ArchiveSelectiveExtractor.shared.extractSolidLzma2PreviewData(
                archivePath: archive.path,
                entryPath: entry
            )
            XCTFail("Opt-in read accepted an incorrect entry CRC")
        } catch SolidLzma2PreviewError.Corrupt {}
    }

    func testOversizedHeaderIsTerminal() async throws {
        let archive = try fixture("N3q8ryccAAQnZWbRNQAAAAAAAAABAAEAAAAAAGgcwpM=")
        do {
            _ = try await ArchiveSelectiveExtractor.shared.extractSolidLzma2PreviewData(
                archivePath: archive.path,
                entryPath: entry
            )
            XCTFail("Opt-in read accepted a next header larger than 64 KiB")
        } catch SolidLzma2PreviewError.BudgetExceeded {}
    }

    func testNamedPipeWithoutWriterIsRejected() async throws {
        let archive = FileManager.default.temporaryDirectory.appendingPathComponent("\(UUID().uuidString).7z")
        guard mkfifo(archive.path, S_IRUSR | S_IWUSR) == 0 else {
            XCTFail("Could not create the named pipe fixture")
            return
        }
        addTeardownBlock { try FileManager.default.removeItem(at: archive) }
        let completed = expectation(description: "Named pipe rejected without a writer")
        let token = CancellationToken()
        let entry = entry
        Task {
            defer { completed.fulfill() }
            do {
                _ = try await ArchiveSelectiveExtractor.shared.extractSolidLzma2PreviewData(
                    archivePath: archive.path,
                    entryPath: entry,
                    token: token
                )
                XCTFail("Opt-in read accepted a named pipe")
            } catch SolidLzma2PreviewError.Unsupported {
            } catch {
                XCTFail("Opt-in read returned \(error) for a named pipe")
            }
        }
        defer { token.cancel() }
        await fulfillment(of: [completed], timeout: 3)
    }

    func testPreCancelledTokenIsTerminal() async throws {
        let archive = try fixture(compressed)
        let token = CancellationToken()
        token.cancel()
        do {
            _ = try await ArchiveSelectiveExtractor.shared.extractSolidLzma2PreviewData(
                archivePath: archive.path,
                entryPath: entry,
                token: token
            )
            XCTFail("Opt-in read accepted a cancelled token")
        } catch SolidLzma2PreviewError.Cancelled {}
    }
}
