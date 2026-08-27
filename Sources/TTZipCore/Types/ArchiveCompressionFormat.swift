// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Logical grouping category for archive formats.
public enum ArchiveFormatCategory: String, Sendable, CaseIterable, Identifiable, Codable {
    case standard = "Standard & Universal"
    case unixPackage = "Linux & Unix Packages"
    case diskImage = "Disk & System Images"
    case modernStream = "High-Throughput Streams"
    
    public var id: String { rawValue }
    
    public var iconName: String {
        switch self {
        case .standard: return "archivebox.fill"
        case .unixPackage: return "shippingbox.fill"
        case .diskImage: return "opticaldisc.fill"
        case .modernStream: return "bolt.fill"
        }
    }
}

/// Comprehensive enumeration of all archive, container, packaging, disc image, and stream formats.
public enum ArchiveCompressionFormat: String, Sendable, CaseIterable, Codable {
    // MARK: - Core Standard Formats
    case sevenZip = "7z"
    case zip = "zip"
    case tar = "tar"
    
    // MARK: - Composite Tarball Formats
    case tarGz = "tar.gz"
    case tarBz2 = "tar.bz2"
    case tarXz = "tar.xz"
    case tarZst = "tar.zst"
    case tarLz4 = "tar.lz4"
    case tarBrotli = "tar.br"
    case tarLzip = "tar.lz"
    case tarLrzip = "tar.lrz"
    
    // MARK: - Disk & Packaging Images
    case iso = "iso"
    case cab = "cab"
    case wim = "wim"
    case dmg = "dmg"
    case aar = "aar"
    case cpio = "cpio"
    case ar = "ar"
    case deb = "deb"
    case rpm = "rpm"
    case xar = "xar"
    
    // MARK: - Read-Only / System Formats
    case rar = "rar"
    case squashfs = "squashfs"
    case lzfse = "lzfse"
    case lzh = "lzh"
    
    // MARK: - Standalone Streams
    case zst = "zst"
    case gz = "gz"
    case bz2 = "bz2"
    case xz = "xz"
    case lzip = "lzip"
    case lz4 = "lz4"
    case brotli = "brotli"
    case snappy = "snappy"
    case lrzip = "lrzip"
    
    /// Display name of the format.
    public var displayName: String {
        switch self {
        case .sevenZip: return "7-Zip"
        case .zip: return "ZIP"
        case .tar: return "TAR"
        case .tarGz: return "TAR.GZ"
        case .tarBz2: return "TAR.BZ2"
        case .tarXz: return "TAR.XZ"
        case .tarZst: return "TAR.ZST"
        case .tarLz4: return "TAR.LZ4"
        case .tarBrotli: return "TAR.BR"
        case .tarLzip: return "TAR.LZ"
        case .tarLrzip: return "TAR.LRZ"
        case .iso: return "ISO"
        case .cab: return "CAB"
        case .wim: return "WIM"
        case .dmg: return "DMG"
        case .aar: return "AAR"
        case .cpio: return "CPIO"
        case .ar: return "AR"
        case .deb: return "DEB"
        case .rpm: return "RPM"
        case .xar: return "XAR"
        case .rar: return "RAR"
        case .squashfs: return "SquashFS"
        case .lzfse: return "LZFSE"
        case .lzh: return "LZH"
        case .zst: return "Zstandard"
        case .gz: return "GZIP"
        case .bz2: return "BZIP2"
        case .xz: return "XZ"
        case .lzip: return "LZIP"
        case .lz4: return "LZ4"
        case .brotli: return "Brotli"
        case .snappy: return "Snappy"
        case .lrzip: return "LRZIP"
        }
    }
    
