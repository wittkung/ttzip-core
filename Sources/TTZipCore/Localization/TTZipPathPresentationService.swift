// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Canonical Single Source of Truth (SSOT) for path presentation, localized display names,
/// breadcrumb segment generation, and standard directory icon resolution across TTZip.
public final class TTZipPathPresentationService: Sendable {
    public static let shared = TTZipPathPresentationService()

    /// Resolves the actual physical user home directory outside the App Sandbox container.
    public static var realUserHomeURL: URL {
        if let pw = getpwuid(getuid()), let homeDir = pw.pointee.pw_dir {
            let path = String(cString: homeDir)
            return URL(fileURLWithPath: path).standardizedFileURL.resolvingSymlinksInPath()
        }
        let rawHome = NSHomeDirectory()
        if let range = rawHome.range(of: "/Library/Containers/") {
            let base = String(rawHome[..<range.lowerBound])
            return URL(fileURLWithPath: base).standardizedFileURL.resolvingSymlinksInPath()
        }
        return URL(fileURLWithPath: rawHome).standardizedFileURL.resolvingSymlinksInPath()
    }

    public var realUserHomeURL: URL {
        Self.realUserHomeURL
    }

    private let homeStandardized: String
    public let homeURL: URL
    public let username: String

    public init() {
        let realHome = Self.realUserHomeURL
        let stdHome = realHome.path
        self.homeStandardized = stdHome
        self.homeURL = realHome
        self.username = NSUserName()
    }

    // MARK: - Standard Directory Classification

    public enum StandardDirectory: Sendable, Hashable {
        case home
        case desktop
        case documents
        case downloads
        case pictures
        case movies
        case music
        case applications
        case utilities
        case library
        case publicFolder
        case trash
        case icloudDrive
        case root
    }

    private static let homeRelativeDirectories: [(subpath: String, directory: StandardDirectory)] = [
        ("Documents", .documents),
        ("Downloads", .downloads),
        ("Desktop", .desktop),
        ("Pictures", .pictures),
        ("Movies", .movies),
        ("Music", .music),
        ("Library", .library),
        ("Public", .publicFolder),
        (".Trash", .trash),
        ("Library/Mobile Documents/com~apple~CloudDocs", .icloudDrive),
        ("Applications", .applications)
    ]

    private static let systemDirectories: [String: StandardDirectory] = [
        "/": .root,
        "/Applications": .applications,
        "/System/Applications": .applications,
        "/Applications/Utilities": .utilities,
        "/System/Applications/Utilities": .utilities
    ]

    private static let homeSuffixFallbacks: [(suffix: String, directory: StandardDirectory)] = [
        ("/downloads", .downloads),
        ("/documents", .documents),
        ("/desktop", .desktop),
        ("/pictures", .pictures),
        ("/movies", .movies),
        ("/music", .music)
    ]

    /// Normalizes path by expanding tilde and standardizing components without trailing slashes.
    public func normalizePath(_ path: String) -> String {
        var p = (path as NSString).expandingTildeInPath
        p = (p as NSString).standardizingPath
        if p.count > 1 && p.hasSuffix("/") {
            p.removeLast()
        }
        return p
    }

    /// Classifies whether a given path corresponds to a standard macOS user or system directory.
    public func standardDirectory(for path: String) -> StandardDirectory? {
        let p = normalizePath(path)
        let home = homeStandardized

        if p == home {
            return .home
        }
        if let systemDir = Self.systemDirectories[p] {
            return systemDir
        }
        for entry in Self.homeRelativeDirectories {
            if p == (home as NSString).appendingPathComponent(entry.subpath) {
                return entry.directory
            }
        }
        if p.hasPrefix(home) {
            let lower = p.lowercased()
            for fallback in Self.homeSuffixFallbacks where lower.hasSuffix(fallback.suffix) {
                return fallback.directory
            }
        }
        return nil
    }

