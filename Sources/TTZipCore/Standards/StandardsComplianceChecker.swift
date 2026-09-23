// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

/// State representation for archive inspection and standards compliance auditing.
public struct ArchiveInspectorState: Sendable, Equatable {
    public let filePath: String
    public let fileName: String
    public let fileByteSize: Int64
    public let detectedFormat: ArchiveCompressionFormat?
    public let standardSpec: ArchiveFormatStandardSpec?
    public let signatureMatches: [ArchiveMagicSignature]
    public let parsedExtraFields: ParsedZipExtraFields?
    public let complianceReport: StandardsComplianceReport?
    public let isScanning: Bool
    public let scanDurationMs: Double
    public let errorMessage: String?
    
    public init(
        filePath: String,
        fileName: String,
        fileByteSize: Int64,
        detectedFormat: ArchiveCompressionFormat?,
        standardSpec: ArchiveFormatStandardSpec?,
        signatureMatches: [ArchiveMagicSignature],
        parsedExtraFields: ParsedZipExtraFields?,
        complianceReport: StandardsComplianceReport?,
        isScanning: Bool,
        scanDurationMs: Double,
        errorMessage: String?
    ) {
        self.filePath = filePath
        self.fileName = fileName
        self.fileByteSize = fileByteSize
        self.detectedFormat = detectedFormat
        self.standardSpec = standardSpec
        self.signatureMatches = signatureMatches
        self.parsedExtraFields = parsedExtraFields
        self.complianceReport = complianceReport
        self.isScanning = isScanning
        self.scanDurationMs = scanDurationMs
        self.errorMessage = errorMessage
    }
}

/// Cache key for archive diagnostics snapshots.
public struct ArchiveDiagnosticsCacheKey: Hashable, Sendable {
    public let filePath: String
    public let fileByteSize: Int64
    public let modificationTimestamp: Double
    
    public init(filePath: String, fileByteSize: Int64, modificationTimestamp: Double) {
        self.filePath = filePath
        self.fileByteSize = fileByteSize
        self.modificationTimestamp = modificationTimestamp
    }
}

// MARK: - Zip Extra Field Parser

//
//



/// Extended timestamp metadata parsed from Info-ZIP Extra Field tag `0x5455` ("UT").
public struct ExtendedTimestamp: Sendable, Equatable, Codable {
    public let modificationTime: Date?
    public let accessTime: Date?
    public let creationTime: Date?

    public var modTime: Date? { modificationTime }
    public var accTime: Date? { accessTime }
    public var createTime: Date? { creationTime }

    public init(
        modificationTime: Date? = nil,
        accessTime: Date? = nil,
        creationTime: Date? = nil
    ) {
        self.modificationTime = modificationTime
        self.accessTime = accessTime
        self.creationTime = creationTime
    }

    public init(
        modTime: Date? = nil,
        accTime: Date? = nil,
        createTime: Date? = nil
    ) {
        self.modificationTime = modTime
        self.accessTime = accTime
        self.creationTime = createTime
    }
}

/// Zip64 extended information parsed from PKWARE Extra Field tag `0x0001`.
public struct Zip64ExtraField: Sendable, Equatable, Codable {
    public let uncompressedSize: UInt64?
    public let compressedSize: UInt64?
    public let relativeOffset: UInt64?
    public let diskNumber: UInt32?

    public init(
        uncompressedSize: UInt64? = nil,
        compressedSize: UInt64? = nil,
        relativeOffset: UInt64? = nil,
        diskNumber: UInt32? = nil
    ) {
        self.uncompressedSize = uncompressedSize
        self.compressedSize = compressedSize
        self.relativeOffset = relativeOffset
        self.diskNumber = diskNumber
    }
}

/// WinZip AES encryption parameters parsed from Extra Field tag `0x9901` ("AE").
public struct WinZipAESExtraField: Sendable, Equatable, Codable {
    public enum Strength: Int, Sendable, Equatable, Codable {
        case aes128 = 128
        case aes192 = 192
        case aes256 = 256
    }

    public let version: UInt16
    public let vendorID: UInt16
    public let strength: Strength
    public let actualMethod: UInt16

    public init(
        version: UInt16 = 2,
        vendorID: UInt16 = 0x4541,
        strength: Strength,
        actualMethod: UInt16
    ) {
        self.version = version
        self.vendorID = vendorID
        self.strength = strength
        self.actualMethod = actualMethod
    }
}

/// Aggregated strongly-typed representation of all parsed standard ZIP Extra Fields.
public struct ParsedZipExtraFields: Sendable, Equatable {
    public var extendedTimestamp: ExtendedTimestamp?
    public var unicodePath: String?
    public var posixPermissions: (uid: UInt32, gid: UInt32)?
    public var zip64Info: Zip64ExtraField?
    public var winZipAES: WinZipAESExtraField?

