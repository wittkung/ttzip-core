// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
import TTZipArchiveContract
import TTZipOperationContract

final class TTZipOperationContractTests: XCTestCase {
    func testHashTypeRawValues() {
        XCTAssertEqual(HashType.crc32.rawValue, "CRC32")
        XCTAssertEqual(HashType.sha256.rawValue, "SHA-256")
        XCTAssertEqual(HashType.md5.rawValue, "MD5")
        XCTAssertEqual(HashType.sha1.rawValue, "SHA-1")
    }

    func testArchiveCompressionFormatResolutionAndCategories() {
        XCTAssertEqual(ArchiveCompressionFormat.from(extensionOrName: ".tgz"), .tarGz)
        XCTAssertEqual(ArchiveCompressionFormat.from(extensionOrName: "CB7"), .sevenZip)
        XCTAssertEqual(ArchiveCompressionFormat.from(extensionOrName: "7-Zip"), .sevenZip)
        XCTAssertEqual(ArchiveCompressionFormat.zst.correspondingTarCompositeFormat, .tarZst)
        XCTAssertEqual(ArchiveCompressionFormat.sevenZip.category, .standard)
        XCTAssertEqual(ArchiveCompressionFormat.tarZst.category, .unixPackage)
        XCTAssertEqual(ArchiveCompressionFormat.dmg.category, .diskImage)
        XCTAssertEqual(ArchiveCompressionFormat.lz4.category, .modernStream)
        XCTAssertTrue(ArchiveCompressionFormat.rar.isReadOnly)
        XCTAssertFalse(ArchiveCompressionFormat.rar.isWritable)
        XCTAssertTrue(ArchiveCompressionFormat.zip.supportsRecoveryRecord)
        XCTAssertTrue(ArchiveCompressionFormat.isArchiveExtension("001", path: "/tmp/archive.7z.001"))
        XCTAssertEqual(ArchiveCompressionFormat.kindDescription(forExtension: "tgz", isArchive: true), "TAR.GZ Archive")
        XCTAssertEqual(ArchiveCompressionFormat.kindDescription(forExtension: "swift", isArchive: false), "Source Code")
    }

    func testArchiveFilterAndOptionsMatchingAgainstArchiveEntry() {
        let swiftEntry = ArchiveEntry(path: "Sources/App/Main.swift", uncompressedSize: 4096, isDirectory: false)
        let dsStoreEntry = ArchiveEntry(path: "Sources/.DS_Store", uncompressedSize: 128, isDirectory: false)
        let gitEntry = ArchiveEntry(path: ".git/config", uncompressedSize: 256, isDirectory: false)

        XCTAssertTrue(ArchiveFilter(expression: "ext:swift AND size:>1024").evaluate(entry: swiftEntry))
        XCTAssertFalse(ArchiveFilter(expression: "ext:rs OR size:<512").evaluate(entry: swiftEntry))
        XCTAssertEqual(ArchiveFilter.filter(entries: [swiftEntry, dsStoreEntry], expression: "ext:swift"), [swiftEntry])

        let options = ArchiveFilterOptions(skipMacJunk: true, skipGitDirectory: true, customIgnorePatterns: [" Vendor/"])
        XCTAssertTrue(options.matches(entry: swiftEntry, dslQuery: "ext:swift"))
        XCTAssertFalse(options.matches(entry: dsStoreEntry))
        XCTAssertFalse(options.matches(entry: gitEntry))
        XCTAssertTrue(ArchiveFilterOptions.isSystemMetadata(path: "./__MACOSX/._Main.swift"))

        let mutated = options.clone { $0.flattenPaths = true }
        XCTAssertTrue(mutated.flattenPaths)
        XCTAssertFalse(options.flattenPaths)
    }

    func testCompressionLevelsFormatOptionsAndProfileClone() {
        XCTAssertEqual(ArchiveCompressionLevel(levelInt: -99), .fast5)
        XCTAssertEqual(ArchiveCompressionLevel(levelInt: 99), .level22)
        XCTAssertEqual(ArchiveCompressionLevel.normal.effectiveZipRawLevel, 6)
        XCTAssertEqual(ArchiveCompressionLevel.store.speedBadge(), "100% Speed")

        var advanced = ArchiveAdvancedOptions(
            cpuThreads: 8,
            recoveryRecordRedundancyPercent: 5,
            zipOptions: ZipFormatOptions(zipEncryptionMethod: "AES-256", enableZeroCopy: false),
            sevenZipOptions: SevenZipFormatOptions(dictionarySizeMB: 128, encryptFileNames: true),
            zstdOptions: ZstdFormatOptions(zstdLevel: 9, zstdEnableLDM: true)
        )
        let clonedAdvanced = advanced.clone()
        advanced.dictionarySizeMB = 32
        XCTAssertEqual(clonedAdvanced.dictionarySizeMB, 128)
        XCTAssertTrue(clonedAdvanced.encryptFileNames)
        XCTAssertFalse(clonedAdvanced.enableZeroCopy)
        XCTAssertEqual(clonedAdvanced.zstdLevel, 9)

        let profile = ZipCompressionProfile.profile(for: .level4)
        XCTAssertEqual(profile, .graphFast)
        let clonedProfile = profile.clone()
        XCTAssertEqual(clonedProfile, profile)
        XCTAssertEqual(clonedProfile.zopfliIterations, 2)
    }

    func testOperationProgressWriteRequestAndTransactionDiagnostics() {
        let progress = ArchiveProgress(state: .processing, bytesProcessed: 512, totalBytes: 1024, currentFileName: "a.bin", throughputMBs: .nan)
        XCTAssertEqual(progress.fractionCompleted, 0.5, accuracy: 0.0001)
        XCTAssertEqual(progress.throughputMBs, 0.0)

        let info = ArchiveProgressInfo(bytesProcessed: 2048, totalBytes: 1024, estimatedTimeRemaining: -1, operationType: .extract)
        XCTAssertEqual(info.fractionCompleted, 1.0, accuracy: 0.0001)
        XCTAssertNil(info.estimatedTimeRemaining)

        let batch = BatchProgressInfo(completedTasks: 2, totalTasks: 4, currentTaskPath: "/tmp/a.zip")
        XCTAssertEqual(batch.fractionCompleted, 0.5, accuracy: 0.0001)

        let request = ArchiveWriteRequest(outputPath: "/tmp/out.7z", format: .sevenZip, level: .ultra, inputPaths: ["/tmp/in.txt"])
        XCTAssertEqual(request.format, .sevenZip)
        XCTAssertEqual(request.level, .ultra)
        XCTAssertGreaterThanOrEqual(request.advancedOptions.cpuThreads, 1)

        let report = ArchiveCreateTransactionFailureReport(
            primaryStatusCode: -5,
            wasPrimaryCancelled: false,
            isCancellationRequested: false,
            stageDir: "/tmp/.stage",
            stageDirPreserved: true,
            unconfirmedReleaseCount: 1,
            recoveryState: .rollbackIncompletePreservedInBackup,
            summaryMessage: "Stage preserved"
        )
        XCTAssertTrue(report.hasUnconfirmedDescriptorRelease)
        XCTAssertTrue(report.hasCleanupOrRecoveryIssues)
        XCTAssertEqual(ArchiveEncryptionTier.headerAndData.rawValue, "HEADER_AND_DATA")
    }
}
