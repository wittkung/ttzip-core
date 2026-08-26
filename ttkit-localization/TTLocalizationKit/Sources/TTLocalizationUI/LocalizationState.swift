// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTKit: High-performance cross-platform system utility foundation.

import Foundation
import SwiftUI
import Observation
import TTLocalizationCore
import TTLocalizationIPC

/// Swift 6 observable localization state machine.
@Observable
@MainActor
public final class LocalizationState {
    public static let shared = LocalizationState()
    
    public private(set) var currentLanguage: AppLanguage
    public var byteUnitStandard: ByteSizeStandard {
        didSet {
            DarwinNotificationBridge.shared.broadcastChange(language: currentLanguage, byteStandard: byteUnitStandard)
        }
    }
    
    private init() {
        if let stored = DarwinNotificationBridge.shared.getSavedLanguage() {
            self.currentLanguage = stored
        } else {
            self.currentLanguage = TTLocalizationManager.shared.currentLanguage
        }
        self.byteUnitStandard = DarwinNotificationBridge.shared.getSavedByteStandard()
        TTLocalizationManager.shared.currentLanguage = self.currentLanguage
    }
    
    /// Switches the application's active language dynamically in real time (< 1ms).
    public func setLanguage(_ language: AppLanguage) {
        guard language != currentLanguage else { return }
        withTransaction(Transaction(animation: nil)) {
            self.currentLanguage = language
        }
        TTLocalizationManager.shared.currentLanguage = language
        DarwinNotificationBridge.shared.broadcastChange(language: language, byteStandard: byteUnitStandard)
    }
    
    /// Resolves a localized string for the specified key in the current language.
    public func t(_ key: any LocaleKeyProtocol) -> String {
        return TTLocalizationManager.shared.string(for: key, language: currentLanguage)
    }
    
    /// Resolves a formatted localized string with positional arguments.
    public func format(_ key: any LocaleKeyProtocol, _ args: CVarArg...) -> String {
        let formatStr = TTLocalizationManager.shared.string(for: key, language: currentLanguage)
        return String(format: formatStr, locale: Locale(identifier: currentLanguage.bcp47), arguments: args)
    }
}

/// Reactive SwiftUI Text primitive that automatically updates on localization changes.
public struct L10nText: View {
    private var l10n = LocalizationState.shared
    private let key: any LocaleKeyProtocol
    private let args: [CVarArg]
    
    public init(_ key: any LocaleKeyProtocol, _ args: CVarArg...) {
        self.key = key
        self.args = args
    }
    
    public var body: some View {
        if args.isEmpty {
            Text(l10n.t(key))
        } else {
            let template = TTLocalizationManager.shared.string(for: key, language: l10n.currentLanguage)
            let formatted = String(format: template, locale: Locale(identifier: l10n.currentLanguage.bcp47), arguments: args)
            Text(formatted)
        }
    }
}

/// Reactive SwiftUI Label primitive that automatically updates on localization changes.
public struct L10nLabel: View {
    private var l10n = LocalizationState.shared
    private let key: any LocaleKeyProtocol
    private let systemImage: String
    private let args: [CVarArg]
    
    public init(_ key: any LocaleKeyProtocol, systemImage: String, _ args: CVarArg...) {
        self.key = key
        self.systemImage = systemImage
        self.args = args
    }
    
    public var body: some View {
        if args.isEmpty {
            Label(l10n.t(key), systemImage: systemImage)
        } else {
            let template = TTLocalizationManager.shared.string(for: key, language: l10n.currentLanguage)
            let formatted = String(format: template, locale: Locale(identifier: l10n.currentLanguage.bcp47), arguments: args)
            Label(formatted, systemImage: systemImage)
        }
    }
}

public extension View {
    /// Applies a reactive localized tooltip to the view.
    @ViewBuilder
    func l10nHelp(_ key: any LocaleKeyProtocol) -> some View {
        self.help(LocalizationState.shared.t(key))
    }
}
