// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import XCTest
@testable import TTZipCore

final class PresetAndCapabilitiesTests: XCTestCase {
    func testFormatCapabilitiesDelegation() {
        // Zip format capabilities
        let zip = ArchiveCompressionFormat.zip
        XCTAssertTrue(zip.supportsPasswordEncryption)
        XCTAssertTrue(zip.supportsSplitVolume)
        XCTAssertEqual(zip.minCompressionLevel, 0)
        XCTAssertEqual(zip.maxCompressionLevel, 12)
        XCTAssertEqual(zip.defaultCompressionLevel, 6)

        // 7z format capabilities
        let sevenZ = ArchiveCompressionFormat.sevenZip
        XCTAssertTrue(sevenZ.supportsPasswordEncryption)
        XCTAssertTrue(sevenZ.supportsSplitVolume)
        XCTAssertTrue(sevenZ.supportsSolidMode)
        XCTAssertEqual(sevenZ.minCompressionLevel, 0)
        XCTAssertEqual(sevenZ.maxCompressionLevel, 9)
        XCTAssertEqual(sevenZ.defaultCompressionLevel, 6)

        // Tar format capabilities
        let tar = ArchiveCompressionFormat.tar
        XCTAssertFalse(tar.supportsPasswordEncryption)
        XCTAssertFalse(tar.supportsSplitVolume)
        XCTAssertTrue(tar.supportsSolidMode)
        XCTAssertEqual(tar.minCompressionLevel, 0)
        XCTAssertEqual(tar.maxCompressionLevel, 0)
        XCTAssertEqual(tar.defaultCompressionLevel, 0)

        // Zstandard format capabilities
        let zstd = ArchiveCompressionFormat.zst
        XCTAssertFalse(zstd.supportsPasswordEncryption)
        XCTAssertFalse(zstd.supportsSplitVolume)
        XCTAssertEqual(zstd.minCompressionLevel, 0)
        XCTAssertEqual(zstd.maxCompressionLevel, 22)
        XCTAssertEqual(zstd.defaultCompressionLevel, 3)
    }

    func testFormatParameterValidation() {
        let zip = ArchiveCompressionFormat.zip
        XCTAssertNoThrow(try zip.validateConfiguration(level: 6, hasPassword: true, isSplit: true))
        XCTAssertThrowsError(try zip.validateConfiguration(level: 15, hasPassword: false, isSplit: false))

        let tar = ArchiveCompressionFormat.tar
        XCTAssertNoThrow(try tar.validateConfiguration(level: 0, hasPassword: false, isSplit: false))
        XCTAssertThrowsError(try tar.validateConfiguration(level: 0, hasPassword: true, isSplit: false))
        XCTAssertThrowsError(try tar.validateConfiguration(level: 0, hasPassword: false, isSplit: true))
        XCTAssertThrowsError(try tar.validateConfiguration(level: 5, hasPassword: false, isSplit: false))
    }

    func testPresetManagerRecommendedLevel() {
        let manager = PresetManager.shared
        XCTAssertEqual(manager.recommendedLevel(for: "store", format: .sevenZip), .store)
        XCTAssertEqual(manager.recommendedLevel(for: "fast", format: .sevenZip), .level1)
        XCTAssertEqual(manager.recommendedLevel(for: "balanced", format: .sevenZip), .level6)
        XCTAssertEqual(manager.recommendedLevel(for: "maximum", format: .sevenZip), .level9)
        XCTAssertEqual(manager.recommendedLevel(for: "ultra", format: .zip), .level12)
        XCTAssertEqual(manager.recommendedLevel(for: "ultra", format: .zst), .level19)
    }

    func testPresetValidationAndPersistence() throws {
        let validPreset = CompressionPreset(
            name: "Test 7Z Valid",
            format: .sevenZip,
            level: .normal,
            splitVolumeSizeBytes: 100 * 1024 * 1024,
            defaultPassword: "secretPassword",
            skipMacJunk: true
        )
        XCTAssertNoThrow(try validPreset.validate())

        let invalidTarPreset = CompressionPreset(
            name: "Test Tar Invalid Encryption",
            format: .tar,
            level: .store,
            splitVolumeSizeBytes: nil,
            defaultPassword: "cannotEncryptTar",
            skipMacJunk: true
        )
        XCTAssertThrowsError(try invalidTarPreset.validate())

        let suiteName = "test_preset_suite_\(UUID().uuidString)"
        guard let customDefaults = UserDefaults(suiteName: suiteName) else {
            XCTFail("Failed to create UserDefaults test suite")
            return
        }
        defer { customDefaults.removePersistentDomain(forName: suiteName) }

        let customManager = PresetManager(userDefaults: customDefaults, storageKey: "TestPresetsKey")
        XCTAssertFalse(customManager.presets.isEmpty)

        customManager.savePreset(validPreset)
        XCTAssertEqual(customManager.preset(for: validPreset.id)?.name, "Test 7Z Valid")

        let duplicated = customManager.duplicatePreset(id: validPreset.id, newName: "Duplicated Preset")
        XCTAssertNotNil(duplicated)
        XCTAssertEqual(duplicated?.name, "Duplicated Preset")
        XCTAssertNotEqual(duplicated?.id, validPreset.id)

        if let dupId = duplicated?.id {
            customManager.deletePreset(id: dupId)
            XCTAssertNil(customManager.preset(for: dupId))
        }
    }
}
