// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import TTZipCoreBindings
import Foundation

/// Represents the authoritative standards definition for an archive or compression format.
public struct ArchiveFormatStandardSpec: Sendable, Equatable, Identifiable, Codable {
    public let id: String
    public let format: ArchiveCompressionFormat
    public let officialName: String
    public let standardCitations: [StandardCitation]
    public let mimeType: String
    public let appleUTI: String
    public let magicSignatures: [ArchiveMagicSignature]
    public let supportedEncryption: [EncryptionStandardSpec]
    public let supportsMultiVolume: Bool
    public let supportedExtraFields: [ZipExtraFieldStandardSpec]

    public init(
        id: String,
        format: ArchiveCompressionFormat,
        officialName: String,
        standardCitations: [StandardCitation],
        mimeType: String,
        appleUTI: String,
        magicSignatures: [ArchiveMagicSignature],
        supportedEncryption: [EncryptionStandardSpec] = [],
        supportsMultiVolume: Bool = false,
        supportedExtraFields: [ZipExtraFieldStandardSpec] = []
    ) {
        self.id = id
        self.format = format
        self.officialName = officialName
        self.standardCitations = standardCitations
        self.mimeType = mimeType
        self.appleUTI = appleUTI
        self.magicSignatures = magicSignatures
        self.supportedEncryption = supportedEncryption
        self.supportsMultiVolume = supportsMultiVolume
        self.supportedExtraFields = supportedExtraFields
    }
}

/// Official citation of an RFC, ISO, IEEE, POSIX, or vendor specification.
public struct StandardCitation: Sendable, Equatable, Codable {
    public let organization: String
    public let standardNumber: String
    public let title: String
    public let canonicalURL: String

    public init(
        organization: String,
        standardNumber: String,
        title: String,
        canonicalURL: String
    ) {
        self.organization = organization
        self.standardNumber = standardNumber
        self.title = title
        self.canonicalURL = canonicalURL
    }
}

/// Magic signature definition with position anchoring.
public struct ArchiveMagicSignature: Sendable, Equatable, Codable {
    public enum Anchor: Sendable, Equatable, Codable {
        case head(offset: Int)
        case tail(offsetFromEOF: Int)
        case sector(sectorIndex: Int, byteOffset: Int)
        case tarOffset(byteOffset: Int)

        private enum CodingKeys: String, CodingKey {
            case type
            case offset
            case sectorIndex
            case byteOffset
        }

        public init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let type = try container.decode(String.self, forKey: .type)
            switch type {
            case "head":
                let offset = try container.decode(Int.self, forKey: .offset)
                self = .head(offset: offset)
            case "tail":
                let offset = try container.decode(Int.self, forKey: .offset)
                self = .tail(offsetFromEOF: offset)
            case "sector":
                let sector = try container.decode(Int.self, forKey: .sectorIndex)
                let byteOffset = try container.decode(Int.self, forKey: .byteOffset)
                self = .sector(sectorIndex: sector, byteOffset: byteOffset)
            case "tarOffset":
                let byteOffset = try container.decode(Int.self, forKey: .byteOffset)
                self = .tarOffset(byteOffset: byteOffset)
            default:
                throw DecodingError.dataCorrupted(
                    DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Unknown anchor type: \(type)")
                )
            }
        }

