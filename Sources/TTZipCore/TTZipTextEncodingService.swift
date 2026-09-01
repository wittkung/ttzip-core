// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation

/// Strongly-typed character encoding metadata record.
public struct TTZipEncodingInfo: Identifiable, Sendable, Hashable, Codable {
    /// Unique identifier matching canonical encoding name.
    public var id: String { name }
    /// Canonical encoding identifier (e.g., "UTF-8", "GB18030", "Shift_JIS", "Big5").
    public let name: String
    /// User-facing descriptive display title with script and language region.
    public let displayName: String
    /// Standard IANA encoding label string.
    public let ianaName: String
    /// Indicates whether the encoding belongs to the Unicode standard.
    public let isUnicode: Bool
    /// Indicates whether the encoding is a CJK multibyte codepage.
    public let isCJK: Bool
    /// Indicates whether the encoding is a single-byte (8-bit) legacy codepage.
    public let isSingleByte: Bool

    public init(
        name: String,
        displayName: String,
        ianaName: String,
        isUnicode: Bool,
        isCJK: Bool,
        isSingleByte: Bool
    ) {
        self.name = name
        self.displayName = displayName
        self.ianaName = ianaName
        self.isUnicode = isUnicode
        self.isCJK = isCJK
        self.isSingleByte = isSingleByte
    }

    /// Converts UniFFI raw record to domain model.
    public init(from uniffi: UniFfiEncodingInfo) {
        self.name = uniffi.name
        self.displayName = uniffi.displayName
        self.ianaName = uniffi.ianaName
        self.isUnicode = uniffi.isUnicode
        self.isCJK = uniffi.isCjk
        self.isSingleByte = uniffi.isSingleByte
    }

    // MARK: - Well-Known Encodings

    public static let utf8 = TTZipEncodingInfo(
        name: "UTF-8",
        displayName: "UTF-8 (Unicode)",
        ianaName: "utf-8",
        isUnicode: true,
        isCJK: false,
        isSingleByte: false
    )

    public static let gb18030 = TTZipEncodingInfo(
        name: "GB18030",
        displayName: "GB18030 / GBK / GB2312 (Simplified Chinese)",
        ianaName: "gb18030",
        isUnicode: false,
        isCJK: true,
        isSingleByte: false
    )

    public static let big5 = TTZipEncodingInfo(
        name: "Big5",
        displayName: "Big5 / CP950 (Traditional Chinese)",
        ianaName: "big5",
        isUnicode: false,
        isCJK: true,
        isSingleByte: false
    )

    public static let shiftJIS = TTZipEncodingInfo(
        name: "Shift_JIS",
        displayName: "Shift-JIS / CP932 (Japanese)",
        ianaName: "shift_jis",
        isUnicode: false,
        isCJK: true,
        isSingleByte: false
    )

    public static let eucKR = TTZipEncodingInfo(
        name: "EUC-KR",
        displayName: "EUC-KR / CP949 (Korean)",
        ianaName: "euc-kr",
        isUnicode: false,
        isCJK: true,
        isSingleByte: false
    )

    public static let windows1252 = TTZipEncodingInfo(
        name: "windows-1252",
        displayName: "Windows-1252 (Western European)",
        ianaName: "windows-1252",
        isUnicode: false,
        isCJK: false,
        isSingleByte: true
    )
}

/// Sniffing result providing detected encoding name and statistical confidence.
public struct TTZipDetectedEncoding: Sendable, Hashable, Codable {
    /// Canonical detected character set name.
    public let encodingName: String
    /// Statistical confidence score bounded in [0.0..1.0].
    public let confidence: Float
    /// Whether the payload transcoded into valid UTF-8 without replacement characters.
    public let isLossless: Bool
    /// UTF-8 decoded text sample preview.
    public let samplePreview: String

    public init(
        encodingName: String,
        confidence: Float,
        isLossless: Bool,
        samplePreview: String
    ) {
        self.encodingName = encodingName
        self.confidence = confidence
        self.isLossless = isLossless
        self.samplePreview = samplePreview
    }

    /// Converts UniFFI raw record to domain model.
    public init(from uniffi: UniFfiDetectedEncoding) {
        self.encodingName = uniffi.encodingName
        self.confidence = uniffi.confidence
        self.isLossless = uniffi.isLossless
        self.samplePreview = uniffi.samplePreview
    }
}

/// Structured outcome of a filename or string remediation operation.
public struct TTZipRemediationResult: Sendable, Hashable, Codable {
    /// Original raw filename representation.
    public let originalName: String
    /// Remediated, clean UTF-8 string output.
    public let remediatedName: String
    /// Character encoding applied during remediation.
    public let encodingUsed: String
    /// Confidence score of the applied encoding [0.0..1.0].
    public let confidence: Float
    /// Whether any byte translation or transformation was performed.
    public let wasRemediated: Bool
    /// Whether unmapped bytes or replacement characters (U+FFFD) were encountered.
    public let hasUnmappedChars: Bool

