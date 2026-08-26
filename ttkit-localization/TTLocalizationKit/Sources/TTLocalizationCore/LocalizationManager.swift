// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTKit: High-performance cross-platform system utility foundation.

import Foundation


/// Supported application language enumeration.
public enum AppLanguage: String, Sendable, CaseIterable, Identifiable {
    case en = "en"
    case zhHans = "zh-Hans"
    case zhHant = "zh-Hant"
    case ja = "ja"
    case de = "de"
    case fr = "fr"
    case es = "es"
    
    public var id: String { rawValue }
    public var bcp47: String { rawValue }
    
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
    
    public static func from(identifier: String) -> AppLanguage? {
        let clean = identifier.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if clean.starts_with("zh-hant") || clean.starts_with("zh-tw") || clean.starts_with("zh-hk") {
            return .zhHant
        } else if clean.starts_with("zh") {
            return .zhHans
        } else if clean.starts_with("ja") {
            return .ja
        } else if clean.starts_with("de") {
            return .de
        } else if clean.starts_with("fr") {
            return .fr
        } else if clean.starts_with("es") {
            return .es
        } else if clean.starts_with("en") {
            return .en
        }
        return nil
    }
}

private extension String {
    func starts_with(_ prefix: String) -> Bool {
        self.hasPrefix(prefix)
    }
}

/// Byte sizing standard specification.
public enum ByteSizeStandard: String, Sendable, CaseIterable {
    case metricSI = "MetricSI"
    case binaryIEC = "BinaryIEC"
}

/// Centralized thread-safe localization manager.
public final class TTLocalizationManager: @unchecked Sendable {
    public static let shared = TTLocalizationManager()
    
    private let lock = NSLock()
    private var _currentLanguage: AppLanguage = .en
    private var catalogLookupHandler: (@Sendable (String, AppLanguage) -> String)?
    
    public var currentLanguage: AppLanguage {
        get {
            lock.lock()
            defer { lock.unlock() }
            return _currentLanguage
        }
        set {
            lock.lock()
            _currentLanguage = newValue
            lock.unlock()
        }
    }
    
    private init() {
        if let env = ProcessInfo.processInfo.environment["LANG"], let parsed = AppLanguage.from(identifier: env) {
            self._currentLanguage = parsed
        } else if let preferred = Locale.preferredLanguages.first, let parsed = AppLanguage.from(identifier: preferred) {
            self._currentLanguage = parsed
        } else {
            self._currentLanguage = .en
        }
    }
    
    /// Register custom backend lookup handler (e.g. bridging to UniFFI).
    public func registerLookupHandler(_ handler: @escaping @Sendable (String, AppLanguage) -> String) {
        lock.lock()
        self.catalogLookupHandler = handler
        lock.unlock()
    }
    
    /// Resolves localized string for a key in target or current language.
    public func string(for key: any LocaleKeyProtocol, language: AppLanguage? = nil) -> String {
        let lang = language ?? currentLanguage
        lock.lock()
        let handler = self.catalogLookupHandler
        lock.unlock()
        
        if let handler = handler {
            let res = handler(key.rawKey, lang)
            if !res.isEmpty {
                return res
            }
        }
        return key.rawKey
    }
}
