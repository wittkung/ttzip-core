// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

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

public enum ArchiveCompressionFormat: String, Sendable, CaseIterable, Codable {
    case sevenZip = "7z"
    case zip = "zip"
    case tar = "tar"

    case tarGz = "tar.gz"
    case tarBz2 = "tar.bz2"
    case tarXz = "tar.xz"
    case tarZst = "tar.zst"
    case tarLz4 = "tar.lz4"
    case tarBrotli = "tar.br"
    case tarLzip = "tar.lz"
    case tarLrzip = "tar.lrz"

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

    case rar = "rar"
    case squashfs = "squashfs"
    case lzfse = "lzfse"
    case lzh = "lzh"

    case zst = "zst"
    case gz = "gz"
    case bz2 = "bz2"
    case xz = "xz"
    case lzip = "lzip"
    case lz4 = "lz4"
    case brotli = "brotli"
    case snappy = "snappy"
    case lrzip = "lrzip"

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

    public var isReadOnly: Bool {
        switch self {
        case .rar, .squashfs, .lzh, .lzfse, .dmg, .cab, .wim, .deb, .rpm:
            return true
        default:
            return false
        }
    }

    public var isWritable: Bool {
        !isReadOnly
    }

    public var isSingleStreamFormat: Bool {
        switch self {
        case .zst, .gz, .bz2, .xz, .lzip, .lz4, .brotli, .snappy, .lrzip, .lzfse:
            return true
        default:
            return false
        }
    }

    /// Single-volume archives in these formats accept an embedded RS-FEC `TTZR`/`TTRC` recovery trailer.
    public var supportsRecoveryRecord: Bool {
        switch self {
        case .zip, .sevenZip, .tar:
            return true
        default:
            return false
        }
    }

    public var correspondingTarCompositeFormat: ArchiveCompressionFormat? {
        switch self {
        case .gz: return .tarGz
        case .bz2: return .tarBz2
        case .xz: return .tarXz
        case .zst: return .tarZst
        case .lz4: return .tarLz4
        case .brotli: return .tarBrotli
        case .lzip: return .tarLzip
        case .lrzip: return .tarLrzip
        default: return nil
        }
    }

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

    public static let primary17WritableFormats: [ArchiveCompressionFormat] = [
        .sevenZip, .zip, .tar, .tarGz, .tarBz2, .tarXz, .tarZst,
        .tarLz4, .tarBrotli, .tarLzip, .tarLrzip,
        .iso, .aar, .cpio, .ar, .xar
    ]

    public static let allWritableCases: [ArchiveCompressionFormat] = [
        .sevenZip, .zip, .tar, .tarGz, .tarBz2, .tarXz, .tarZst,
        .iso, .aar, .cpio, .ar, .xar,
        .tarLz4, .tarBrotli, .tarLzip, .tarLrzip,
        .zst, .gz, .bz2, .xz, .lzip, .lz4, .brotli, .snappy, .lrzip
    ]

    public static let allReadableCases: [ArchiveCompressionFormat] = allCases
}

extension ArchiveCompressionFormat {
    public static let supportedArchiveExtensions: Set<String> = [
        "7z", "cb7", "zip", "zipx", "tar", "tgz", "tbz", "tbz2", "txz", "tzst",
        "tlz4", "tbr", "tlz", "tlrz", "gz", "bz2", "xz", "zst", "lz4", "br",
        "brotli", "lz", "lzip", "lrz", "lrzip", "sz", "snappy", "iso", "img",
        "cab", "wim", "swm", "esd", "dmg", "aar", "aea", "cpio", "ar", "a",
        "deb", "rpm", "xar", "pkg", "rar", "squashfs", "sqsh", "lzfse", "lzh",
        "lha", "001", "z01"
    ]

    public static let sevenZipFamilyExtensions: Set<String> = [
        ".7z", ".cb7", ".dmg", ".iso", ".001"
    ]

    public static let tarFamilyExtensions: Set<String> = [
        ".tar", ".tar.gz", ".tgz", ".tar.zst", ".tzst",
        ".tar.xz", ".txz", ".tar.bz2", ".tbz2", ".tbz", ".tar.lz",
        ".tlz", ".tar.lz4", ".tlz4", ".tar.br", ".tbr", ".tar.lrz", ".tlrz",
        ".gz", ".bz2", ".xz", ".lz", ".lzip", ".zst",
        ".lz4", ".br", ".brotli", ".lrz", ".lrzip", ".sz", ".snappy",
        ".aar", ".wim", ".dmg", ".iso", ".rar",
        ".cab", ".cpio", ".ar", ".a", ".deb", ".rpm", ".xar", ".pkg",
        ".squashfs", ".sqsh", ".lzfse", ".lzh", ".lha"
    ]

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
            "zipx", "rar", "cab", "001", "002", "003", "z01", "z02", "z03",
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

    private static let extensionAliasMap: [String: ArchiveCompressionFormat] = [
        "7zip": .sevenZip, "sevenzip": .sevenZip, "cb7": .sevenZip,
        "tgz": .tarGz, "tbz": .tarBz2, "tbz2": .tarBz2, "txz": .tarXz,
        "tzst": .tarZst, "tlz4": .tarLz4, "tbr": .tarBrotli,
        "tlz": .tarLzip, "tlrz": .tarLrzip, "lz": .lzip, "br": .brotli,
        "lrz": .lrzip, "sz": .snappy, "zipx": .zip, "sqsh": .squashfs,
        "pkg": .xar, "a": .ar, "aea": .aar, "lha": .lzh,
        "img": .iso, "swm": .wim, "esd": .wim
    ]

    public static func from(extensionOrName: String) -> ArchiveCompressionFormat? {
        let cleaned = extensionOrName.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "."))
        if let direct = ArchiveCompressionFormat(rawValue: cleaned) {
            return direct
        }
        if let alias = extensionAliasMap[cleaned] {
            return alias
        }
        return allCases.first {
            $0.rawValue.lowercased() == cleaned || $0.displayName.lowercased() == cleaned
        }
    }

    private static let nonArchiveKindMap: [String: String] = [
        "jpg": "JPEG Image", "jpeg": "JPEG Image",
        "png": "PNG Image",
        "gif": "GIF Animation",
        "webp": "WebP Image",
        "heic": "HEIC Image",
        "svg": "SVG Vector Image",
        "pdf": "PDF Document",
        "mp4": "MPEG-4 Video", "mov": "MPEG-4 Video", "m4v": "MPEG-4 Video", "mkv": "MPEG-4 Video", "avi": "MPEG-4 Video",
        "mp3": "Audio File", "wav": "Audio File", "m4a": "Audio File", "flac": "Audio File", "aac": "Audio File",
        "txt": "Text Document", "md": "Text Document", "markdown": "Text Document",
        "swift": "Source Code", "py": "Source Code", "json": "Source Code", "js": "Source Code", "ts": "Source Code",
        "cpp": "Source Code", "c": "Source Code", "h": "Source Code", "rs": "Source Code", "go": "Source Code",
        "html": "Source Code", "css": "Source Code",
        "csv": "Spreadsheet Document", "tsv": "Spreadsheet Document"
    ]

    public static func kindDescription(forExtension ext: String, isArchive: Bool, path: String = "") -> String {
        let lowerExt = ext.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "."))
        if let format = ArchiveCompressionFormat.from(extensionOrName: lowerExt) {
            return "\(format.displayName) Archive"
        }
        if isArchive {
            return "Archive Package"
        }
        if let kind = nonArchiveKindMap[lowerExt] {
            return kind
        }
        return "\(ext.uppercased()) File"
    }
}
