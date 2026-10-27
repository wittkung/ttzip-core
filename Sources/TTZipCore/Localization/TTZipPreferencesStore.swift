// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
    
    private static let lock = NSLock()
    private nonisolated(unsafe) static var inMemoryLanguage: AppLanguage?
    private nonisolated(unsafe) static var inMemoryByteStandard: ByteSizeStandard?
    
    /// Returns the AppGroup shared `UserDefaults` suite if available, falling back to standard.
    public nonisolated(unsafe) static let sharedDefaults: UserDefaults = {
        UserDefaults(suiteName: appGroupID) ?? UserDefaults.standard
    }()
    
    /// Reads saved language across in-memory cache, AppGroup suite, and standard defaults.
    public static func getSavedLanguage() -> AppLanguage? {
        lock.lock()
        if let memory = inMemoryLanguage {
            lock.unlock()
            return memory
        }
        lock.unlock()
        
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
    
    /// Persists language selection across in-memory cache, AppGroup suite, and standard defaults, broadcasting Darwin notification.
    public static func saveLanguage(_ language: AppLanguage) {
        lock.lock()
        inMemoryLanguage = language
        lock.unlock()
        
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
        lock.lock()
        if let memory = inMemoryByteStandard {
            lock.unlock()
            return memory
        }
        lock.unlock()
        
        let stored = sharedDefaults.string(forKey: byteStandardKey) ?? UserDefaults.standard.string(forKey: byteStandardKey)
        return (stored == "binaryIEC") ? .binaryIEC : .metricSI
    }
    
    /// Persists byte unit standard across in-memory cache, AppGroup suite, and standard defaults.
    public static func saveByteStandard(_ standard: ByteSizeStandard) {
        lock.lock()
        inMemoryByteStandard = standard
        lock.unlock()
        
        let val = (standard == .binaryIEC) ? "binaryIEC" : "metricSI"
        sharedDefaults.set(val, forKey: byteStandardKey)
        UserDefaults.standard.set(val, forKey: byteStandardKey)
    }
}
