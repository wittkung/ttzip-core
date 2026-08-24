// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.

import XCTest
@testable import TTZipCore

final class TTZipLocalizationSecurityTests: XCTestCase {
    
    // MARK: - 1. Full Key Coverage Gate
    
    func testAllCatalogsHaveEqualKeyCountAndNoMissingKeys() {
        let manager = TTZipLocalizationManager.shared
        let enCatalog = manager.catalog(for: .en)
        let enKeys = Set(enCatalog.keys)
        
        XCTAssertGreaterThan(enKeys.count, 350, "English catalog should contain > 350 localized keys")
        
        let targetLanguages: [AppLanguage] = [.zhHans, .zhHant, .ja, .de, .fr, .es]
        
        for lang in targetLanguages {
            let cat = manager.catalog(for: lang)
            let catKeys = Set(cat.keys)
            
            let missingKeys = enKeys.subtracting(catKeys)
            XCTAssertTrue(
                missingKeys.isEmpty,
                "Language [\(lang.rawValue)] is missing keys: \(missingKeys)"
            )
            
            let extraKeys = catKeys.subtracting(enKeys)
            XCTAssertTrue(
                extraKeys.isEmpty,
                "Language [\(lang.rawValue)] has unrecognized extra keys: \(extraKeys)"
            )
            
            // Ensure no empty string values
            for (key, val) in cat {
                XCTAssertFalse(val.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, "Key [\(key)] in [\(lang.rawValue)] is empty")
            }
        }
    }
    
    // MARK: - 2. Anti-Fake Localization Gate
    
    func testAntiFakeLocalizationThresholds() {
        let manager = TTZipLocalizationManager.shared
        let enCatalog = manager.catalog(for: .en)
        let nonEnglishLanguages: [AppLanguage] = [.de, .fr, .es, .ja]
        
        for lang in nonEnglishLanguages {
            let cat = manager.catalog(for: lang)
            var identicalCount = 0
            
            for (k, enVal) in enCatalog {
                if let targetVal = cat[k], targetVal == enVal {
                    // Allow short technical acronyms/standards to be identical (e.g. "AES-256", "7-Zip", "MD5", "SHA-256", "UTF-8", "POSIX")
                    if enVal.count > 5 && !enVal.contains("7-Zip") && !enVal.contains("AES") && !enVal.contains("SHA") {
                        identicalCount += 1
                    }
                }
            }
            
            let duplicateRatio = Double(identicalCount) / Double(enCatalog.count)
            XCTAssertLessThan(
                duplicateRatio,
                0.15,
                "Language [\(lang.rawValue)] has suspicious fake-localization duplicate ratio: \(String(format: "%.1f%%", duplicateRatio * 100.0)) (> 15%)"
            )
        }
    }
    
    // MARK: - 3. Format Specifier Crash Safety Gate
    
    func testFormatSpecifiersConsistencyAcrossLanguages() {
        let manager = TTZipLocalizationManager.shared
        
        for lang in AppLanguage.allCases {
            let cat = manager.catalog(for: lang)
            let locale = Locale(identifier: lang.bcp47)
            
            for (_, targetVal) in cat {
                if targetVal.contains("%") {
                    // Test formatting safety with synthetic arguments
                    if targetVal.contains("%d") && targetVal.contains("%@") {
                        let formatted = String(format: targetVal, locale: locale, "TestFile.zip", 42)
                        XCTAssertFalse(formatted.isEmpty)
                    } else if targetVal.contains("%@") {
                        let formatted = String(format: targetVal, locale: locale, "TestString")
                        XCTAssertFalse(formatted.isEmpty)
                    } else if targetVal.contains("%d") {
                        let formatted = String(format: targetVal, locale: locale, 100)
                        XCTAssertFalse(formatted.isEmpty)
                    }
                }
            }
        }
    }
    
    // MARK: - 4. ByteSizeFormatter Standards & Locale Delimiters
    