    public init(
        originalName: String,
        remediatedName: String,
        encodingUsed: String,
        confidence: Float,
        wasRemediated: Bool,
        hasUnmappedChars: Bool
    ) {
        self.originalName = originalName
        self.remediatedName = remediatedName
        self.encodingUsed = encodingUsed
        self.confidence = confidence
        self.wasRemediated = wasRemediated
        self.hasUnmappedChars = hasUnmappedChars
    }

    /// Converts UniFFI raw record to domain model.
    public init(from uniffi: UniFfiRemediationResult) {
        self.originalName = uniffi.originalName
        self.remediatedName = uniffi.remediatedName
        self.encodingUsed = uniffi.encodingUsed
        self.confidence = uniffi.confidence
        self.wasRemediated = uniffi.wasRemediated
        self.hasUnmappedChars = uniffi.hasUnmappedChars
    }
}

/// Swift 6 `@Observable` and `Sendable` character encoding and filename remediation service.
///
/// Wraps the high-performance Rust charset microkernel to provide zero-copy encoding detection,
/// legacy CJK codepage remediation (GB18030, Shift-JIS, Big5, EUC-KR, Windows-1252),
/// automated mojibake repair, and batch VFS hierarchy filename sanitization.
@Observable
public final class TTZipTextEncodingService: @unchecked Sendable {

    /// Shared singleton instance.
    public static let shared = TTZipTextEncodingService()

    @ObservationIgnored
    private let engine: UniFfiTextEncodingService
    @ObservationIgnored
    private let lock = NSLock()

    // MARK: - Published Observable Metrics

    /// Total cumulative encoding detection operations performed.
    public private(set) var totalDetectionsCount: Int = 0
    /// Total cumulative filename and string remediation operations performed.
    public private(set) var totalRemediationsCount: Int = 0
    /// User-selected manual encoding override (if any).
    public private(set) var activeEncodingOverride: String? = nil
    /// Most recently detected character set name.
    public private(set) var lastDetectedEncoding: String? = nil
    /// List of all supported character encodings cached in memory.
    public private(set) var supportedEncodings: [TTZipEncodingInfo] = []

    /// Initializes service preloading supported encodings from the Rust microkernel.
    public init() {
        self.engine = UniFfiTextEncodingService()
        self.supportedEncodings = self.engine.supportedEncodings().map(TTZipEncodingInfo.init)
    }

    // MARK: - Manual Encoding Override Management

    /// Sets or clears manual encoding override for subsequent remediation tasks.
    public func setEncodingOverride(_ encodingName: String?) {
        lock.lock()
        defer { lock.unlock() }
        self.activeEncodingOverride = encodingName
    }

    /// Resets all observable telemetry metrics.
    public func resetTelemetry() {
        lock.lock()
        defer { lock.unlock() }
        self.totalDetectionsCount = 0
        self.totalRemediationsCount = 0
        self.lastDetectedEncoding = nil
    }

    // MARK: - Encoding Sniffing & Detection

    /// Detects character set encoding for given raw byte sequence synchronously.
    public func detectEncoding(data: Data) -> TTZipDetectedEncoding {
        let uniffiRes = engine.detectEncoding(data: data)
        let domainRes = TTZipDetectedEncoding(from: uniffiRes)

        lock.lock()
        totalDetectionsCount += 1
        lastDetectedEncoding = domainRes.encodingName
        lock.unlock()

        return domainRes
    }

    /// Detects character set encoding asynchronously for Swift concurrency.
    public func detectEncodingAsync(data: Data) async -> TTZipDetectedEncoding {
        return detectEncoding(data: data)
    }

    // MARK: - Transcoding Primitives

    /// Transcodes raw byte sequence to valid UTF-8 String using the specified encoding.
    public func transcodeToUTF8(data: Data, encodingName: String) throws -> String {
        do {
            return try engine.transcodeToUtf8(data: data, encodingName: encodingName)
        } catch let err as TtZipError {
            throw err
        } catch {
            throw TtZipError.IoError(message: "Transcoding failed: \(error.localizedDescription)")
        }
    }

    /// Transcodes a UTF-8 string into target legacy or Unicode byte sequence.
    public func transcodeFromUTF8(text: String, encodingName: String) throws -> Data {
        do {
            return try engine.transcodeFromUtf8(text: text, encodingName: encodingName)
        } catch let err as TtZipError {
            throw err
        } catch {
            throw TtZipError.IoError(message: "Encoding failed: \(error.localizedDescription)")
        }
    }