        public func encode(to encoder: Encoder) throws {
            var container = encoder.container(keyedBy: CodingKeys.self)
            switch self {
            case .head(let offset):
                try container.encode("head", forKey: .type)
                try container.encode(offset, forKey: .offset)
            case .tail(let offsetFromEOF):
                try container.encode("tail", forKey: .type)
                try container.encode(offsetFromEOF, forKey: .offset)
            case .sector(let sectorIndex, let byteOffset):
                try container.encode("sector", forKey: .type)
                try container.encode(sectorIndex, forKey: .sectorIndex)
                try container.encode(byteOffset, forKey: .byteOffset)
            case .tarOffset(let byteOffset):
                try container.encode("tarOffset", forKey: .type)
                try container.encode(byteOffset, forKey: .byteOffset)
            }
        }
    }

    public let anchor: Anchor
    public let bytes: [UInt8]
    public let description: String

    public var bytesHex: String {
        bytes.map { String(format: "%02X", $0) }.joined()
    }

    public init(
        anchor: Anchor,
        bytes: [UInt8],
        description: String
    ) {
        self.anchor = anchor
        self.bytes = bytes
        self.description = description
    }

    private enum CodingKeys: String, CodingKey {
        case anchorType
        case offset
        case bytesHex
        case description
        case anchor
        case bytes
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        if container.contains(.anchor) && container.contains(.bytes) {
            self.anchor = try container.decode(Anchor.self, forKey: .anchor)
            self.bytes = try container.decode([UInt8].self, forKey: .bytes)
            self.description = try container.decode(String.self, forKey: .description)
        } else {
            let anchorType = try container.decode(String.self, forKey: .anchorType)
            let offset = try container.decodeIfPresent(Int.self, forKey: .offset) ?? 0
            switch anchorType {
            case "head":
                self.anchor = .head(offset: offset)
            case "tail":
                self.anchor = .tail(offsetFromEOF: offset)
            case "sector":
                self.anchor = .sector(sectorIndex: offset / 2048, byteOffset: offset % 2048)
            case "tarOffset":
                self.anchor = .tarOffset(byteOffset: offset)
            default:
                self.anchor = .head(offset: offset)
            }
            let hex = try container.decode(String.self, forKey: .bytesHex)
            var byteArray: [UInt8] = []
            var index = hex.startIndex
            while index < hex.endIndex {
                let nextIndex = hex.index(index, offsetBy: 2, limitedBy: hex.endIndex) ?? hex.endIndex
                if let byte = UInt8(hex[index..<nextIndex], radix: 16) {
                    byteArray.append(byte)
                }
                index = nextIndex
            }
            self.bytes = byteArray
            self.description = try container.decode(String.self, forKey: .description)
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch anchor {
        case .head(let offset):
            try container.encode("head", forKey: .anchorType)
            try container.encode(offset, forKey: .offset)
        case .tail(let offsetFromEOF):
            try container.encode("tail", forKey: .anchorType)
            try container.encode(offsetFromEOF, forKey: .offset)
        case .sector(let sectorIndex, let byteOffset):
            try container.encode("sector", forKey: .anchorType)
            try container.encode(sectorIndex * 2048 + byteOffset, forKey: .offset)
        case .tarOffset(let byteOffset):
            try container.encode("tarOffset", forKey: .anchorType)
            try container.encode(byteOffset, forKey: .offset)
        }
        try container.encode(bytesHex, forKey: .bytesHex)
        try container.encode(description, forKey: .description)
    }
}

/// Specification of an archive encryption standard.
public struct EncryptionStandardSpec: Sendable, Equatable, Codable {
    public let standardName: String
    public let keyDerivationFunction: String
    public let cipher: String
    public let authenticationTag: String?

    public init(
        standardName: String,
        keyDerivationFunction: String,
        cipher: String,
        authenticationTag: String? = nil
    ) {
        self.standardName = standardName
        self.keyDerivationFunction = keyDerivationFunction
        self.cipher = cipher
        self.authenticationTag = authenticationTag
    }
}

/// Standard definition of a ZIP Extra Field header tag.
public struct ZipExtraFieldStandardSpec: Sendable, Equatable, Codable {
    public let headerID: UInt16
    public let name: String
    public let sourceSpecification: String

    public init(
        headerID: UInt16,
        name: String,
        sourceSpecification: String
    ) {
        self.headerID = headerID
        self.name = name
        self.sourceSpecification = sourceSpecification
    }
}

// MARK: - Magic Signature Scanner

//
//


/// High-performance, zero-allocation multi-anchor magic signature scanner.
///
/// Dispatches primary format sniffing and SFX detection to high-performance Rust C-ABI
/// while providing zero-copy fallback inspection across registered specifications.
public enum ArchiveMagicSignatureScanner {

