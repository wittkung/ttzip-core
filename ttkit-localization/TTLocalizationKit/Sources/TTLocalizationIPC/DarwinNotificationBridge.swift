// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTKit: High-performance cross-platform system utility foundation.

import Foundation
import TTLocalizationCore

/// Cross-process Darwin notification center and AppGroup preference synchronization manager.
public final class DarwinNotificationBridge: @unchecked Sendable {
    public static let shared = DarwinNotificationBridge()
    
    private let appGroupSuite: String
    private let languageKey = "TTKit_AppSelectedLanguage"
    private let byteStandardKey = "TTKit_AppByteSizeStandard"
    private let notificationName: String
    
    public init(
        appGroupSuite: String = "group.com.metastudyline.ttzip",
        notificationName: String = "com.metastudyline.ttzip.languageChanged"
    ) {
        self.appGroupSuite = appGroupSuite
        self.notificationName = notificationName
    }
    
    /// Broadcast language and byte standard change across process sandbox boundaries.
    public func broadcastChange(language: AppLanguage, byteStandard: ByteSizeStandard? = nil) {
        if let defaults = UserDefaults(suiteName: appGroupSuite) {
            defaults.set(language.rawValue, forKey: languageKey)
            if let byteStandard = byteStandard {
                defaults.set(byteStandard.rawValue, forKey: byteStandardKey)
            }
            defaults.synchronize()
        }
        
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        CFNotificationCenterPostNotification(
            center,
            CFNotificationName(notificationName as CFString),
            nil,
            nil,
            true
        )
    }
    
    /// Read saved language from shared AppGroup suite.
    public func getSavedLanguage() -> AppLanguage? {
        guard let defaults = UserDefaults(suiteName: appGroupSuite),
              let raw = defaults.string(forKey: languageKey),
              let parsed = AppLanguage.from(identifier: raw) else {
            return nil
        }
        return parsed
    }
    
    /// Read saved byte standard from shared AppGroup suite.
    public func getSavedByteStandard() -> ByteSizeStandard {
        guard let defaults = UserDefaults(suiteName: appGroupSuite),
              let raw = defaults.string(forKey: byteStandardKey),
              let parsed = ByteSizeStandard(rawValue: raw) else {
            return .metricSI
        }
        return parsed
    }
    
    /// Start listening for cross-process Darwin language change notifications.
    public func observeChanges(onChanged: @escaping @Sendable (AppLanguage) -> Void) {
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        let observerContext = Unmanaged.passRetained(ObserverContext(bridge: self, callback: onChanged)).toOpaque()
        
        let callback: CFNotificationCallback = { _, observer, _, _, _ in
            guard let observer = observer else { return }
            let context = Unmanaged<ObserverContext>.fromOpaque(observer).takeUnretainedValue()
            if let saved = context.bridge.getSavedLanguage() {
                context.callback(saved)
            }
        }
        
        CFNotificationCenterAddObserver(
            center,
            observerContext,
            callback,
            notificationName as CFString,
            nil,
            .deliverImmediately
        )
    }
}

private final class ObserverContext: @unchecked Sendable {
    let bridge: DarwinNotificationBridge
    let callback: @Sendable (AppLanguage) -> Void
    
    init(bridge: DarwinNotificationBridge, callback: @escaping @Sendable (AppLanguage) -> Void) {
        self.bridge = bridge
        self.callback = callback
    }
}
