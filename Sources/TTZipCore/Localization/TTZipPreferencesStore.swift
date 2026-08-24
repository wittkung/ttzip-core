// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
#if canImport(CoreFoundation)
import CoreFoundation
#endif

/// Unified AppGroup shared preferences and cross-process notification synchronization engine.
public enum TTZipPreferencesStore: Sendable {
    public static let appGroupID = "group.com.metastudyline.ttzip"
    public static let languageKey = "TTZip_AppSelectedLanguage"
    public static let byteStandardKey = "TTZip_ByteUnitStandard"
    public static let darwinNotificationName = "com.metastudyline.ttzip.languageChanged"
    
    /// Returns the AppGroup shared `UserDefaults` suite if available, falling back to standard.
    public static var sharedDefaults: UserDefaults {
        UserDefaults(suiteName: appGroupID) ?? UserDefaults.standard
    }
    
    /// Reads saved language across AppGroup suite and standard defaults.
    public static func getSavedLanguage() -> AppLanguage? {
        if let raw = sharedDefaults.string(forKey: languageKey),
           let lang = AppLanguage(rawValue: raw) ?? AppLanguage.from(identifier: raw) {
            return lang
        }
        if let raw = UserDefaults.standard.string(forKey: languageKey),
           let lang = AppLanguage(rawValue: raw) ?? AppLanguage.from(identifier: raw) {
            return lang
        }
        return nil
    }
    
    /// Persists language selection across both AppGroup suite and standard defaults, broadcasting Darwin notification.
    public static func saveLanguage(_ language: AppLanguage) {
        sharedDefaults.set(language.rawValue, forKey: languageKey)
        UserDefaults.standard.set(language.rawValue, forKey: languageKey)
        
        #if canImport(CoreFoundation)
        let notificationName = CFNotificationName(darwinNotificationName as CFString)
        CFNotificationCenterPostNotification(
            CFNotificationCenterGetDarwinNotifyCenter(),
            notificationName,
            nil,
            nil,
            true
        )
        #endif
    }
    
    /// Reads saved byte unit standard.
    public static func getSavedByteStandard() -> ByteSizeStandard {
        let stored = sharedDefaults.string(forKey: byteStandardKey) ?? UserDefaults.standard.string(forKey: byteStandardKey)
        return (stored == "binaryIEC") ? .binaryIEC : .metricSI
    }
    
    /// Persists byte unit standard across AppGroup suite and standard defaults.
    public static func saveByteStandard(_ standard: ByteSizeStandard) {
        let val = (standard == .binaryIEC) ? "binaryIEC" : "metricSI"
        sharedDefaults.set(val, forKey: byteStandardKey)
        UserDefaults.standard.set(val, forKey: byteStandardKey)
    }
}