    /// Prioritized sequence of formats for signature scanning.
    public static let prioritizedFormats: [ArchiveCompressionFormat] = [
        .wim,       // 8 bytes (MSWIM\0\0\0)
        .snappy,    // 10 bytes (\xFF\x06\x00\x00sNaPpY)
        .sevenZip,  // 6 bytes (37 7A BC AF 27 1C)
        .xz,        // 6 bytes (\xFD7zXZ\x00) + 2 bytes footer (YZ)
        .iso,       // Sector 16, offset 1 (CD001 / BEA01)
        .dmg,       // Tail 512 (koly)
        .aar,       // 4 bytes (AA01 / AEA1)
        .lzip,      // 4 bytes (LZIP)
        .lrzip,     // 4 bytes (LRZI)
        .lz4,       // 4 bytes (0x184D2204 / 0x184C2102)
        .zst,       // 4 bytes (0xFD2FB528)
        .tar,       // Offset 257 (ustar\0 / ustar  \0)
        .zip,       // 4 bytes (PK\x03\x04 / PK\x05\x06 / PK\x07\x08 / EOCD)
        .bz2,       // 3 bytes (BZh)
        .gz         // 2 bytes (0x1F8B)
    ]

    // MARK: - Offset Calculation

    /// Resolves the absolute starting byte offset in the archive stream for a given anchor.
    @inline(__always)
    public static func targetOffset(for anchor: ArchiveMagicSignature.Anchor, fileSize: Int64) -> Int64 {
        switch anchor {
        case .head(let offset):
            return Int64(offset)
        case .tail(let offsetFromEOF):
            return fileSize - Int64(offsetFromEOF)
        case .sector(let sectorIndex, let byteOffset):
            return Int64(sectorIndex) * 2048 + Int64(byteOffset)
        case .tarOffset(let byteOffset):
            return Int64(byteOffset)
        }
    }

    // MARK: - Buffer Matching (Zero Heap Allocation)

    /// Verifies if a magic signature matches the contents of an in-memory raw buffer at its designated anchor.
    @inline(__always)
    public static func matchesSignature(
        _ signature: ArchiveMagicSignature,
        in buffer: UnsafeRawBufferPointer,
        fileSize: Int64
    ) -> Bool {
        let sigBytes = signature.bytes
        let sigLen = sigBytes.count
        guard sigLen > 0 else { return false }

        let offset = targetOffset(for: signature.anchor, fileSize: fileSize)
        guard offset >= 0 else { return false }

        let endOffset = offset + Int64(sigLen)
        guard endOffset <= fileSize, endOffset <= Int64(buffer.count) else { return false }
        guard let baseAddress = buffer.baseAddress else { return false }

        let targetPtr = baseAddress.advanced(by: Int(offset))
        return sigBytes.withUnsafeBufferPointer { sigBuf in
            guard let sigBase = sigBuf.baseAddress else { return false }
            return memcmp(targetPtr, sigBase, sigLen) == 0
        }
    }

    @inline(__always)
    public static func matchesSignature(
        _ signature: ArchiveMagicSignature,
        in buffer: UnsafeBufferPointer<UInt8>,
        fileSize: Int64
    ) -> Bool {
        return matchesSignature(signature, in: UnsafeRawBufferPointer(buffer), fileSize: fileSize)
    }

    @inline(__always)
    public static func matchesSignature(
        _ signature: ArchiveMagicSignature,
        in data: Data,
        fileSize: Int64? = nil
    ) -> Bool {
        let size = fileSize ?? Int64(data.count)
        return data.withUnsafeBytes { rawBuffer in
            matchesSignature(signature, in: rawBuffer, fileSize: size)
        }
    }

    // MARK: - FileHandle Matching

