// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation

/// macOS Finder context menu action item matching industrial standards.
public struct FinderContextMenuItem: Sendable, Equatable {
    public let title: String
    public let actionIdentifier: String
    public let iconSystemName: String
    public let isDestructive: Bool
    
    public init(
        title: String,
        actionIdentifier: String,
        iconSystemName: String = "archivebox",
        isDestructive: Bool = false
    ) {
        self.title = title
        self.actionIdentifier = actionIdentifier
        self.iconSystemName = iconSystemName
        self.isDestructive = isDestructive
    }
}

/// Helper facilitating FinderSync context menu construction across all 16 supported archive formats.
public final class FinderSyncHelper: @unchecked Sendable {
    public static let shared = FinderSyncHelper()
    
    private init() {}
    
    public typealias ContextMenuItem = FinderContextMenuItem
    
    /// Supported archive extensions recognized by FinderSync context menu dispatcher.
    public static let supportedArchiveExtensions: Set<String> = [
        "zip", "zipx", "cbz", "7z", "cb7", "tar", "gz", "tgz", "bz2", "tbz2", "tbz",
        "xz", "txz", "zst", "tzst", "lz4", "br", "lz", "lzip", "lrz", "lrzip",
        "aar", "applearchive", "sz", "snappy", "wim", "dmg", "iso", "rar", "cbr", "cab", "001"
    ]
    
    /// Returns dynamic context menu items based on selected file URLs and target language.
    public func getContextMenuItems(selectedURLs: [URL], language: AppLanguage? = nil) -> [FinderContextMenuItem] {
        guard !selectedURLs.isEmpty else { return [] }
        
        let firstURL = selectedURLs[0]
        let baseName = selectedURLs.count == 1 ? firstURL.deletingPathExtension().lastPathComponent : "ArchiveBundle"
        let ext = firstURL.pathExtension.lowercased()
        let isArchive = Self.supportedArchiveExtensions.contains(ext)
        let manager = TTZipLocalizationManager.shared
        let targetLang = language ?? manager.currentLanguage
        
        if isArchive {
            return [
                FinderContextMenuItem(
                    title: "\(manager.string(for: L10n.FinderSync.extractHereTitle, language: targetLang)) (\(baseName))",
                    actionIdentifier: "extract_here",
                    iconSystemName: "arrow.down.doc"
                ),
                FinderContextMenuItem(
                    title: manager.string(for: L10n.FinderSync.extractSubfolderTitle, language: targetLang),
                    actionIdentifier: "extract_to_subfolder",
                    iconSystemName: "folder.badge.plus"
                ),
                FinderContextMenuItem(
                    title: manager.string(for: L10n.FinderSync.inspectTitle, language: targetLang),
                    actionIdentifier: "inspect_archive",
                    iconSystemName: "eye"
                ),
                FinderContextMenuItem(
                    title: manager.string(for: L10n.FinderSync.autofillTitle, language: targetLang),
                    actionIdentifier: "autofill_password",
                    iconSystemName: "key"
                ),
                FinderContextMenuItem(
                    title: manager.string(for: L10n.FinderSync.computeHashTitle, language: targetLang),
                    actionIdentifier: "compute_hash",
                    iconSystemName: "checkmark.shield"
                )
            ]
        } else {
            return [
                FinderContextMenuItem(
                    title: "\(manager.string(for: L10n.FinderSync.compress7zTitle, language: targetLang)) (\"\(baseName).7z\")",
                    actionIdentifier: "compress_quick_7z",
                    iconSystemName: "sparkles"
                ),
                FinderContextMenuItem(
                    title: "\(manager.string(for: L10n.FinderSync.compressZipTitle, language: targetLang)) (\"\(baseName).zip\")",
                    actionIdentifier: "compress_quick_zip",
                    iconSystemName: "archivebox"
                ),
                FinderContextMenuItem(
                    title: manager.string(for: L10n.FinderSync.compressSeparateTitle, language: targetLang),
                    actionIdentifier: "compress_separate",
                    iconSystemName: "doc.on.doc"
                ),
                FinderContextMenuItem(
                    title: manager.string(for: L10n.FinderSync.compressDeleteSourceTitle, language: targetLang),
                    actionIdentifier: "compress_and_delete_source",
                    iconSystemName: "trash",
                    isDestructive: true
                ),
                FinderContextMenuItem(
                    title: manager.string(for: L10n.FinderSync.compressAdvancedTitle, language: targetLang),
                    actionIdentifier: "compress_modal_advanced",
                    iconSystemName: "slider.horizontal.3"
                )
            ]
        }
    }
}

/// Action type identifier dispatched from macOS Finder context menu or Services.
public enum FinderSyncActionIdentifier: String, Codable, Sendable, CaseIterable {
    case extractHere = "extract_here"
    case extractToSubfolder = "extract_to_subfolder"
    case inspectArchive = "inspect_archive"
    case compressQuick7z = "compress_quick_7z"
    case compressQuickZip = "compress_quick_zip"
    case compressSeparate = "compress_separate"
    case compressAndDeleteSource = "compress_and_delete_source"
    case compressModalAdvanced = "compress_modal_advanced"
    case autofillPassword = "autofill_password"
    case computeHash = "compute_hash"
}

/// Request model representing an IPC action request dispatched from FinderSync context menus or Services.
public struct FinderSyncActionRequest: Codable, Sendable, Equatable {
    public let actionIdentifier: String
    public let sourcePaths: [String]
    public let destinationDirectory: String?
    public let sanitizeMacMetadata: Bool
    public let password: String?
    
    public var typedAction: FinderSyncActionIdentifier? {
        FinderSyncActionIdentifier(rawValue: actionIdentifier)
    }
    
    public init(
        actionIdentifier: String,
        sourcePaths: [String],
        destinationDirectory: String? = nil,
        sanitizeMacMetadata: Bool = false,
        password: String? = nil
    ) {
        self.actionIdentifier = actionIdentifier
        self.sourcePaths = sourcePaths
        self.destinationDirectory = destinationDirectory
        self.sanitizeMacMetadata = sanitizeMacMetadata
        self.password = password
    }
    
    public init(
        action: FinderSyncActionIdentifier,
        sourcePaths: [String],
        destinationDirectory: String? = nil,
        sanitizeMacMetadata: Bool = false,
        password: String? = nil
    ) {
        self.actionIdentifier = action.rawValue
        self.sourcePaths = sourcePaths
        self.destinationDirectory = destinationDirectory
        self.sanitizeMacMetadata = sanitizeMacMetadata
        self.password = password
    }
}