    /// Standard file extension with leading dot.
    public var fileExtension: String {
        switch self {
        case .sevenZip: return ".7z"
        case .zip: return ".zip"
        case .tar: return ".tar"
        case .tarGz: return ".tar.gz"
        case .tarBz2: return ".tar.bz2"
        case .tarXz: return ".tar.xz"
        case .tarZst: return ".tar.zst"
        case .tarLz4: return ".tar.lz4"
        case .tarBrotli: return ".tar.br"
        case .tarLzip: return ".tar.lz"
        case .tarLrzip: return ".tar.lrz"
        case .iso: return ".iso"
        case .cab: return ".cab"
        case .wim: return ".wim"
        case .dmg: return ".dmg"
        case .aar: return ".aar"
        case .cpio: return ".cpio"
        case .ar: return ".ar"
        case .deb: return ".deb"
        case .rpm: return ".rpm"
        case .xar: return ".xar"
        case .rar: return ".rar"
        case .squashfs: return ".squashfs"
        case .lzfse: return ".lzfse"
        case .lzh: return ".lzh"
        case .zst: return ".zst"
        case .gz: return ".gz"
        case .bz2: return ".bz2"
        case .xz: return ".xz"
        case .lzip: return ".lz"
        case .lz4: return ".lz4"
        case .brotli: return ".br"
        case .snappy: return ".sz"
        case .lrzip: return ".lrz"
        }
    }
    
    /// Whether the format is strictly read-only for inspection and extraction.
    public var isReadOnly: Bool {
        switch self {
        case .rar, .squashfs, .lzh:
            return true
        default:
            return false
        }
    }
    
    /// Whether the format supports writing and creating archives.
    public var isWritable: Bool {
        return !isReadOnly
    }
    
    /// Format classification category.
    public var category: ArchiveFormatCategory {
        switch self {
        case .sevenZip, .zip, .tar, .rar, .lzh:
            return .standard
        case .tarGz, .tarBz2, .tarXz, .tarZst, .tarLz4, .tarBrotli, .tarLzip, .tarLrzip, .cpio, .ar, .deb, .rpm, .xar:
            return .unixPackage
        case .iso, .cab, .wim, .dmg, .aar, .squashfs:
            return .diskImage
        case .zst, .gz, .bz2, .xz, .lzip, .lz4, .brotli, .snappy, .lrzip, .lzfse:
            return .modernStream
        }
    }
    
    /// SF Symbol icon name.
    public var iconName: String {
        switch self {
        case .sevenZip, .zip: return "archivebox.fill"
        case .tar: return "archivebox"
        case .tarGz, .tarBz2, .tarXz, .tarZst, .tarLz4, .tarBrotli, .tarLzip, .tarLrzip: return "shippingbox.fill"
        case .iso: return "opticaldisc.fill"
        case .dmg: return "internaldrive.fill"
        case .wim: return "externaldrive.fill"
        case .cab: return "cabinet.fill"
        case .aar, .lzfse: return "apple.logo"
        case .cpio, .ar: return "doc.zipper"
        case .deb, .rpm: return "cube.box.fill"
        case .xar: return "folder.badge.gearshape"
        case .rar: return "lock.doc.fill"
        case .squashfs: return "externaldrive.connected.to.line.below.fill"
        case .lzh: return "archivebox"
        case .zst, .lz4, .brotli, .snappy, .lzip, .lrzip, .gz, .bz2, .xz: return "bolt.fill"
        }
    }
    