    public init(
        extendedTimestamp: ExtendedTimestamp? = nil,
        unicodePath: String? = nil,
        posixPermissions: (uid: UInt32, gid: UInt32)? = nil,
        zip64Info: Zip64ExtraField? = nil,
        winZipAES: WinZipAESExtraField? = nil
    ) {
        self.extendedTimestamp = extendedTimestamp
        self.unicodePath = unicodePath
        self.posixPermissions = posixPermissions
        self.zip64Info = zip64Info
        self.winZipAES = winZipAES
    }

    public static func == (lhs: ParsedZipExtraFields, rhs: ParsedZipExtraFields) -> Bool {
        guard lhs.extendedTimestamp == rhs.extendedTimestamp,
              lhs.unicodePath == rhs.unicodePath,
              lhs.zip64Info == rhs.zip64Info,
              lhs.winZipAES == rhs.winZipAES else {
            return false
        }
        switch (lhs.posixPermissions, rhs.posixPermissions) {
        case (nil, nil):
            return true
        case let (l?, r?):
            return l.uid == r.uid && l.gid == r.gid
        default:
            return false
        }
    }
}

/// Deprecated compatibility stub for ZipExtraFieldParser.
/// Handwritten TLV raw-pointer parsing has been removed in favor of microkernel standards compliance checks.
@available(*, deprecated, message: "Manual TLV raw pointer parsing has been retired in favor of microkernel compliance auditing.")
public enum ZipExtraFieldParser {
    @inlinable
    public static func parse(
        extraData: UnsafeRawBufferPointer,
        standardFilename: String? = nil
    ) -> ParsedZipExtraFields {
        return ParsedZipExtraFields()
    }

    @inlinable
    public static func parse(
        extraData: Data,
        standardFilename: String? = nil
    ) -> ParsedZipExtraFields {
        return ParsedZipExtraFields()
    }

    @inlinable
    public static func parse(
        extraData: [UInt8],
        standardFilename: String? = nil
    ) -> ParsedZipExtraFields {
        return ParsedZipExtraFields()
    }
}

// MARK: - Compliance Checker

//
//


/// Comprehensive standards compliance report detailing specification adherence, validated headers, warnings, and violations.
public struct StandardsComplianceReport: Sendable, Equatable, Codable {
    public let format: ArchiveCompressionFormat
    public let isCompliant: Bool
    public let standardCitation: StandardCitation?
    public let validatedHeaders: [String]
    public let warnings: [String]
    public let violations: [String]

    public init(
        format: ArchiveCompressionFormat,
        isCompliant: Bool,
        standardCitation: StandardCitation?,
        validatedHeaders: [String],
        warnings: [String] = [],
        violations: [String] = []
    ) {
        self.format = format
        self.isCompliant = isCompliant
        self.standardCitation = standardCitation
        self.validatedHeaders = validatedHeaders
        self.warnings = warnings
        self.violations = violations
    }
}

/// Standards Compliance Checker delegating directly to high-performance Rust validation kernels.
public enum StandardsComplianceChecker {

    // MARK: - Public Validation APIs

    /// Validates compliance of a file on disk at `fileURL` against official standards.
    public static func checkCompliance(
        fileURL: URL,
        expectedFormat: ArchiveCompressionFormat? = nil
    ) throws -> StandardsComplianceReport {
        let path = fileURL.path
        guard FileManager.default.fileExists(atPath: path) else {
            let format = expectedFormat ?? .zip
            let citation = ArchiveFormatStandardRegistry.shared.spec(for: format)?.standardCitations.first
            return StandardsComplianceReport(
                format: format,
                isCompliant: false,
                standardCitation: citation,
                validatedHeaders: [],
                warnings: [],
                violations: ["File does not exist at path: \(path)"]
            )
        }

        let rawReport = try checkArchiveStandardsCompliance(archivePath: path)
        return mapUniFfiReport(rawReport, expectedFormat: expectedFormat)
    }

    /// Validates compliance of an in-memory byte buffer.
    public static func checkCompliance(
        data: Data,
        expectedFormat: ArchiveCompressionFormat? = nil
    ) throws -> StandardsComplianceReport {
        guard !data.isEmpty else {
            let format = expectedFormat ?? .zip
            let citation = ArchiveFormatStandardRegistry.shared.spec(for: format)?.standardCitations.first
            return StandardsComplianceReport(
                format: format,
                isCompliant: false,
                standardCitation: citation,
                validatedHeaders: [],
                warnings: [],
                violations: ["Data buffer is empty (0 bytes)"]
            )
        }

        return try data.withUnsafeBytes { rawBuf in
            try checkCompliance(buffer: rawBuf, fileSize: Int64(data.count), expectedFormat: expectedFormat)
        }
    }