    private static let directoryTranslations: [StandardDirectory: [AppLanguage: String]] = [
        .documents: [
            .zhHans: "文稿", .zhHant: "文件", .ja: "書類",
            .de: "Dokumente", .fr: "Documents", .es: "Documentos", .en: "Documents"
        ],
        .downloads: [
            .zhHans: "下载", .zhHant: "下載", .ja: "ダウンロード",
            .de: "Downloads", .fr: "Téléchargements", .es: "Descargas", .en: "Downloads"
        ],
        .desktop: [
            .zhHans: "桌面", .zhHant: "桌面", .ja: "デスクトップ",
            .de: "Schreibtisch", .fr: "Bureau", .es: "Escritorio", .en: "Desktop"
        ],
        .pictures: [
            .zhHans: "图片", .zhHant: "圖片", .ja: "ピクチャ",
            .de: "Bilder", .fr: "Images", .es: "Imágenes", .en: "Pictures"
        ],
        .movies: [
            .zhHans: "影片", .zhHant: "影片", .ja: "ムービー",
            .de: "Filme", .fr: "Vidéos", .es: "Películas", .en: "Movies"
        ],
        .music: [
            .zhHans: "音乐", .zhHant: "音樂", .ja: "ミュージック",
            .de: "Musik", .fr: "Musique", .es: "Música", .en: "Music"
        ],
        .applications: [
            .zhHans: "应用程序", .zhHant: "應用程式", .ja: "アプリケーション",
            .de: "Programme", .fr: "Applications", .es: "Aplicaciones", .en: "Applications"
        ],
        .utilities: [
            .zhHans: "实用工具", .zhHant: "工具程式", .ja: "ユーティリティ",
            .de: "Dienstprogramme", .fr: "Utilitaires", .es: "Utilidades", .en: "Utilities"
        ],
        .library: [
            .zhHans: "资源库", .zhHant: "資源庫", .ja: "ライブラリ",
            .de: "Library", .fr: "Bibliothèque", .es: "Biblioteca", .en: "Library"
        ],
        .publicFolder: [
            .zhHans: "公共", .zhHant: "共享", .ja: "パブリック",
            .de: "Öffentlich", .fr: "Public", .es: "Público", .en: "Public"
        ],
        .trash: [
            .zhHans: "废纸篓", .zhHant: "垃圾桶", .ja: "ゴミ箱",
            .de: "Papierkorb", .fr: "Corbeille", .es: "Papelera", .en: "Trash"
        ],
        .icloudDrive: [
            .zhHans: "iCloud 云盘", .zhHant: "iCloud 雲碟", .ja: "iCloud Drive",
            .de: "iCloud Drive", .fr: "iCloud Drive", .es: "iCloud Drive", .en: "iCloud Drive"
        ]
    ]

    /// Resolves canonical localized name for standard directory in target language.
    public func localizedName(for directory: StandardDirectory, language: AppLanguage) -> String {
        switch directory {
        case .home:
            return username
        case .root:
            return "Macintosh HD"
        default:
            return Self.directoryTranslations[directory]?[language] ?? "Folder"
        }
    }

    /// Resolves canonical localized name for standard directory path if matched.
    public func standardDirectoryLocalizedName(for canonicalPath: String, language: AppLanguage? = nil) -> String? {
        guard let dir = standardDirectory(for: canonicalPath) else { return nil }
        let targetLanguage = language ?? TTZipLocalizationManager.shared.currentLanguage
        return localizedName(for: dir, language: targetLanguage)
    }

    // MARK: - Display Name Resolution (SSOT)

    /// Resolves localized display name for any path or URL string with comprehensive fallback.
    public func displayName(for path: String, fallback: String? = nil, language: AppLanguage? = nil) -> String {
        if let virtualName = resolveVirtualDisplayName(for: path, fallback: fallback, language: language) {
            return virtualName
        }

        let normalized = normalizePath(path)
        let targetLanguage = language ?? TTZipLocalizationManager.shared.currentLanguage

        if let dir = standardDirectory(for: normalized) {
            return resolveStandardDirectoryDisplayName(dir, language: targetLanguage)
        }
        if let systemName = Self.resolveSystemDisplayName(for: normalized) {
            return systemName
        }
        if let fallback, !fallback.isEmpty {
            return fallback
        }
        let last = (normalized as NSString).lastPathComponent
        return last.isEmpty ? "/" : last
    }

    private func resolveVirtualDisplayName(
        for path: String,
        fallback: String?,
        language: AppLanguage?
    ) -> String? {
        guard path.hasPrefix("ttzip://") || path.contains("?subpath=") else { return nil }
        let (archivePath, subpath) = Self.parseVirtualURL(path)
        if !subpath.isEmpty {
            return (subpath as NSString).lastPathComponent
        }
        if !archivePath.isEmpty && archivePath != path {
            return displayName(for: archivePath, fallback: fallback, language: language)
        }
        return nil
    }

    private func resolveStandardDirectoryDisplayName(
        _ directory: StandardDirectory,
        language: AppLanguage
    ) -> String {
        if directory == .root {
            let rootURL = URL(fileURLWithPath: "/")
            if let volName = (try? rootURL.resourceValues(forKeys: [.volumeLocalizedNameKey]))?.volumeLocalizedName,
               !volName.isEmpty, volName != "/" {
                return volName
            }
            return "Macintosh HD"
        }
        return localizedName(for: directory, language: language)
    }

