// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Canonical Single Source of Truth (SSOT) for path presentation, localized display names,
/// breadcrumb segment generation, and standard directory icon resolution across TTZip.
public final class TTZipPathPresentationService: @unchecked Sendable {
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
        if p == "/" {
            return .root
        }

        // Relative to Home
        if p == (home as NSString).appendingPathComponent("Documents") {
            return .documents
        }
        if p == (home as NSString).appendingPathComponent("Downloads") {
            return .downloads
        }
        if p == (home as NSString).appendingPathComponent("Desktop") {
            return .desktop
        }
        if p == (home as NSString).appendingPathComponent("Pictures") {
            return .pictures
        }
        if p == (home as NSString).appendingPathComponent("Movies") {
            return .movies
        }
        if p == (home as NSString).appendingPathComponent("Music") {
            return .music
        }
        if p == (home as NSString).appendingPathComponent("Library") {
            return .library
        }
        if p == (home as NSString).appendingPathComponent("Public") {
            return .publicFolder
        }
        if p == (home as NSString).appendingPathComponent(".Trash") {
            return .trash
        }
        let icloudPath = (home as NSString).appendingPathComponent("Library/Mobile Documents/com~apple~CloudDocs")
        if p == icloudPath || p.hasPrefix(icloudPath + "/") {
            if p == icloudPath { return .icloudDrive }
        }

        // System Applications & Utilities
        if p == "/Applications" || p == "/System/Applications" || p == (home as NSString).appendingPathComponent("Applications") {
            return .applications
        }
        if p == "/Applications/Utilities" || p == "/System/Applications/Utilities" {
            return .utilities
        }

        // Suffix fallbacks for localized directory paths or custom volumes rooted in user folders
        let lower = p.lowercased()
        if p.hasPrefix(home) {
            if lower.hasSuffix("/downloads") { return .downloads }
            if lower.hasSuffix("/documents") { return .documents }
            if lower.hasSuffix("/desktop") { return .desktop }
            if lower.hasSuffix("/pictures") { return .pictures }
            if lower.hasSuffix("/movies") { return .movies }
            if lower.hasSuffix("/music") { return .music }
        }