    // MARK: - Filename Remediation

    /// Remediates raw filename bytes into clean UTF-8 string with automatic sniffing or fallback.
    public func remediateFilename(rawBytes: Data, fallbackEncoding: String? = nil) -> TTZipRemediationResult {
        let effectiveFallback = fallbackEncoding ?? activeEncodingOverride
        let uniffiRes = engine.remediateFilename(rawBytes: rawBytes, fallbackEncoding: effectiveFallback)
        let domainRes = TTZipRemediationResult(from: uniffiRes)

        lock.lock()
        totalRemediationsCount += 1
        if domainRes.wasRemediated {
            lastDetectedEncoding = domainRes.encodingUsed
        }
        lock.unlock()

        return domainRes
    }

    /// Batch remediates a collection of raw filename byte sequences.
    public func remediateFilenamesBatch(
        items: [Data],
        fallbackEncoding: String? = nil
    ) -> [TTZipRemediationResult] {
        let effectiveFallback = fallbackEncoding ?? activeEncodingOverride
        let uniffiResults = engine.remediateFilenamesBatch(items: items, fallbackEncoding: effectiveFallback)
        let domainResults = uniffiResults.map(TTZipRemediationResult.init)

        lock.lock()
        totalRemediationsCount += items.count
        if let last = domainResults.last(where: { $0.wasRemediated }) {
            lastDetectedEncoding = last.encodingUsed
        }
        lock.unlock()

        return domainResults
    }

    /// Attempts to repair mojibake in a UTF-8 string caused by misinterpreting legacy bytes as Windows-1252/Latin-1.
    public func remediateMojibake(text: String, sourceEncoding: String? = nil) -> TTZipRemediationResult {
        let effectiveSource = sourceEncoding ?? activeEncodingOverride
        let uniffiRes = engine.remediateMojibakeUtf8(text: text, sourceEncoding: effectiveSource)
        let domainRes = TTZipRemediationResult(from: uniffiRes)

        lock.lock()
        totalRemediationsCount += 1
        if domainRes.wasRemediated {
            lastDetectedEncoding = domainRes.encodingUsed
        }
        lock.unlock()

        return domainRes
    }

    // MARK: - VFS Entry Batch Remediation

    /// Remediates a single `ArchiveEntry` by correcting its path and updating its detected encoding attribute.
    public func remediateArchiveEntry(
        entry: ArchiveEntry,
        fallbackEncoding: String? = nil
    ) -> ArchiveEntry {
        let rawData = entry.path.data(using: .isoLatin1) ?? entry.path.data(using: .utf8) ?? Data()
        let result = remediateFilename(rawBytes: rawData, fallbackEncoding: fallbackEncoding)

        guard result.wasRemediated else {
            return entry
        }

        return ArchiveEntry(
            path: result.remediatedName,
            uncompressedSize: entry.uncompressedSize,
            isDirectory: entry.isDirectory,
            detectedEncoding: result.encodingUsed,
            modificationDate: entry.modificationDate,
            isEncrypted: entry.isEncrypted,
            isDataEncrypted: entry.isDataEncrypted,
            isMetadataEncrypted: entry.isMetadataEncrypted,
            encryptionMethod: entry.encryptionMethod
        )
    }

    /// Batch remediates a list of `ArchiveEntry` items across an entire archive hierarchy.
    public func remediateArchiveEntries(
        entries: [ArchiveEntry],
        fallbackEncoding: String? = nil
    ) -> [ArchiveEntry] {
        return entries.map { remediateArchiveEntry(entry: $0, fallbackEncoding: fallbackEncoding) }
    }

    /// Remediates structured `ArchiveEntryMetadata` by correcting path and detected encoding.
    public func remediateArchiveMetadata(
        metadata: ArchiveEntryMetadata,
        fallbackEncoding: String? = nil
    ) -> ArchiveEntryMetadata {
        let rawData = metadata.path.data(using: .isoLatin1) ?? metadata.path.data(using: .utf8) ?? Data()
        let result = remediateFilename(rawBytes: rawData, fallbackEncoding: fallbackEncoding)

        guard result.wasRemediated else {
            return metadata
        }

        var updated = metadata
        updated.path = result.remediatedName
        updated.detectedEncoding = result.encodingUsed
        return updated
    }

    /// Batch remediates a list of `ArchiveEntryMetadata` records.
    public func remediateArchiveMetadataBatch(
        items: [ArchiveEntryMetadata],
        fallbackEncoding: String? = nil
    ) -> [ArchiveEntryMetadata] {
        return items.map { remediateArchiveMetadata(metadata: $0, fallbackEncoding: fallbackEncoding) }
    }
}
