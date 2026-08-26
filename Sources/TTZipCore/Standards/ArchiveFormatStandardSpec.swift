// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

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

    public static func detectFormat(fileURL: URL) throws -> ArchiveCompressionFormat? {
        let path = fileURL.path
        guard FileManager.default.fileExists(atPath: path) else { return nil }

        if let uniffiFmt = try? detectArchiveFormat(path: path), uniffiFmt != .auto {
            let format: ArchiveCompressionFormat
            switch uniffiFmt {
            case .zip: format = .zip
            case .sevenZip: format = .sevenZip
            case .tar: format = .tar
            case .tarGz: format = .tarGz
            case .tarBz2: format = .tarBz2
            case .tarXz: format = .tarXz
            case .tarZstd: format = .tarZst
            case .tarLz4: format = .tarLz4
            case .tarBrotli: format = .tarBrotli
            case .tarLzip: format = .tarLzip
            case .tarLrzip: format = .tarLrzip
            case .gzip: format = .gz
            case .bzip2: format = .bz2
            case .xz: format = .xz
            case .zstd: format = .zst
            case .lz4: format = .lz4
            case .brotli: format = .brotli
            case .iso: format = .iso
            case .cab: format = .cab
            case .wim: format = .wim
            case .dmg: format = .dmg
            case .aar: format = .aar
            case .cpio: format = .cpio
            case .ar: format = .ar
            case .deb: format = .deb
            case .rpm: format = .rpm
            case .xar: format = .xar
            case .rar: format = .rar
            case .squashfs: format = .squashfs
            case .lzfse: format = .lzfse
            case .lzh: format = .lzh
            case .snappy: format = .snappy
            case .lzip: format = .lzip
            case .lrzip: format = .lrzip
            case .auto: format = .zip
            }
            return resolveCompoundFormat(detected: format, fileURL: fileURL)
        }

        return detectFormatFromExtension(fileURL: fileURL)
    }

    public static func detectFormat(path: String) throws -> ArchiveCompressionFormat? {
        return try detectFormat(fileURL: URL(fileURLWithPath: path))
    }

    // MARK: - Mapping & Compound Resolution Helpers

    @inline(__always)
    private static func mapDetectedFormat(_ code: Int32) -> ArchiveCompressionFormat? {
        switch code {
        case 1: return .zip
        case 2: return .sevenZip
        case 3: return .tar
        case 4: return .gz
        case 5: return .bz2
        case 6: return .xz
        case 7: return .zst
        case 10: return .iso
        case 11: return .dmg
        case 12: return .cab
        case 13: return .cpio
        case 14: return .ar
        case 15: return .deb
        case 16: return .snappy
        case 17: return .lz4
        case 18: return .lzip
        case 19: return .lrzip
        case 20: return .brotli
        case 21: return .aar
        case 22: return .wim
        case 23: return .rpm
        case 24: return .xar
        case 25: return .rar
        case 26: return .squashfs
        case 27: return .lzfse
        case 28: return .lzh
        default: return nil
        }
    }

    private static func resolveCompoundFormat(
        detected: ArchiveCompressionFormat,
        fileURL: URL
    ) -> ArchiveCompressionFormat {
        let lower = fileURL.lastPathComponent.lowercased()
        switch detected {
        case .gz where lower.hasSuffix(".tar.gz") || lower.hasSuffix(".tgz"):
            return .tarGz
        case .bz2 where lower.hasSuffix(".tar.bz2") || lower.hasSuffix(".tbz2") || lower.hasSuffix(".tbz"):
            return .tarBz2
        case .xz where lower.hasSuffix(".tar.xz") || lower.hasSuffix(".txz"):
            return .tarXz
        case .zst where lower.hasSuffix(".tar.zst") || lower.hasSuffix(".tzst"):
            return .tarZst
        case .lz4 where lower.hasSuffix(".tar.lz4") || lower.hasSuffix(".tlz4"):
            return .tarLz4
        case .brotli where lower.hasSuffix(".tar.br") || lower.hasSuffix(".tbr"):
            return .tarBrotli
        case .lzip where lower.hasSuffix(".tar.lz") || lower.hasSuffix(".tlz"):
            return .tarLzip
        case .lrzip where lower.hasSuffix(".tar.lrz") || lower.hasSuffix(".tlrz"):
            return .tarLrzip
        default:
            return detected
        }
    }

    private static func detectFormatFromExtension(fileURL: URL) -> ArchiveCompressionFormat? {
        let lower = fileURL.lastPathComponent.lowercased()
        if lower.hasSuffix(".tar.gz") || lower.hasSuffix(".tgz") { return .tarGz }
        if lower.hasSuffix(".tar.bz2") || lower.hasSuffix(".tbz2") || lower.hasSuffix(".tbz") { return .tarBz2 }
        if lower.hasSuffix(".tar.xz") || lower.hasSuffix(".txz") { return .tarXz }
        if lower.hasSuffix(".tar.zst") || lower.hasSuffix(".tzst") { return .tarZst }
        if lower.hasSuffix(".tar.lz4") || lower.hasSuffix(".tlz4") { return .tarLz4 }
        if lower.hasSuffix(".tar.br") || lower.hasSuffix(".tbr") { return .tarBrotli }
        if lower.hasSuffix(".tar.lz") || lower.hasSuffix(".tlz") { return .tarLzip }
        if lower.hasSuffix(".tar.lrz") || lower.hasSuffix(".tlrz") { return .tarLrzip }
        if lower.hasSuffix(".tar") || lower.hasSuffix(".cbt") { return .tar }
        if lower.hasSuffix(".zip") || lower.hasSuffix(".zipx") || lower.hasSuffix(".jar") || lower.hasSuffix(".apk") || lower.hasSuffix(".cbz") || lower.hasSuffix(".epub") { return .zip }
        if lower.hasSuffix(".7z") || lower.hasSuffix(".cb7") { return .sevenZip }
        if lower.hasSuffix(".gz") { return .gz }
        if lower.hasSuffix(".bz2") { return .bz2 }
        if lower.hasSuffix(".xz") { return .xz }
        if lower.hasSuffix(".zst") { return .zst }
        if lower.hasSuffix(".lz") || lower.hasSuffix(".lzip") { return .lzip }
        if lower.hasSuffix(".lz4") { return .lz4 }
        if lower.hasSuffix(".br") || lower.hasSuffix(".brotli") { return .brotli }
        if lower.hasSuffix(".lrz") || lower.hasSuffix(".lrzip") { return .lrzip }
        if lower.hasSuffix(".aar") || lower.hasSuffix(".aea") { return .aar }
        if lower.hasSuffix(".sz") || lower.hasSuffix(".snappy") { return .snappy }
        if lower.hasSuffix(".wim") || lower.hasSuffix(".swm") || lower.hasSuffix(".esd") { return .wim }
        if lower.hasSuffix(".dmg") { return .dmg }
        if lower.hasSuffix(".iso") || lower.hasSuffix(".img") { return .iso }
        if lower.hasSuffix(".cab") { return .cab }
        if lower.hasSuffix(".cpio") { return .cpio }
        if lower.hasSuffix(".ar") || lower.hasSuffix(".a") { return .ar }
        if lower.hasSuffix(".deb") { return .deb }
        if lower.hasSuffix(".rpm") { return .rpm }
        if lower.hasSuffix(".xar") || lower.hasSuffix(".pkg") { return .xar }
        if lower.hasSuffix(".rar") || lower.hasSuffix(".cbr") { return .rar }
        if lower.hasSuffix(".squashfs") || lower.hasSuffix(".sqsh") { return .squashfs }
        if lower.hasSuffix(".lzfse") { return .lzfse }
        if lower.hasSuffix(".lzh") || lower.hasSuffix(".lha") { return .lzh }
        return nil
    }
}