    private static func resolveSystemDisplayName(for normalized: String) -> String? {
        let fileURL = URL(fileURLWithPath: normalized)
        let lastComp = fileURL.lastPathComponent
        if let resName = (try? fileURL.resourceValues(forKeys: [.localizedNameKey]))?.localizedName,
           !resName.isEmpty, resName != normalized, resName != lastComp {
            return resName
        }
        let fmName = FileManager.default.displayName(atPath: normalized)
        if !fmName.isEmpty && fmName != normalized && fmName != lastComp {
            return fmName
        }
        return nil
    }

    /// Convenience overload resolving display name for a file URL.
    public func displayName(for url: URL, language: AppLanguage? = nil) -> String {
        let abs = url.absoluteString
        if abs.hasPrefix("ttzip://") || abs.contains("?subpath=") {
            return displayName(for: abs, fallback: url.lastPathComponent, language: language)
        }
        return displayName(for: url.path, fallback: url.lastPathComponent, language: language)
    }

    // MARK: - Path Abbreviation (~ / Folders)

    /// Abbreviates physical path using tilde for home directory (matching NSString.abbreviatingWithTildeInPath).
    public func abbreviateWithTilde(_ path: String) -> String {
        abbreviatedPath(for: path, abbreviateHome: true, localizeComponents: false)
    }

    /// Abbreviates physical path using tilde for home directory with localized directory names.
    public func abbreviateWithTildeLocalized(_ path: String, language: AppLanguage? = nil) -> String {
        abbreviatedPath(for: path, abbreviateHome: true, localizeComponents: true, language: language)
    }

    /// Abbreviates physical path using tilde for home directory, with optional component localization.
    public func abbreviatedPath(
        for path: String,
        abbreviateHome: Bool = true,
        localizeComponents: Bool = true,
        language: AppLanguage? = nil
    ) -> String {
        let normalized = normalizePath(path)
        guard !normalized.isEmpty, normalized != "/" else { return "/" }

        let targetLanguage = language ?? TTZipLocalizationManager.shared.currentLanguage

        if abbreviateHome && (normalized == homeStandardized || normalized.hasPrefix(homeStandardized + "/")) {
            if normalized == homeStandardized {
                return "~"
            }
            let relative = String(normalized.dropFirst(homeStandardized.count + 1))
            let parts = relative.split(separator: "/").map(String.init)
            guard !parts.isEmpty else { return "~" }
            let rendered = formatPathComponents(
                parts,
                basePath: homeStandardized,
                localizeComponents: localizeComponents,
                language: targetLanguage
            )
            return "~/" + rendered.joined(separator: "/")
        }

        if localizeComponents {
            let parts = normalized.split(separator: "/").map(String.init)
            let rendered = formatPathComponents(
                parts,
                basePath: "",
                localizeComponents: true,
                language: targetLanguage
            )
            return "/" + rendered.joined(separator: "/")
        }

        return normalized
    }

    private func formatPathComponents(
        _ parts: [String],
        basePath: String,
        localizeComponents: Bool,
        language: AppLanguage
    ) -> [String] {
        guard localizeComponents else { return parts }
        var resultParts: [String] = []
        var currentPath = basePath
        for part in parts {
            currentPath += "/" + part
            resultParts.append(displayName(for: currentPath, fallback: part, language: language))
        }
        return resultParts
    }

    // MARK: - Breadcrumb Segments Generator

    /// Builds structured breadcrumb presentation segments for an URL.
    public func breadcrumbSegments(
        for url: URL,
        abbreviateHome: Bool = true,
        language: AppLanguage? = nil
    ) -> [TTZipPathSegment] {
        let targetLanguage = language ?? TTZipLocalizationManager.shared.currentLanguage
        let path = normalizePath(url.standardizedFileURL.path)

        if path == "/" {
            return [makeRootSegment(language: targetLanguage, isCurrent: true)]
        }

        if abbreviateHome && (path == homeStandardized || path.hasPrefix(homeStandardized + "/")) {
            let isAtHome = (path == homeStandardized)
            var segments = [
                TTZipPathSegment(
                    id: homeStandardized,
                    displayName: "~",
                    url: homeURL,
                    iconName: "house.fill",
                    isCurrent: isAtHome
                )
            ]
            if !isAtHome {
                let relative = String(path.dropFirst(homeStandardized.count + 1))
                let parts = relative.split(separator: "/").map(String.init)
                appendRelativeSegments(parts: parts, baseURL: homeURL, language: targetLanguage, into: &segments)
            }
            return segments
        }

        var segments = [makeRootSegment(language: targetLanguage, isCurrent: false)]
        let parts = path.split(separator: "/").map(String.init)
        appendRelativeSegments(
            parts: parts,
            baseURL: URL(fileURLWithPath: "/"),
            language: targetLanguage,
            into: &segments
        )
        return segments
    }

