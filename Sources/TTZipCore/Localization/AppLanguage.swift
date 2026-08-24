// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation

extension AppLanguage: @unchecked Sendable, CaseIterable, Identifiable, RawRepresentable, Codable {
    public static var allCases: [AppLanguage] {
        [.en, .zhHans, .zhHant, .ja, .de, .fr, .es]
    }
    
    public var id: String { bcp47 }
    
    public var rawValue: String { bcp47 }
    
    public init?(rawValue: String) {
        if let parsed = AppLanguage.from(identifier: rawValue) {
            self = parsed
        } else {
            return nil
        }
    }
    
    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let raw = try container.decode(String.self)
        if let parsed = AppLanguage.from(identifier: raw) {
            self = parsed
        } else {
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "Invalid AppLanguage: \(raw)")
        }
    }
    
    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(bcp47)
    }
    
    public var bcp47: String {
        switch self {
        case .en: return "en"
        case .zhHans: return "zh-Hans"
        case .zhHant: return "zh-Hant"
        case .ja: return "ja"
        case .de: return "de"
        case .fr: return "fr"
        case .es: return "es"
        }
    }
    
    public var displayName: String {
        switch self {
        case .en: return "English"
        case .zhHans: return "简体中文"
        case .zhHant: return "繁體中文"
        case .ja: return "日本語"
        case .de: return "Deutsch"
        case .fr: return "Français"
        case .es: return "Español"
        }
    }
    
    /// Parses language from POSIX environment variables (LC_ALL / LANG) or CLI arguments.
    public static func from(identifier: String) -> AppLanguage? {
        let clean = identifier.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if clean.starts(with: "zh-hant") || clean.starts(with: "zh_tw") || clean.starts(with: "zh_hk") {
            return .zhHant
        }
        if clean.starts(with: "zh") {
            return .zhHans
        }
        if clean.starts(with: "ja") {
            return .ja
        }
        if clean.starts(with: "de") {
            return .de
        }
        if clean.starts(with: "fr") {
            return .fr
        }
        if clean.starts(with: "es") {
            return .es
        }
        if clean.starts(with: "en") {
            return .en
        }
        return nil
    }
}