        return nil
    }

    /// Resolves canonical localized name for standard directory in target language.
    public func localizedName(for directory: StandardDirectory, language: AppLanguage) -> String {
        switch directory {
        case .home:
            return username
        case .root:
            return "Macintosh HD"
        case .documents:
            switch language {
            case .zhHans: return "文稿"
            case .zhHant: return "文件"
            case .ja: return "書類"
            case .de: return "Dokumente"
            case .fr: return "Documents"
            case .es: return "Documentos"
            case .en: return "Documents"
            }
        case .downloads:
            switch language {
            case .zhHans: return "下载"
            case .zhHant: return "下載"
            case .ja: return "ダウンロード"
            case .de: return "Downloads"
            case .fr: return "Téléchargements"
            case .es: return "Descargas"
            case .en: return "Downloads"
            }
        case .desktop:
            switch language {
            case .zhHans, .zhHant: return "桌面"
            case .ja: return "デスクトップ"
            case .de: return "Schreibtisch"
            case .fr: return "Bureau"
            case .es: return "Escritorio"
            case .en: return "Desktop"
            }
        case .pictures:
            switch language {
            case .zhHans: return "图片"
            case .zhHant: return "圖片"
            case .ja: return "ピクチャ"
            case .de: return "Bilder"
            case .fr: return "Images"
            case .es: return "Imágenes"
            case .en: return "Pictures"
            }
        case .movies:
            switch language {
            case .zhHans, .zhHant: return "影片"
            case .ja: return "ムービー"
            case .de: return "Filme"
            case .fr: return "Vidéos"
            case .es: return "Películas"
            case .en: return "Movies"
            }
        case .music:
            switch language {
            case .zhHans: return "音乐"
            case .zhHant: return "音樂"
            case .ja: return "ミュージック"
            case .de: return "Musik"
            case .fr: return "Musique"
            case .es: return "Música"
            case .en: return "Music"
            }
        case .applications:
            switch language {
            case .zhHans: return "应用程序"
            case .zhHant: return "應用程式"
            case .ja: return "アプリケーション"
            case .de: return "Programme"
            case .fr: return "Applications"
            case .es: return "Aplicaciones"
            case .en: return "Applications"
            }
        case .utilities:
            switch language {
            case .zhHans: return "实用工具"
            case .zhHant: return "工具程式"
            case .ja: return "ユーティリティ"
            case .de: return "Dienstprogramme"
            case .fr: return "Utilitaires"
            case .es: return "Utilidades"
            case .en: return "Utilities"
            }
        case .library:
            switch language {
            case .zhHans: return "资源库"
            case .zhHant: return "資源庫"
            case .ja: return "ライブラリ"
            case .de: return "Library"
            case .fr: return "Bibliothèque"
            case .es: return "Biblioteca"
            case .en: return "Library"
            }
        case .publicFolder:
            switch language {
            case .zhHans: return "公共"
            case .zhHant: return "共享"
            case .ja: return "パブリック"
            case .de: return "Öffentlich"
            case .fr: return "Public"
            case .es: return "Público"
            case .en: return "Public"
            }
        case .trash:
            switch language {
            case .zhHans: return "废纸篓"
            case .zhHant: return "垃圾桶"
            case .ja: return "ゴミ箱"
            case .de: return "Papierkorb"
            case .fr: return "Corbeille"
            case .es: return "Papelera"
            case .en: return "Trash"
            }
        case .icloudDrive:
            switch language {
            case .zhHans: return "iCloud 云盘"
            case .zhHant: return "iCloud 雲碟"
            case .ja, .de, .fr, .es, .en: return "iCloud Drive"
            }
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
        // 1. Virtual Archive URL Support
        if path.hasPrefix("ttzip://") || path.contains("?subpath=") {
            let (archivePath, subpath) = Self.parseVirtualURL(path)
            if !subpath.isEmpty {
                return (subpath as NSString).lastPathComponent
            }
            if !archivePath.isEmpty && archivePath != path {
                return displayName(for: archivePath, fallback: fallback, language: language)
            }
        }

        let normalized = normalizePath(path)
        let targetLanguage = language ?? TTZipLocalizationManager.shared.currentLanguage

        // 2. Standard Directory Match
        if let dir = standardDirectory(for: normalized) {
            if dir == .root {
                let rootURL = URL(fileURLWithPath: "/")
                if let volName = (try? rootURL.resourceValues(forKeys: [.volumeLocalizedNameKey]))?.volumeLocalizedName,
                   !volName.isEmpty, volName != "/" {
                    return volName
                }
                return "Macintosh HD"
            }
            return localizedName(for: dir, language: targetLanguage)
        }

        // 3. System Resource Values (.localized bundle attribute)
        let fileURL = URL(fileURLWithPath: normalized)
        if let resName = (try? fileURL.resourceValues(forKeys: [.localizedNameKey]))?.localizedName,
           !resName.isEmpty, resName != normalized, resName != fileURL.lastPathComponent {
            return resName
        }

        // 4. FileManager displayName
        let fmName = FileManager.default.displayName(atPath: normalized)
        if !fmName.isEmpty && fmName != normalized && fmName != fileURL.lastPathComponent {
            return fmName
        }

        // 5. Fallback or lastPathComponent
        if let fallback, !fallback.isEmpty {
            return fallback
        }
        let last = (normalized as NSString).lastPathComponent
        return last.isEmpty ? "/" : last
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

            var resultParts: [String] = []
            var currentPath = homeStandardized
            for part in parts {
                currentPath += "/" + part
                if localizeComponents {
                    resultParts.append(displayName(for: currentPath, fallback: part, language: targetLanguage))
                } else {
                    resultParts.append(part)
                }
            }
            return "~/" + resultParts.joined(separator: "/")
        }

        if localizeComponents {
            let parts = normalized.split(separator: "/").map(String.init)
            var resultParts: [String] = []
            var currentPath = ""
            for part in parts {
                currentPath += "/" + part
                resultParts.append(displayName(for: currentPath, fallback: part, language: targetLanguage))
            }
            return "/" + resultParts.joined(separator: "/")
        }

        return normalized
    }

    // MARK: - Breadcrumb Segments Generator

    /// Builds structured breadcrumb presentation segments for an URL.
    public func breadcrumbSegments(
        for url: URL,
        abbreviateHome: Bool = true,
        language: AppLanguage? = nil
    ) -> [TTZipPathSegment] {
        let targetLanguage = language ?? TTZipLocalizationManager.shared.currentLanguage
        let standardizedURL = url.standardizedFileURL
        let path = normalizePath(standardizedURL.path)

        // 1. Root Directory
        if path == "/" {
            return [
                TTZipPathSegment(
                    id: "/",
                    displayName: displayName(for: "/", language: targetLanguage),
                    url: URL(fileURLWithPath: "/"),
                    iconName: "internaldrive.fill",
                    isCurrent: true
                )
            ]
        }

        var segments: [TTZipPathSegment] = []

        // 2. User Home Relative
        if abbreviateHome && (path == homeStandardized || path.hasPrefix(homeStandardized + "/")) {
            let isAtHome = (path == homeStandardized)
            segments.append(
                TTZipPathSegment(
                    id: homeStandardized,
                    displayName: "~",
                    url: homeURL,
                    iconName: "house.fill",
                    isCurrent: isAtHome
                )
            )

            if !isAtHome {
                let relative = String(path.dropFirst(homeStandardized.count + 1))
                let parts = relative.split(separator: "/").map(String.init)
                var cumulativeURL = homeURL

                for (idx, part) in parts.enumerated() {
                    cumulativeURL = cumulativeURL.appendingPathComponent(part)
                    let isLast = (idx == parts.count - 1)
                    let name = displayName(for: cumulativeURL.path, fallback: part, language: targetLanguage)
                    let icon = icon(for: cumulativeURL.path, isDirectory: true)

                    segments.append(
                        TTZipPathSegment(
                            id: cumulativeURL.path,
                            displayName: name,
                            url: cumulativeURL,
                            iconName: icon,
                            isCurrent: isLast
                        )
                    )
                }
            }
            return segments
        }

        // 3. Absolute Root Relative
        segments.append(
            TTZipPathSegment(
                id: "/",
                displayName: displayName(for: "/", language: targetLanguage),
                url: URL(fileURLWithPath: "/"),
                iconName: "internaldrive.fill",
                isCurrent: false
            )
        )

        let parts = path.split(separator: "/").map(String.init)
        var cumulativeURL = URL(fileURLWithPath: "/")
        for (idx, part) in parts.enumerated() {
            cumulativeURL = cumulativeURL.appendingPathComponent(part)
            let isLast = (idx == parts.count - 1)
            let name = displayName(for: cumulativeURL.path, fallback: part, language: targetLanguage)
            let icon = icon(for: cumulativeURL.path, isDirectory: true)

            segments.append(
                TTZipPathSegment(
                    id: cumulativeURL.path,
                    displayName: name,
                    url: cumulativeURL,
                    iconName: icon,
                    isCurrent: isLast
                )
            )
        }

        return segments
    }

    // MARK: - Canonical SF Symbol Icon Resolution

    /// Resolves canonical SF Symbol icon name for a path.
    public func icon(for path: String, isDirectory: Bool = true) -> String {
        let normalized = normalizePath(path)
        if let std = standardDirectory(for: normalized) {
            switch std {
            case .home: return "house.fill"
            case .root: return "internaldrive.fill"
            case .documents: return "doc.text.fill"
            case .downloads: return "arrow.down.circle"
            case .desktop: return "desktopcomputer"
            case .pictures: return "photo.fill"
            case .movies: return "film.fill"
            case .music: return "music.note"
            case .applications: return "app.badge"
            case .utilities: return "wrench.and.screwdriver.fill"
            case .library: return "books.vertical.fill"
            case .publicFolder: return "person.2.fill"
            case .trash: return "trash.fill"
            case .icloudDrive: return "cloud.fill"
            }
        }

        let lower = normalized.lowercased()
        let lastComp = (normalized as NSString).lastPathComponent.lowercased()

        if lower.contains("cloud") || lower.contains("drive") || lower.contains("dropbox") || lower.contains("nextcloud") {
            return "cloud.fill"
        }
        if lower.contains("/volumes/") {
            return "internaldrive.fill"
        }
        if lastComp.contains("dev") || lastComp.contains("code") || lastComp.contains("git") || lastComp.contains("project") {
            return "chevron.left.forwardslash.chevron.right"
        }
        if lastComp.contains("blog") || lastComp.contains("note") || lastComp.contains("writing") {
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