    private func makeRootSegment(language: AppLanguage, isCurrent: Bool) -> TTZipPathSegment {
        TTZipPathSegment(
            id: "/",
            displayName: displayName(for: "/", language: language),
            url: URL(fileURLWithPath: "/"),
            iconName: "internaldrive.fill",
            isCurrent: isCurrent
        )
    }

    private func appendRelativeSegments(
        parts: [String],
        baseURL: URL,
        language: AppLanguage,
        into segments: inout [TTZipPathSegment]
    ) {
        var cumulativeURL = baseURL
        for (idx, part) in parts.enumerated() {
            cumulativeURL = cumulativeURL.appendingPathComponent(part)
            let isLast = (idx == parts.count - 1)
            let name = displayName(for: cumulativeURL.path, fallback: part, language: language)
            let iconName = icon(for: cumulativeURL.path, isDirectory: true)
            segments.append(
                TTZipPathSegment(
                    id: cumulativeURL.path,
                    displayName: name,
                    url: cumulativeURL,
                    iconName: iconName,
                    isCurrent: isLast
                )
            )
        }
    }

    // MARK: - Canonical SF Symbol Icon Resolution

    private static let standardDirectoryIcons: [StandardDirectory: String] = [
        .home: "house.fill",
        .root: "internaldrive.fill",
        .documents: "doc.text.fill",
        .downloads: "arrow.down.circle",
        .desktop: "desktopcomputer",
        .pictures: "photo.fill",
        .movies: "film.fill",
        .music: "music.note",
        .applications: "app.badge",
        .utilities: "wrench.and.screwdriver.fill",
        .library: "books.vertical.fill",
        .publicFolder: "person.2.fill",
        .trash: "trash.fill",
        .icloudDrive: "cloud.fill"
    ]

    private static let cloudPathKeywords = ["cloud", "drive", "dropbox", "nextcloud"]
    private static let codeComponentKeywords = ["dev", "code", "git", "project"]
    private static let noteComponentKeywords = ["blog", "note", "writing"]

    /// Resolves canonical SF Symbol icon name for a path.
    public func icon(for path: String, isDirectory: Bool = true) -> String {
        let normalized = normalizePath(path)
        if let std = standardDirectory(for: normalized),
           let stdIcon = Self.standardDirectoryIcons[std] {
            return stdIcon
        }

        let lower = normalized.lowercased()
        if Self.cloudPathKeywords.contains(where: { lower.contains($0) }) {
            return "cloud.fill"
        }
        if lower.contains("/volumes/") {
            return "internaldrive.fill"
        }

        let lastComp = (normalized as NSString).lastPathComponent.lowercased()
        if Self.codeComponentKeywords.contains(where: { lastComp.contains($0) }) {
            return "chevron.left.forwardslash.chevron.right"
        }
        if Self.noteComponentKeywords.contains(where: { lastComp.contains($0) }) {
            return "text.book.closed.fill"
        }
        return isDirectory ? "folder.fill" : "doc"
    }

    // MARK: - Virtual URL Parser

    /// Parses virtual archive URLs into archive disk path and archive-internal subpath.
    public static func parseVirtualURL(_ path: String) -> (archivePath: String, subpath: String) {
        if path.hasPrefix("ttzip://") {
            let stripped = String(path.dropFirst("ttzip://".count))
            let parts = stripped.components(separatedBy: "?subpath=")
            let archive = parts.first ?? ""
            let sub = parts.count > 1 ? parts[1] : ""
            return (archive, sub)
        }
        if let queryRange = path.range(of: "?subpath=") {
            let archive = String(path[..<queryRange.lowerBound])
            let sub = String(path[queryRange.upperBound...])
            return (archive, sub)
        }
        return (path, "")
    }
}

/// Immutable presentation segment for hierarchical path rendering.
public struct TTZipPathSegment: Identifiable, Hashable, Sendable {
    public let id: String
    public let displayName: String
    public var name: String { displayName }
    public let url: URL
    public let iconName: String
    public let isCurrent: Bool

    public init(id: String, displayName: String, url: URL, iconName: String, isCurrent: Bool) {
        self.id = id
        self.displayName = displayName
        self.url = url
        self.iconName = iconName
        self.isCurrent = isCurrent
    }
}