    func testByteSizeFormatterDecimalSeparators() {
        let bytes: Int64 = 1536 * 1024 // 1.5 MB in SI or 1.5 MiB
        
        let enSI = ByteSizeFormatter.format(bytes: bytes, style: .metricSI, language: .en)
        XCTAssertTrue(enSI.contains("."), "English should use '.' decimal separator")
        XCTAssertTrue(enSI.contains("MB") || enSI.contains("KB"))
        
        let deSI = ByteSizeFormatter.format(bytes: bytes, style: .metricSI, language: .de)
        XCTAssertTrue(deSI.contains(","), "German should use ',' decimal separator")
        
        let frSI = ByteSizeFormatter.format(bytes: bytes, style: .metricSI, language: .fr)
        XCTAssertTrue(frSI.contains(","), "French should use ',' decimal separator")
        
        let esSI = ByteSizeFormatter.format(bytes: bytes, style: .metricSI, language: .es)
        XCTAssertTrue(esSI.contains(","), "Spanish should use ',' decimal separator")
        
        let zhIEC = ByteSizeFormatter.format(bytes: bytes, style: .binaryIEC, language: .zhHans)
        XCTAssertTrue(zhIEC.contains("KiB") || zhIEC.contains("MiB"))
    }
    
    // MARK: - 5. ThroughputFormatter
    
    func testThroughputFormatter() {
        let rate = 1250.75
        let enStr = ThroughputFormatter.format(mbPerSec: rate, language: .en)
        XCTAssertEqual(enStr, "1250.8 MB/s")
        
        let deStr = ThroughputFormatter.format(mbPerSec: rate, language: .de)
        XCTAssertEqual(deStr, "1250,8 MB/s")
    }
    
    // MARK: - 6. Plural Rules Evaluation
    
    func testPluralRuleEngine() {
        XCTAssertEqual(PluralRuleEngine.evaluate(count: 0, language: .en), .other)
        XCTAssertEqual(PluralRuleEngine.evaluate(count: 1, language: .en), .one)
        XCTAssertEqual(PluralRuleEngine.evaluate(count: 2, language: .en), .other)
        
        XCTAssertEqual(PluralRuleEngine.evaluate(count: 0, language: .fr), .one)
        XCTAssertEqual(PluralRuleEngine.evaluate(count: 1, language: .fr), .one)
        XCTAssertEqual(PluralRuleEngine.evaluate(count: 2, language: .fr), .other)
        
        XCTAssertEqual(PluralRuleEngine.evaluate(count: 0, language: .zhHans), .other)
        XCTAssertEqual(PluralRuleEngine.evaluate(count: 1, language: .zhHans), .other)
        XCTAssertEqual(PluralRuleEngine.evaluate(count: 100, language: .zhHans), .other)
    }
    
    // MARK: - 7. ArchiveError Localized Descriptions
    
    func testArchiveErrorLocalizationInAllLanguages() {
        let errors: [ArchiveError] = [
            .fileNotFound,
            .readFailed(code: -1),
            .invalidFormat,
            .passwordRequired,
            .passwordRequiredDetailed(archivePath: "/tmp/secret.7z", tier: .headerAndData),
            .wrongPassword(archivePath: "/tmp/secret.7z"),
            .unsupportedEncryptionMethod(archivePath: "/tmp/data.zip", method: "AES-512"),
            .corruptedData(archivePath: "/tmp/corrupt.zip", entryPath: "file.txt"),
            .cancelled,
            .engineFailure(code: 500, message: "Internal memory fault")
        ]
        
        for lang in AppLanguage.allCases {
            for err in errors {
                let desc = err.localizedDescription(for: lang)
                XCTAssertFalse(desc.isEmpty, "Error description for [\(err)] in [\(lang.rawValue)] should not be empty")
            }
        }
    }
    
    // MARK: - 8. PasswordStrengthTier Localization
    
    func testPasswordStrengthTiers() {
        for lang in AppLanguage.allCases {
            for tier in PasswordStrengthTier.allCases {
                let label = tier.localizedLabel(language: lang)
                XCTAssertFalse(label.isEmpty, "Tier [\(tier)] should have valid label in [\(lang.rawValue)]")
            }
        }
    }
}