    /// Validates compliance of an `UnsafeRawBufferPointer` by delegating directly to Rust microkernel.
    public static func checkCompliance(
        buffer: UnsafeRawBufferPointer,
        fileSize: Int64,
        expectedFormat: ArchiveCompressionFormat? = nil,
        fileURL: URL? = nil
    ) throws -> StandardsComplianceReport {
        if let fileURL = fileURL, FileManager.default.fileExists(atPath: fileURL.path) {
            return try checkCompliance(fileURL: fileURL, expectedFormat: expectedFormat)
        }

        guard !buffer.isEmpty else {
            let format = expectedFormat ?? .zip
            let citation = ArchiveFormatStandardRegistry.shared.spec(for: format)?.standardCitations.first
            return StandardsComplianceReport(
                format: format,
                isCompliant: false,
                standardCitation: citation,
                validatedHeaders: [],
                warnings: [],
                violations: ["Buffer is empty (0 bytes)"]
            )
        }

        let tempURL = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_compliance_\(UUID().uuidString).tmp")
        let data = Data(buffer)
        try data.write(to: tempURL, options: .atomic)
        defer { try? FileManager.default.removeItem(at: tempURL) }
        return try checkCompliance(fileURL: tempURL, expectedFormat: expectedFormat)
    }

    // MARK: - Native Direct String APIs

    /// Performs direct standards compliance verification returning raw JSON.
    public static func checkComplianceNative(
        buffer: UnsafeRawBufferPointer,
        expectedFormat: ArchiveCompressionFormat? = nil
    ) -> (isCompliant: Bool, reportJson: String?) {
        guard !buffer.isEmpty else { return (false, nil) }
        do {
            let report = try checkCompliance(buffer: buffer, fileSize: Int64(buffer.count), expectedFormat: expectedFormat)
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let jsonData = try encoder.encode(report)
            let jsonString = String(data: jsonData, encoding: .utf8)
            return (report.isCompliant, jsonString)
        } catch {
            return (false, nil)
        }
    }

    /// Performs direct standards compliance verification on disk returning raw JSON.
    public static func checkComplianceNative(
        fileURL: URL
    ) -> (isCompliant: Bool, reportJson: String?) {
        do {
            let report = try checkCompliance(fileURL: fileURL)
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let jsonData = try encoder.encode(report)
            let jsonString = String(data: jsonData, encoding: .utf8)
            return (report.isCompliant, jsonString)
        } catch {
            return (false, nil)
        }
    }

    // MARK: - Format & Report Mapping Helpers

    private static func mapUniFfiReport(
        _ raw: UniFfiStandardsComplianceReport,
        expectedFormat: ArchiveCompressionFormat? = nil
    ) -> StandardsComplianceReport {
        let format = mapFormatNameToCompressionFormat(raw.formatName) ?? expectedFormat ?? .zip
        let citation = ArchiveFormatStandardRegistry.shared.spec(for: format)?.standardCitations.first
            ?? (!raw.standardCitation.isEmpty ? StandardCitation(
                organization: "Standards Body",
                standardNumber: raw.standardCitation,
                title: raw.standardCitation,
                canonicalURL: ""
            ) : nil)

        return StandardsComplianceReport(
            format: format,
            isCompliant: raw.isCompliant,
            standardCitation: citation,
            validatedHeaders: raw.validatedHeaders,
            warnings: raw.warnings,
            violations: raw.violations
        )
    }

    private static func mapFormatNameToCompressionFormat(_ name: String) -> ArchiveCompressionFormat? {
        switch name.lowercased() {
        case "zip": return .zip
        case "sevenzip", "7z": return .sevenZip
        case "tar": return .tar
        case "gzip", "gz": return .gz
        case "bzip2", "bz2": return .bz2
        case "xz": return .xz
        case "zstd", "zst": return .zst
        case "rar": return .rar
        case "cab": return .cab
        case "iso": return .iso
        case "dmg": return .dmg
        case "xar": return .xar
        case "lzh": return .lzh
        case "ar": return .ar
        case "lzfse": return .lzfse
        case "snappy": return .snappy
        case "lz4": return .lz4
        case "lzip": return .lzip
        case "lrzip": return .lrzip
        case "brotli": return .brotli
        case "aar": return .aar
        case "wim": return .wim
        case "cpio": return .cpio
        case "deb": return .deb
        case "rpm": return .rpm
        case "squashfs": return .squashfs
        default: return ArchiveCompressionFormat(rawValue: name.lowercased())
        }
    }
}
