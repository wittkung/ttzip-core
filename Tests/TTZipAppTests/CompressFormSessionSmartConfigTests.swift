// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore
@testable import TTZipApp

final class CompressFormSessionSmartConfigTests: XCTestCase {
    
    // MARK: - 1. Store Mode Tests
    
    @MainActor
    func testStoreModeDefaultsAndZeroDictionary() {
        let session = CompressFormSession()
        session.selectedFormat = .sevenZip
        session.compressionLevel = .store
        
        XCTAssertEqual(session.effectiveDictionarySizeMB, 0, "Store mode must result in 0MB dictionary")
        XCTAssertNil(session.customDictionarySizeMB, "Default session should have nil customDictionarySizeMB")
    }
    
    // MARK: - 2. Level-Driven Dictionary Scaling
    
    @MainActor
    func testLevelDrivenDictionaryScaling() {
        let session = CompressFormSession()
        session.customDictionarySizeMB = nil
        
        // Fastest Level (1) -> 16 MB
        session.compressionLevel = .level1
        XCTAssertEqual(session.effectiveDictionarySizeMB, 16, "Level 1 (Fastest) should default to 16 MB dictionary")
        
        // Normal Level (5 / 6) -> 64 MB
        session.compressionLevel = .level6
        XCTAssertEqual(session.effectiveDictionarySizeMB, 64, "Level 6 (Normal) should default to 64 MB dictionary")
        
        // Ultra Level (9) -> Scales according to Apple Silicon unified memory (>= 128 MB)
        session.compressionLevel = .level9
        let ultraDict = session.effectiveDictionarySizeMB
        XCTAssertTrue([128, 256, 512, 1024].contains(ultraDict), "Ultra Level 9 should be 128MB, 256MB, 512MB, or 1024MB depending on RAM (was: \(ultraDict) MB)")
    }
    
    // MARK: - 3. Custom Dictionary Override
    
    @MainActor
    func testCustomDictionaryOverride() {
        let session = CompressFormSession()
        session.compressionLevel = .level6
        XCTAssertEqual(session.effectiveDictionarySizeMB, 64)
        
        // Explicitly override with 512 MB
        session.customDictionarySizeMB = 512
        XCTAssertEqual(session.effectiveDictionarySizeMB, 512, "Explicit custom dictionary must override level default")
        
        // Explicitly override with 1024 MB (1 GB)
        session.customDictionarySizeMB = 1024
        XCTAssertEqual(session.effectiveDictionarySizeMB, 1024, "1GB custom dictionary must be accepted")
        
        // Reset to automatic
        session.customDictionarySizeMB = nil
        XCTAssertEqual(session.effectiveDictionarySizeMB, 64, "Setting customDictionarySizeMB to nil must restore level-driven automatic calculation")
    }
    
    // MARK: - 4. 7-Zip Safety Limits for Ultra Dictionaries
    
    func testUltraLargeDictionarySafetyBounds() {
        let supportedDictSizes: [Int] = [16, 32, 64, 128, 256, 512, 1024, 1536]
        
        for size in supportedDictSizes {
            XCTAssertLessThanOrEqual(size, 1536, "Dictionary size \(size) MB must not exceed 7-Zip LZMA2 1536 MB architecture limit")
            XCTAssertGreaterThan(size, 0, "Dictionary size \(size) MB must be positive")
        }
    }
    
    // MARK: - 5. Format Level Compatibility
    
    @MainActor
    func testFormatSupportedLevelsValidation() {
        // TAR, DMG, ISO, AAR strictly support .store only
        let pureContainers: [ArchiveCompressionFormat] = [.tar, .dmg, .iso, .aar]
        for fmt in pureContainers {
            XCTAssertEqual(fmt.supportedLevels, [.store], "Pure container format \(fmt.rawValue) must only support .store")
        }
        
        // 7Z and ZSTD support multi-level (.store, .level1, .level6, .level9)
        XCTAssertTrue(ArchiveCompressionFormat.sevenZip.supportedLevels.contains(.store))
        XCTAssertTrue(ArchiveCompressionFormat.sevenZip.supportedLevels.contains(.level9))
        XCTAssertTrue(ArchiveCompressionFormat.zst.supportedLevels.contains(.store))
        XCTAssertTrue(ArchiveCompressionFormat.zst.supportedLevels.contains(.level9))
    }
    
    // MARK: - 6. i18n Completeness for All New Keys Across 7 Locales
    
    func testI18nCompletenessAcrossAll7Locales() {
        let manager = TTZipLocalizationManager.shared
        let allLanguages: [AppLanguage] = [.en, .zhHans, .zhHant, .ja, .de, .fr, .es]
        
        let newKeys: [any LocaleKeyProtocol] = [
            L10n.Compress.algorithm,
            L10n.Compress.dictAutoFormat,
            L10n.Compress.dictSpeedUnit,
            L10n.Compress.dictStandardUnit,
            L10n.Compress.dictLargeMemoryUnit,
            L10n.Compress.dictUltraUnit,
            L10n.Compress.dictPhysicalLimitUnit,
            L10n.Compress.solidArchiveDesc,
            L10n.Compress.encryptFileNames7z,
            L10n.Compress.zipMethod,
            L10n.Compress.zipMethodAes,
            L10n.Compress.zipMethodZipCrypto,
            L10n.Compress.zstdLevel,
            L10n.Compress.zstdLdm
        ]
        
        for lang in allLanguages {
            for key in newKeys {
                let localizedStr = manager.string(for: key, language: lang)
                XCTAssertFalse(
                    localizedStr.isEmpty,
                    "Key '\(key.rawKey)' must have a non-empty translation for language '\(lang.bcp47)'"
                )
            }
        }
    }
}