    public static func matchesSignature(
        _ signature: ArchiveMagicSignature,
        fileHandle: FileHandle,
        fileSize: Int64
    ) throws -> Bool {
        let sigBytes = signature.bytes
        let sigLen = sigBytes.count
        guard sigLen > 0 else { return false }

        let offset = targetOffset(for: signature.anchor, fileSize: fileSize)
        guard offset >= 0, offset + Int64(sigLen) <= fileSize else { return false }

        try fileHandle.seek(toOffset: UInt64(offset))
        guard let data = try fileHandle.read(upToCount: sigLen), data.count == sigLen else {
            return false
        }

        return data.withUnsafeBytes { dataBuf in
            guard let dataBase = dataBuf.baseAddress else { return false }
            return sigBytes.withUnsafeBufferPointer { sigBuf in
                guard let sigBase = sigBuf.baseAddress else { return false }
                return memcmp(dataBase, sigBase, sigLen) == 0
            }
        }
    }

    // MARK: - Format Detection (Buffer & Rust C-ABI Bridge)

    /// Detects the archive or compression format of an in-memory buffer by evaluating magic signatures.
    public static func detectFormat(
        buffer: UnsafeRawBufferPointer,
        fileSize: Int64
    ) -> ArchiveCompressionFormat? {
        guard !buffer.isEmpty, fileSize > 0, buffer.baseAddress != nil else { return nil }

        // Evaluation against format standard signatures
        let registry = ArchiveFormatStandardRegistry.shared
        for format in prioritizedFormats {
            guard let spec = registry.spec(for: format) else { continue }
            for signature in spec.magicSignatures {
                if matchesSignature(signature, in: buffer, fileSize: fileSize) {
                    return format
                }
            }
        }
        for spec in registry.allSpecs() {
            if prioritizedFormats.contains(spec.format) { continue }
            for signature in spec.magicSignatures {
                if matchesSignature(signature, in: buffer, fileSize: fileSize) {
                    return spec.format
                }
            }
        }

        return nil
    }

    public static func detectFormat(data: Data) -> ArchiveCompressionFormat? {
        let size = Int64(data.count)
        guard size > 0 else { return nil }
        return data.withUnsafeBytes { rawBuffer in
            detectFormat(buffer: rawBuffer, fileSize: size)
        }
    }

    // MARK: - Format Detection (File URL & Path)

    private static let uniffiFormatMap: [ArchiveFormat: ArchiveCompressionFormat] = [
        .zip: .zip, .sevenZip: .sevenZip, .tar: .tar,
        .tarGz: .tarGz, .tarBz2: .tarBz2, .tarXz: .tarXz, .tarZstd: .tarZst,
        .tarLz4: .tarLz4, .tarBrotli: .tarBrotli, .tarLzip: .tarLzip, .tarLrzip: .tarLrzip,
        .gzip: .gz, .bzip2: .bz2, .xz: .xz, .zstd: .zst, .lz4: .lz4, .brotli: .brotli,
        .iso: .iso, .cab: .cab, .wim: .wim, .dmg: .dmg, .aar: .aar,
        .cpio: .cpio, .ar: .ar, .deb: .deb, .rpm: .rpm, .xar: .xar,
        .rar: .rar, .squashfs: .squashfs, .lzfse: .lzfse, .lzh: .lzh,
        .snappy: .snappy, .lzip: .lzip, .lrzip: .lrzip, .auto: .zip
    ]

    public static func detectFormat(fileURL: URL) throws -> ArchiveCompressionFormat? {
        let path = fileURL.path
        guard FileManager.default.fileExists(atPath: path) else { return nil }

        if let uniffiFmt = try? detectArchiveFormat(path: path), uniffiFmt != .auto {
            let format = uniffiFormatMap[uniffiFmt] ?? .zip
            return resolveCompoundFormat(detected: format, fileURL: fileURL)
        }

        return detectFormatFromExtension(fileURL: fileURL)
    }

    public static func detectFormat(path: String) throws -> ArchiveCompressionFormat? {
        return try detectFormat(fileURL: URL(fileURLWithPath: path))
    }

    // MARK: - Mapping & Compound Resolution Helpers

