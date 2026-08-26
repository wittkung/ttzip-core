// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTKit: High-performance cross-platform system utility foundation.

#if canImport(AppKit)
import AppKit
import TTLocalizationCore

/// 3-Tier topological AppKit main menu synchronizer.
@MainActor
public final class AppKitMenuSynchronizer {
    public static let shared = AppKitMenuSynchronizer()
    
    private var tagMap: [Int: any LocaleKeyProtocol] = [:]
    private var selectorMap: [Selector: any LocaleKeyProtocol] = [:]
    
    private init() {}
    
    /// Register a tag-to-locale-key mapping.
    public func registerTag(_ tag: Int, key: any LocaleKeyProtocol) {
        tagMap[tag] = key
    }
    
    /// Register a selector-to-locale-key mapping.
    public func registerSelector(_ selector: Selector, key: any LocaleKeyProtocol) {
        selectorMap[selector] = key
    }
    
    /// Synchronize all menu items recursively across NSApplication.shared.mainMenu.
    public func synchronize(language: AppLanguage) {
        guard let mainMenu = NSApplication.shared.mainMenu else { return }
        for item in mainMenu.items {
            synchronizeItemRecursively(item, language: language)
        }
        mainMenu.update()
    }
    
    private func synchronizeItemRecursively(_ item: NSMenuItem, language: AppLanguage) {
        let manager = TTLocalizationManager.shared
        
        // Tier 1: Permanent Tag
        if item.tag != 0, let key = tagMap[item.tag] {
            item.title = manager.string(for: key, language: language)
        } else if let action = item.action, let key = selectorMap[action] {
            // Tier 2: Action Selector
            item.title = manager.string(for: key, language: language)
        }
        
        if let submenu = item.submenu {
            submenu.title = item.title
            for subItem in submenu.items {
                synchronizeItemRecursively(subItem, language: language)
            }
            submenu.update()
        }
    }
}
#endif
