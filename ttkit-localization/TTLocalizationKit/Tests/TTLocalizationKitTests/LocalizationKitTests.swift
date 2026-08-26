// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTKit: High-performance cross-platform system utility foundation.

import XCTest
@testable import TTLocalizationCore
@testable import TTLocalizationIPC

enum MockLocaleKey: String, LocaleKeyProtocol {
    case cancel = "common.cancel"
    case ok = "common.ok"
}

final class LocalizationKitTests: XCTestCase {
    func testLanguageEnumParsing() {
        XCTAssertEqual(AppLanguage.from(identifier: "zh-Hans-CN"), .zhHans)
        XCTAssertEqual(AppLanguage.from(identifier: "zh-Hant-TW"), .zhHant)
        XCTAssertEqual(AppLanguage.from(identifier: "de-DE"), .de)
        XCTAssertEqual(AppLanguage.from(identifier: "en-US"), .en)
    }
    
    func testLocalizationManagerFallback() {
        let manager = TTLocalizationManager.shared
        manager.currentLanguage = .en
        
        manager.registerLookupHandler { key, lang in
            if key == "common.ok" && lang == .zhHans {
                return "好"
            } else if key == "common.ok" && lang == .en {
                return "OK"
            }
            return ""
        }
        
        XCTAssertEqual(manager.string(for: MockLocaleKey.ok, language: .zhHans), "好")
        XCTAssertEqual(manager.string(for: MockLocaleKey.ok, language: .en), "OK")
    }
}