    /// Friendly detailed description for guides, tooltips, and inspectors.
    public var formatDescription: String {
        switch self {
        case .sevenZip: return "High compression ratio LZMA2/ZSTD engine with strong AES-256 header encryption."
        case .zip: return "Universal standard container compatible with all major platforms and systems."
        case .tar: return "Standard Unix tape archive container preserving POSIX permissions and metadata."
        case .tarGz: return "Classic GZIP-compressed POSIX tarball standard across Unix and macOS."
        case .tarBz2: return "High-ratio BZIP2-compressed POSIX archive standard."
        case .tarXz: return "Ultra-high compression ratio LZMA2 POSIX tarball."
        case .tarZst: return "Next-generation real-time multi-threaded Zstandard streaming archive."
        case .tarLz4: return "Ultra-fast LZ4 real-time archive for maximum throughput."
        case .tarBrotli: return "Web-optimized Brotli compressed archive."
        case .tarLzip: return "High-integrity LZMA archive with 32-bit CRC validation."
        case .tarLrzip: return "Long-range redundancy compression for massive multi-gigabyte files."
        case .iso: return "ISO 9660 / UDF optical disc image container."
        case .cab: return "Microsoft Cabinet archive for Windows installation packages."
        case .wim: return "Microsoft Windows Imaging Format single-instance container."
        case .dmg: return "Apple UDIF Disk Image container for macOS software distribution."
        case .aar: return "Apple Archive container with Apple Silicon hardware acceleration."
        case .cpio: return "POSIX portable CPIO archive format for system packages and initramfs."
        case .ar: return "Unix common archive format for static libraries and packages."
        case .deb: return "Debian binary software package archive for Debian and Ubuntu."
        case .rpm: return "Red Hat Package Manager binary and source distribution format."
        case .xar: return "eXtensible Archive container used in macOS PKG installer packages."
        case .rar: return "RAR compressed archive (Read-only browsing and extraction)."
        case .squashfs: return "Compressed read-only Linux filesystem image."
        case .lzfse: return "Apple LZFSE high-efficiency compressed stream."
        case .lzh: return "LHA / LZH compressed archive."
        case .zst: return "Standalone Zstandard compressed data stream."
        case .gz: return "Standalone GZIP compressed data stream."
        case .bz2: return "Standalone BZIP2 compressed data stream."
        case .xz: return "Standalone XZ compressed data stream."
        case .lzip: return "Standalone Lzip compressed data stream."
        case .lz4: return "Standalone LZ4 compressed data stream."
        case .brotli: return "Standalone Brotli compressed data stream."
        case .snappy: return "Standalone Google Snappy high-speed framing stream."
        case .lrzip: return "Standalone LRZIP extended-range stream."
        }
    }
    
    /// Keyboard shortcut badge.
    public var shortcutBadge: String {
        switch self {
        case .sevenZip: return "⌥⇧7"
        case .zip: return "⌥⇧Z"
        case .tar: return "⌥⇧T"
        case .tarZst, .zst: return "⌥⇧S"
        case .tarGz, .gz: return "⌥⇧G"
        case .tarBz2, .bz2: return "⌥⇧B"
        case .tarXz, .xz: return "⌥⇧X"
        case .iso: return "⌥⇧I"
        case .cab: return "⌥⇧C"
        case .wim: return "⌥⇧W"
        case .dmg: return "⌥⇧D"
        case .aar: return "⌥⇧A"
        case .cpio: return "⌥⇧O"
        case .ar: return "⌥⇧R"
        case .deb: return "⌥⇧E"
        case .rpm: return "⌥⇧M"
        case .xar: return "⌥⇧K"
        case .rar: return "⌥⇧P"
        case .squashfs: return "⌥⇧Q"
        case .lzfse: return "⌥⇧F"
        case .lzh: return "⌥⇧H"
        case .lzip, .tarLzip: return "⌥⇧L"
        case .lz4, .tarLz4: return "⌥⇧4"
        case .brotli, .tarBrotli: return "⌥⇧Y"
        case .snappy: return "⌥⇧N"
        case .lrzip, .tarLrzip: return "⌥⇧J"
        }
    }
    
    /// Shortcut character for keyboard event matching.
    public var shortcutCharacter: Character {
        switch self {
        case .sevenZip: return "7"
        case .zip: return "z"
        case .tar: return "t"
        case .tarZst, .zst: return "s"
        case .tarGz, .gz: return "g"
        case .tarBz2, .bz2: return "b"
        case .tarXz, .xz: return "x"
        case .iso: return "i"
        case .cab: return "c"
        case .wim: return "w"
        case .dmg: return "d"
        case .aar: return "a"
        case .cpio: return "o"
        case .ar: return "r"
        case .deb: return "e"
        case .rpm: return "m"
        case .xar: return "k"
        case .rar: return "p"
        case .squashfs: return "q"
        case .lzfse: return "f"
        case .lzh: return "h"
        case .lzip, .tarLzip: return "l"
        case .lz4, .tarLz4: return "4"
        case .brotli, .tarBrotli: return "y"
        case .snappy: return "n"
        case .lrzip, .tarLrzip: return "j"
        }
    }
    
