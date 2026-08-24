// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation

/// Centralized thread-safe localization manager bridging to Rust UniFFI localization engine.
public final class TTZipLocalizationManager: @unchecked Sendable {
    public static let shared = TTZipLocalizationManager()
    
    private let lock = NSLock()
    private var _currentLanguage: AppLanguage
    
    /// Current active application and CLI interaction language.
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
        // 1. Inspect shared AppGroup preferences store
        if let saved = TTZipPreferencesStore.getSavedLanguage() {
            self._currentLanguage = saved
            return
        }
        
        // 2. Inspect POSIX environment variables (LC_ALL / LANG)
        let env = ProcessInfo.processInfo.environment
        if let lcAll = env["LC_ALL"], let parsed = AppLanguage.from(identifier: lcAll) {
            self._currentLanguage = parsed
            return
        }
        if let lang = env["LANG"], let parsed = AppLanguage.from(identifier: lang) {
            self._currentLanguage = parsed
            return
        }
        
        // 3. Query system preferred languages
        if let preferred = Locale.preferredLanguages.first, let parsed = AppLanguage.from(identifier: preferred) {
            self._currentLanguage = parsed
            return
        }
        
        // 4. Fallback to English (en)
        self._currentLanguage = .en
    }
    
    /// Resolves localized string for a key in the target or current active language.
    public func string(for key: any LocaleKeyProtocol, language: AppLanguage? = nil) -> String {
        let targetLanguage = language ?? currentLanguage
        let rawKey = key.rawKey
        let res = ttzipI18nGetString(key: rawKey, lang: targetLanguage)
        if !res.isEmpty {
            return res
        }
        return rawKey
    }
    
    /// Maps language enum to corresponding string catalog dictionary via Rust engine.
    public func catalog(for language: AppLanguage) -> [String: String] {
        var dict: [String: String] = [:]
        for key in L10n.allRawKeys {
            let res = ttzipI18nGetString(key: key, lang: language)
            dict[key] = res.isEmpty ? key : res
        }
        return dict
    }
}

// MARK: - Formatters & Extensions

extension ByteSizeStandard: @unchecked Sendable {
    public static var metricSI: ByteSizeStandard { .metricSi }
    public static var binaryIEC: ByteSizeStandard { .binaryIec }
}

/// Zero-heap-allocation byte capacity formatting engine powered by Rust.
public enum ByteSizeFormatter {
    
    /// Formats byte count according to standard and localized decimal conventions.
    public static func format(bytes: Int64, style: ByteSizeStandard = .metricSi, language: AppLanguage = .en) -> String {
        ttzipI18nFormatBytes(bytes: bytes, standard: style, lang: language)
    }
}

/// High-performance throughput rate formatting engine powered by Rust.
public enum ThroughputFormatter {
    
    /// Formats throughput rate in MB/s with locale-sensitive decimal formatting.
    public static func format(mbPerSec: Double, language: AppLanguage = .en) -> String {
        ttzipI18nFormatThroughput(mbPerSec: mbPerSec, lang: language)
    }
}

/// Unicode CLDR plural categories.
public enum PluralCategory: Sendable {
    case zero
    case one
    case two
    case few
    case many
    case other
}

/// Evaluator engine for Unicode CLDR plural rules across supported languages.
public enum PluralRuleEngine {
    
    /// Evaluates plural category given item count and language.
    public static func evaluate(count: Int64, language: AppLanguage) -> PluralCategory {
        switch language {
        case .zhHans, .zhHant, .ja:
            return .other
            
        case .en, .de, .es:
            return (count == 1) ? .one : .other
            
        case .fr:
            return (count == 0 || count == 1) ? .one : .other
        }
    }
}

extension ArchiveError {
    /// Localized error description for the current or specified language.
    public func localizedDescription(for language: AppLanguage? = nil) -> String {
        let manager = TTZipLocalizationManager.shared
        let targetLang = language ?? manager.currentLanguage
        let locale = Locale(identifier: targetLang.bcp47)
        
        switch self {
        case .fileNotFound:
            return manager.string(for: L10n.Errors.fileNotFound, language: targetLang)
        case .readFailed(let code):
            let template = manager.string(for: L10n.Errors.readError, language: targetLang)
            return String(format: template, locale: locale, code)
        case .invalidFormat:
            return manager.string(for: L10n.Errors.unsupportedFormat, language: targetLang)
        case .passwordRequired:
            return manager.string(for: L10n.Errors.passwordRequired, language: targetLang)
        case .passwordRequiredDetailed(let archivePath, let tier):
            let fileName = (archivePath as NSString).lastPathComponent
            if tier == .headerAndData {
                let template = manager.string(for: L10n.Errors.passwordRequiredHeaderAndData, language: targetLang)
                return String(format: template, locale: locale, fileName)
            } else {
                let template = manager.string(for: L10n.Errors.passwordRequiredPayload, language: targetLang)
                return String(format: template, locale: locale, fileName)
            }
        case .wrongPassword:
            return manager.string(for: L10n.Errors.incorrectPassword, language: targetLang)
        case .unsupportedEncryptionMethod(let path, let method):
            let template = manager.string(for: L10n.Errors.unsupportedEncryption, language: targetLang)
            let fileName = (path as NSString).lastPathComponent
            return String(format: template, locale: locale, method, fileName)
        case .corruptedData(let archivePath, let entryPath):
            let template = manager.string(for: L10n.Errors.corruptData, language: targetLang)
            let fileName = (archivePath as NSString).lastPathComponent
            return "\(template) (\(fileName): \(entryPath))"
        case .cancelled:
            return manager.string(for: L10n.Errors.operationCancelled, language: targetLang)
        case .invalidState:
            return manager.string(for: L10n.Common.error, language: targetLang)
        case .engineFailure(let code, let message):
            let template = manager.string(for: L10n.Errors.engineFailure, language: targetLang)
            return String(format: template, locale: locale, code, message)
        }
    }
}