    private static let detectedFormatCodeMap: [Int32: ArchiveCompressionFormat] = [
        1: .zip, 2: .sevenZip, 3: .tar, 4: .gz, 5: .bz2, 6: .xz, 7: .zst,
        10: .iso, 11: .dmg, 12: .cab, 13: .cpio, 14: .ar, 15: .deb,
        16: .snappy, 17: .lz4, 18: .lzip, 19: .lrzip, 20: .brotli,
        21: .aar, 22: .wim, 23: .rpm, 24: .xar, 25: .rar,
        26: .squashfs, 27: .lzfse, 28: .lzh
    ]

    @inline(__always)
    private static func mapDetectedFormat(_ code: Int32) -> ArchiveCompressionFormat? {
        detectedFormatCodeMap[code]
    }

    private static let compoundSuffixRules: [(streamFormat: ArchiveCompressionFormat, suffixes: [String], compound: ArchiveCompressionFormat)] = [
        (.gz, [".tar.gz", ".tgz"], .tarGz),
        (.bz2, [".tar.bz2", ".tbz2", ".tbz"], .tarBz2),
        (.xz, [".tar.xz", ".txz"], .tarXz),
        (.zst, [".tar.zst", ".tzst"], .tarZst),
        (.lz4, [".tar.lz4", ".tlz4"], .tarLz4),
        (.brotli, [".tar.br", ".tbr"], .tarBrotli),
        (.lzip, [".tar.lz", ".tlz"], .tarLzip),
        (.lrzip, [".tar.lrz", ".tlrz"], .tarLrzip)
    ]

    private static func resolveCompoundFormat(
        detected: ArchiveCompressionFormat,
        fileURL: URL
    ) -> ArchiveCompressionFormat {
        let lower = fileURL.lastPathComponent.lowercased()
        for rule in compoundSuffixRules where rule.streamFormat == detected {
            if rule.suffixes.contains(where: { lower.hasSuffix($0) }) {
                return rule.compound
            }
        }
        return detected
    }

    private static let extensionSuffixRules: [(suffixes: [String], format: ArchiveCompressionFormat)] = [
        ([".tar.gz", ".tgz"], .tarGz),
        ([".tar.bz2", ".tbz2", ".tbz"], .tarBz2),
        ([".tar.xz", ".txz"], .tarXz),
        ([".tar.zst", ".tzst"], .tarZst),
        ([".tar.lz4", ".tlz4"], .tarLz4),
        ([".tar.br", ".tbr"], .tarBrotli),
        ([".tar.lz", ".tlz"], .tarLzip),
        ([".tar.lrz", ".tlrz"], .tarLrzip),
        ([".tar", ".cbt"], .tar),
        ([".zip", ".zipx", ".jar", ".apk", ".cbz", ".epub"], .zip),
        ([".7z", ".cb7"], .sevenZip),
        ([".gz"], .gz),
        ([".bz2"], .bz2),
        ([".xz"], .xz),
        ([".zst"], .zst),
        ([".lz", ".lzip"], .lzip),
        ([".lz4"], .lz4),
        ([".br", ".brotli"], .brotli),
        ([".lrz", ".lrzip"], .lrzip),
        ([".aar", ".aea"], .aar),
        ([".sz", ".snappy"], .snappy),
        ([".wim", ".swm", ".esd"], .wim),
        ([".dmg"], .dmg),
        ([".iso", ".img"], .iso),
        ([".cab"], .cab),
        ([".cpio"], .cpio),
        ([".ar", ".a"], .ar),
        ([".deb"], .deb),
        ([".rpm"], .rpm),
        ([".xar", ".pkg"], .xar),
        ([".rar", ".cbr"], .rar),
        ([".squashfs", ".sqsh"], .squashfs),
        ([".lzfse"], .lzfse),
        ([".lzh", ".lha"], .lzh)
    ]

    private static func detectFormatFromExtension(fileURL: URL) -> ArchiveCompressionFormat? {
        let lower = fileURL.lastPathComponent.lowercased()
        for rule in extensionSuffixRules {
            if rule.suffixes.contains(where: { lower.hasSuffix($0) }) {
                return rule.format
            }
        }
        return nil
    }
}