    /// Whether format supports password encryption.
    public var supportsPasswordEncryption: Bool {
        switch self {
        case .sevenZip, .zip, .wim, .dmg, .aar, .rar:
            return true
        default:
            return false
        }
    }
    
    /// Whether format supports split volume multi-part archives.
    public var supportsSplitVolume: Bool {
        switch self {
        case .sevenZip, .zip, .wim, .rar:
            return true
        default:
            return false
        }
    }
    
    /// Supported compression levels for format.
    public var supportedLevels: [ArchiveCompressionLevel] {
        switch self {
        case .tar, .dmg, .iso, .aar, .cpio, .ar, .rar, .squashfs, .lzh:
            return [.store]
        case .zip:
            return [.store, .level1, .level6, .level9, .level12]
        case .sevenZip, .zst, .tarZst, .gz, .tarGz, .bz2, .tarBz2, .xz, .tarXz,
             .lzip, .tarLzip, .lz4, .tarLz4, .brotli, .tarBrotli, .lrzip, .tarLrzip,
             .snappy, .wim, .cab, .deb, .rpm, .xar, .lzfse:
            return [.store, .level1, .level6, .level9]
        }
    }
    
    /// Minimum valid compression level integer.
    public var minCompressionLevel: Int { 0 }

    /// Maximum valid compression level integer.
    public var maxCompressionLevel: Int {
        switch self {
        case .tar, .dmg, .iso, .aar, .cpio, .ar, .rar, .squashfs, .lzh:
            return 0
        case .zip:
            return 12
        case .zst, .tarZst:
            return 22
        case .brotli, .tarBrotli:
            return 11
        case .sevenZip, .gz, .tarGz, .bz2, .tarBz2, .xz, .tarXz,
             .lzip, .tarLzip, .lz4, .tarLz4, .lrzip, .tarLrzip,
             .snappy, .wim, .cab, .deb, .rpm, .xar, .lzfse:
            return 9
        }
    }

    /// Valid compression level integer range for format.
    public var validCompressionLevelRange: ClosedRange<Int> {
        minCompressionLevel...maxCompressionLevel
    }
    
    /// List of all 17 primary non-proprietary writable creation formats.
    public static let primary17WritableFormats: [ArchiveCompressionFormat] = [
        .sevenZip, .zip, .tar, .tarGz, .tarBz2, .tarXz, .tarZst,
        .iso, .cab, .wim, .dmg, .aar, .cpio, .ar, .deb, .rpm, .xar
    ]
    
    /// List of all writable creation formats including standalone streams.
    public static let allWritableCases: [ArchiveCompressionFormat] = [
        .sevenZip, .zip, .tar, .tarGz, .tarBz2, .tarXz, .tarZst,
        .iso, .cab, .wim, .dmg, .aar, .cpio, .ar, .deb, .rpm, .xar,
        .tarLz4, .tarBrotli, .tarLzip, .tarLrzip,
        .zst, .gz, .bz2, .xz, .lzip, .lz4, .brotli, .snappy, .lrzip, .lzfse
    ]
    
    /// List of all 18+ readable and inspectable formats.
    public static let allReadableCases: [ArchiveCompressionFormat] = allCases
}

// MARK: - Extension Resolution & Detection Helpers

extension ArchiveCompressionFormat {
    
    /// 7Z / DMG / ISO / Split Volume (.001) compatible extensions set.
    public static let sevenZipFamilyExtensions: Set<String> = [
        ".7z", ".cb7", ".dmg", ".iso", ".001"
    ]

    /// TAR derivative and libarchive compatible extensions set.
    public static let tarFamilyExtensions: Set<String> = [
        ".tar", ".tar.gz", ".tgz", ".tar.zst", ".tzst",
        ".tar.xz", ".txz", ".tar.bz2", ".tbz2", ".tbz", ".tar.lz",
        ".tlz", ".tar.lz4", ".tlz4", ".tar.br", ".tbr", ".tar.lrz", ".tlrz",
        ".gz", ".bz2", ".xz", ".lz", ".lzip", ".zst",
        ".lz4", ".br", ".brotli", ".lrz", ".lrzip", ".sz", ".snappy",
        ".aar", ".wim", ".dmg", ".iso", ".rar", ".cbr",
        ".cab", ".cpio", ".ar", ".a", ".deb", ".rpm", ".xar", ".pkg",
        ".squashfs", ".sqsh", ".lzfse", ".lzh", ".lha"
    ]

    /// Determines whether a filename or path represents a known archive format.
    public static func isArchiveExtension(_ ext: String, path: String = "") -> Bool {
        let lowerExt = ext.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "."))
        if ArchiveCompressionFormat(rawValue: lowerExt) != nil {
            return true
        }
        let dotExt = ".\(lowerExt)"
        if sevenZipFamilyExtensions.contains(dotExt) || tarFamilyExtensions.contains(dotExt) {
            return true
        }
        let archiveExtraExts: Set<String> = [
            "zipx", "cbz", "jar", "apk", "epub", "rar", "cbr", "cab", "001", "002", "003",
            "zst", "iso", "img", "wim", "swm", "esd", "dmg", "aar", "aea",
            "cpio", "ar", "a", "deb", "rpm", "xar", "pkg", "squashfs", "sqsh", "lzfse", "lzh", "lha"
        ]
        if archiveExtraExts.contains(lowerExt) {
            return true
        }
        let lowerPath = path.lowercased()
        if lowerExt.range(of: #"^\d{3}$"#, options: .regularExpression) != nil ||
           lowerPath.contains(".7z.") || lowerPath.contains(".zip.") || lowerPath.contains(".rar.") ||
           lowerPath.contains(".part1.rar") || lowerPath.contains(".z01") {
            return true
        }
        return false
    }

    /// Resolves compression format from extension or name string.
    public static func from(extensionOrName: String) -> ArchiveCompressionFormat? {
        let cleaned = extensionOrName.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "."))
        if let direct = ArchiveCompressionFormat(rawValue: cleaned) {
            return direct
        }
        for format in allCases {
            if format.rawValue.lowercased() == cleaned || format.displayName.lowercased() == cleaned {
                return format
            }
        }
        switch cleaned {
        case "7zip", "sevenzip", "cb7": return .sevenZip
        case "tgz": return .tarGz
        case "tbz", "tbz2": return .tarBz2
        case "txz": return .tarXz
        case "tzst": return .tarZst
        case "tlz4": return .tarLz4
        case "tbr": return .tarBrotli
        case "tlz": return .tarLzip
        case "tlrz": return .tarLrzip
        case "lz": return .lzip
        case "br": return .brotli
        case "lrz": return .lrzip
        case "sz": return .snappy
        case "cbr": return .rar
        case "cbz", "zipx", "jar", "apk", "epub": return .zip
        case "sqsh": return .squashfs
        case "pkg": return .xar
        case "a": return .ar
        case "aea": return .aar
        case "lha": return .lzh
        case "img": return .iso
        case "swm", "esd": return .wim
        default: return nil
        }
    }

    /// Resolves descriptive kind string for an item.
    public static func kindDescription(forExtension ext: String, isArchive: Bool, path: String = "") -> String {
        let lowerExt = ext.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "."))
        if let format = ArchiveCompressionFormat.from(extensionOrName: lowerExt) {
            return "\(format.displayName) Archive"
        }
        if isArchive {
            return "Archive Package"
        }
        
        switch lowerExt {
        case "jpg", "jpeg": return "JPEG Image"
        case "png": return "PNG Image"
        case "gif": return "GIF Animation"
        case "webp": return "WebP Image"
        case "heic": return "HEIC Image"
        case "svg": return "SVG Vector Image"
        case "pdf": return "PDF Document"
        case "mp4", "mov", "m4v", "mkv", "avi": return "MPEG-4 Video"
        case "mp3", "wav", "m4a", "flac", "aac": return "Audio File"
        case "txt", "md", "markdown": return "Text Document"
        case "swift", "py", "json", "js", "ts", "cpp", "c", "h", "rs", "go", "html", "css": return "Source Code"
        case "csv", "tsv": return "Spreadsheet Document"
        default: return "\(ext.uppercased()) File"
        }
    }
}
