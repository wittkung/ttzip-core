// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation
import os

// MARK: - Enums & Strongly-Typed Domain Models

/// Document container and format categorization.
public enum TTZipDocumentKind: String, Sendable, CaseIterable, Identifiable {
    /// Microsoft Word document (.docx).
    case docx = "DOCX"
    /// Microsoft Excel spreadsheet (.xlsx).
    case xlsx = "XLSX"
    /// Microsoft PowerPoint presentation (.pptx).
    case pptx = "PPTX"
    /// Electronic Publication (.epub).
    case epub = "EPUB"
    /// Apple XML Property List (.plist).
    case plist = "PropertyList"
    /// Portable Document Format (.pdf).
    case pdf = "PDF"
    /// Unclassified or generic XML container document.
    case unknown = "Unknown"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .docx:
            return "Word Document (DOCX)"
        case .xlsx:
            return "Excel Spreadsheet (XLSX)"
        case .pptx:
            return "PowerPoint Presentation (PPTX)"
        case .epub:
            return "EPUB Digital Book"
        case .plist:
            return "Apple Property List"
        case .pdf:
            return "PDF Document"
        case .unknown:
            return "Document"
        }
    }

    /// Infers the document kind from a file extension or format name string.
    public static func from(pathOrExtension: String) -> TTZipDocumentKind {
        let ext = (pathOrExtension as NSString).pathExtension.lowercased()
        let clean = ext.isEmpty ? pathOrExtension.lowercased() : ext
        switch clean {
        case "docx", "doc":
            return .docx
        case "xlsx", "xls":
            return .xlsx
        case "pptx", "ppt":
            return .pptx
        case "epub":
            return .epub
        case "plist":
            return .plist
        case "pdf":
            return .pdf
        default:
            if clean.contains("word") { return .docx }
            if clean.contains("sheet") || clean.contains("excel") { return .xlsx }
            if clean.contains("presentation") || clean.contains("powerpoint") { return .pptx }
            return .unknown
        }
    }
}

/// Universal document metadata descriptor providing Dublin Core and container metrics.
public struct TTZipDocumentMetadata: Sendable, Equatable, Hashable, Identifiable {
    public var id: String
    public var kind: TTZipDocumentKind
    public var formatName: String
    public var title: String?
    public var author: String?
    public var subject: String?
    public var summary: String?
    public var keywords: [String]
    public var createdDateString: String?
    public var modifiedDateString: String?
    public var lastModifiedBy: String?
    public var application: String?
    public var pageCount: Int
    public var wordCount: Int
    public var characterCount: Int
    public var slideCount: Int
    public var sheetCount: Int
    public var sheetNames: [String]
    public var slideTitles: [String]
    public var customAttributes: [String: String]

    public init(
        id: String = UUID().uuidString,
        kind: TTZipDocumentKind,
        formatName: String,
        title: String? = nil,
        author: String? = nil,
        subject: String? = nil,
        summary: String? = nil,
        keywords: [String] = [],
        createdDateString: String? = nil,
        modifiedDateString: String? = nil,
        lastModifiedBy: String? = nil,
        application: String? = nil,
        pageCount: Int = 0,
        wordCount: Int = 0,
        characterCount: Int = 0,
        slideCount: Int = 0,
        sheetCount: Int = 0,
        sheetNames: [String] = [],
        slideTitles: [String] = [],
        customAttributes: [String: String] = [:]
    ) {
        self.id = id
        self.kind = kind
        self.formatName = formatName
        self.title = title
        self.author = author
        self.subject = subject
        self.summary = summary
        self.keywords = keywords
        self.createdDateString = createdDateString
        self.modifiedDateString = modifiedDateString
        self.lastModifiedBy = lastModifiedBy
        self.application = application
        self.pageCount = pageCount
        self.wordCount = wordCount
        self.characterCount = characterCount
        self.slideCount = slideCount
        self.sheetCount = sheetCount
        self.sheetNames = sheetNames
        self.slideTitles = slideTitles
        self.customAttributes = customAttributes
    }

    internal init(from uniffi: UniFfiDocumentMetadata, sourcePath: String) {
        self.id = sourcePath
        self.kind = TTZipDocumentKind.from(pathOrExtension: uniffi.formatName)
        self.formatName = uniffi.formatName
        self.title = uniffi.title
        self.author = uniffi.author
        self.subject = uniffi.subject
        self.summary = uniffi.description
        if let kw = uniffi.keywords {
            self.keywords = kw.components(separatedBy: ",").map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }.filter { !$0.isEmpty }
        } else {
            self.keywords = []
        }
        self.createdDateString = uniffi.createdDate
        self.modifiedDateString = uniffi.modifiedDate
        self.lastModifiedBy = uniffi.lastModifiedBy
        self.application = uniffi.application
        self.pageCount = Int(uniffi.pageCount)
        self.wordCount = Int(uniffi.wordCount)
        self.characterCount = Int(uniffi.characterCount)
        self.slideCount = Int(uniffi.slideCount)
        self.sheetCount = Int(uniffi.sheetCount)
        self.sheetNames = uniffi.sheetNames
        self.slideTitles = uniffi.slideTitles
        self.customAttributes = uniffi.customProperties
    }
}

/// Structural outline of a compound Office document.
public struct TTZipDocumentOutline: Sendable, Equatable, Hashable {
    public var kind: TTZipDocumentKind
    public var documentType: String
    public var headings: [String]
    public var sheets: [String]
    public var slides: [String]
    public var totalSections: Int
    public var summaryPreview: String

    public init(
        kind: TTZipDocumentKind,
        documentType: String,
        headings: [String] = [],
        sheets: [String] = [],
        slides: [String] = [],
        totalSections: Int = 0,
        summaryPreview: String = ""
    ) {
        self.kind = kind
        self.documentType = documentType
        self.headings = headings
        self.sheets = sheets
        self.slides = slides
        self.totalSections = totalSections
        self.summaryPreview = summaryPreview
    }

    internal init(from uniffi: UniFfiOfficeOutline, kind: TTZipDocumentKind) {
        self.kind = kind
        self.documentType = uniffi.documentType
        self.headings = uniffi.headings
        self.sheets = uniffi.sheets
        self.slides = uniffi.slides
        self.totalSections = Int(uniffi.totalSections)
        self.summaryPreview = uniffi.summaryPreview
    }
}

/// Dublin Core metadata descriptor for EPUB publications.
public struct TTZipEpubPublication: Sendable, Equatable, Hashable {
    public var title: String
    public var authors: [String]
    public var publisher: String?
    public var language: String?
    public var identifier: String?
    public var synopsis: String?
    public var publicationDate: String?
    public var modifiedDate: String?
    public var rights: String?

    public init(
        title: String,
        authors: [String] = [],
        publisher: String? = nil,
        language: String? = nil,
        identifier: String? = nil,
        synopsis: String? = nil,
        publicationDate: String? = nil,
        modifiedDate: String? = nil,
        rights: String? = nil
    ) {
        self.title = title
        self.authors = authors
        self.publisher = publisher
        self.language = language
        self.identifier = identifier
        self.synopsis = synopsis
        self.publicationDate = publicationDate
        self.modifiedDate = modifiedDate
        self.rights = rights
    }

    internal init(from uniffi: UniFfiEpubMetadata) {
        self.title = uniffi.title
        self.authors = uniffi.authors
        self.publisher = uniffi.publisher
        self.language = uniffi.language
        self.identifier = uniffi.identifier
        self.synopsis = uniffi.description
        self.publicationDate = uniffi.publicationDate
        self.modifiedDate = uniffi.modifiedDate
        self.rights = uniffi.rights
    }
}

/// Apple XML Property List metadata descriptor.
public struct TTZipPlistMetadata: Sendable, Equatable, Hashable {
    public var bundleIdentifier: String?
    public var bundleName: String?
    public var bundleVersion: String?
    public var bundleShortVersion: String?
    public var minimumOSVersion: String?
    public var executableName: String?
    public var entries: [String: String]
    public var rawXML: String

    public init(
        bundleIdentifier: String? = nil,
        bundleName: String? = nil,
        bundleVersion: String? = nil,
        bundleShortVersion: String? = nil,
        minimumOSVersion: String? = nil,
        executableName: String? = nil,
        entries: [String: String] = [:],
        rawXML: String = ""
    ) {
        self.bundleIdentifier = bundleIdentifier
        self.bundleName = bundleName
        self.bundleVersion = bundleVersion
        self.bundleShortVersion = bundleShortVersion
        self.minimumOSVersion = minimumOSVersion
        self.executableName = executableName
        self.entries = entries
        self.rawXML = rawXML
    }

    internal init(from uniffi: UniFfiPlistDictionary) {
        self.bundleIdentifier = uniffi.bundleIdentifier
        self.bundleName = uniffi.bundleName
        self.bundleVersion = uniffi.bundleVersion
        self.bundleShortVersion = uniffi.bundleShortVersion
        self.minimumOSVersion = uniffi.minimumOsVersion
        self.executableName = uniffi.executableName
        self.entries = uniffi.entries
        self.rawXML = uniffi.rawXml
    }
}

// MARK: - Swift 6 Actor-Isolated Background Worker

/// Actor-isolated background worker executing UniFFI C-ABI extraction pipelines.
public actor TTZipDocumentMetadataWorker {
    private let nativeService: UniFfiXmlMetaService

    public init() {
        self.nativeService = UniFfiXmlMetaService()
    }

    /// Extracts Office document metadata at the specified POSIX filesystem path.
    public func extractOfficeMetadata(at path: String) throws -> TTZipDocumentMetadata {
        let uniffi = try nativeService.extractOfficeMetadata(filePath: path)
        return TTZipDocumentMetadata(from: uniffi, sourcePath: path)
    }

    /// Extracts Office document metadata from raw in-memory bytes.
    public func extractOfficeMetadata(from data: Data) throws -> TTZipDocumentMetadata {
        let uniffi = try nativeService.extractOfficeMetadataFromBytes(bytes: data)
        return TTZipDocumentMetadata(from: uniffi, sourcePath: "memory://buffer")
    }

    /// Extracts structural outline from an Office document on disk.
    public func extractOfficeOutline(at path: String) throws -> TTZipDocumentOutline {
        let uniffi = try nativeService.extractOfficeOutline(filePath: path)
        let kind = TTZipDocumentKind.from(pathOrExtension: path)
        return TTZipDocumentOutline(from: uniffi, kind: kind)
    }

    /// Extracts structural outline from in-memory Office document bytes.
    public func extractOfficeOutline(from data: Data, kind: TTZipDocumentKind = .unknown) throws -> TTZipDocumentOutline {
        let uniffi = try nativeService.extractOfficeOutlineFromBytes(bytes: data)
        return TTZipDocumentOutline(from: uniffi, kind: kind)
    }

    /// Extracts EPUB publication metadata from disk.
    public func extractEpubMetadata(at path: String) throws -> TTZipEpubPublication {
        let uniffi = try nativeService.extractEpubMetadata(filePath: path)
        return TTZipEpubPublication(from: uniffi)
    }

    /// Extracts EPUB publication metadata from in-memory bytes.
    public func extractEpubMetadata(from data: Data) throws -> TTZipEpubPublication {
        let uniffi = try nativeService.extractEpubMetadataFromBytes(bytes: data)
        return TTZipEpubPublication(from: uniffi)
    }

    /// Deserializes XML Property List string.
    public func parsePlist(xml: String) throws -> TTZipPlistMetadata {
        let uniffi = try nativeService.parsePlistXml(xmlContent: xml)
        return TTZipPlistMetadata(from: uniffi)
    }

    /// Deserializes XML Property List from raw bytes.
    public func parsePlist(from data: Data) throws -> TTZipPlistMetadata {
        let uniffi = try nativeService.parsePlistFromBytes(bytes: data)
        return TTZipPlistMetadata(from: uniffi)
    }
}

// MARK: - Swift 6 Observable Facade Service

/// Swift 6 `@Observable` and `Sendable` document metadata and streaming preview service.
///
/// Provides zero-extraction streaming inspection of Microsoft Office documents (DOCX, XLSX, PPTX),
/// EPUB publications, and Apple Property Lists for UI inspector panels and QuickLook preview engines.
@Observable
public final class TTZipDocumentMetadataService: @unchecked Sendable {

    // MARK: - Shared Singleton

    public static let shared = TTZipDocumentMetadataService()

    // MARK: - Published Observable Metrics

    /// Indicates whether one or more document inspection tasks are actively running.
    public private(set) var isProcessing: Bool = false

    /// Number of concurrent extraction operations currently in flight.
    public private(set) var activeOperationsCount: Int = 0

    /// Cumulative total number of documents inspected across the lifetime of this service.
    public private(set) var totalDocumentsInspected: Int = 0

    /// Most recently inspected document metadata record.
    public private(set) var lastInspectedMetadata: TTZipDocumentMetadata? = nil

    /// Most recently inspected document outline structure.
    public private(set) var lastInspectedOutline: TTZipDocumentOutline? = nil

    /// Most recent localized error encountered during document parsing.
    public private(set) var latestError: String? = nil

    // MARK: - Internal Storage & Actor Worker

    private let worker = TTZipDocumentMetadataWorker()

    private struct CacheState {
        var metadataCache: [String: TTZipDocumentMetadata] = [:]
        var outlineCache: [String: TTZipDocumentOutline] = [:]
        var activeCount: Int = 0
        var totalCount: Int = 0
    }

    private let lock = OSAllocatedUnfairLock(initialState: CacheState())

    // MARK: - Initialization

    public init() {}

    // MARK: - High-Level Inspection APIs

    /// Inspects document metadata from a local file URL with memory caching.
    ///
    /// - Parameter url: Local filesystem file URL.
    /// - Returns: Strongly-typed `TTZipDocumentMetadata` record.
    public func inspect(url: URL) async throws -> TTZipDocumentMetadata {
        let path = url.path
        if let cached = lock.withLock({ $0.metadataCache[path] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let meta = try await worker.extractOfficeMetadata(at: path)
            lock.withLock {
                $0.metadataCache[path] = meta
                $0.totalCount += 1
            }
            self.lastInspectedMetadata = meta
            self.totalDocumentsInspected += 1
            return meta
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Inspects Office document metadata directly from an in-memory byte buffer.
    ///
    /// - Parameter data: Raw bytes of the DOCX, XLSX, or PPTX file.
    /// - Returns: Strongly-typed `TTZipDocumentMetadata` record.
    public func inspect(data: Data) async throws -> TTZipDocumentMetadata {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let meta = try await worker.extractOfficeMetadata(from: data)
            self.lastInspectedMetadata = meta
            self.totalDocumentsInspected += 1
            return meta
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts structural outline, headings, sheets, or slide titles from a local file URL.
    ///
    /// - Parameter url: Local filesystem file URL.
    /// - Returns: `TTZipDocumentOutline` descriptor.
    public func outline(url: URL) async throws -> TTZipDocumentOutline {
        let path = url.path
        if let cached = lock.withLock({ $0.outlineCache[path] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.extractOfficeOutline(at: path)
            lock.withLock {
                $0.outlineCache[path] = res
            }
            self.lastInspectedOutline = res
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts structural outline from an in-memory Office document byte buffer.
    ///
    /// - Parameters:
    ///   - data: Raw bytes of the document.
    ///   - kind: Optional document kind hint.
    /// - Returns: `TTZipDocumentOutline` descriptor.
    public func outline(data: Data, kind: TTZipDocumentKind = .unknown) async throws -> TTZipDocumentOutline {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.extractOfficeOutline(from: data, kind: kind)
            self.lastInspectedOutline = res
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts Dublin Core publication metadata from an EPUB publication on disk.
    ///
    /// - Parameter url: Local filesystem file URL of the .epub file.
    /// - Returns: `TTZipEpubPublication` metadata record.
    public func inspectEpub(url: URL) async throws -> TTZipEpubPublication {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let pub = try await worker.extractEpubMetadata(at: url.path)
            self.totalDocumentsInspected += 1
            return pub
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts Dublin Core publication metadata from in-memory EPUB bytes.
    ///
    /// - Parameter data: Raw bytes of the EPUB file.
    /// - Returns: `TTZipEpubPublication` metadata record.
    public func inspectEpub(data: Data) async throws -> TTZipEpubPublication {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let pub = try await worker.extractEpubMetadata(from: data)
            self.totalDocumentsInspected += 1
            return pub
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Synchronously parses an Apple XML Property List string.
    ///
    /// - Parameter xml: Raw XML string content.
    /// - Returns: `TTZipPlistMetadata` dictionary.
    public func inspectPlist(xml: String) throws -> TTZipPlistMetadata {
        let uniffi = try uniffiParsePlistXml(xmlContent: xml)
        return TTZipPlistMetadata(from: uniffi)
    }

    /// Asynchronously parses an Apple XML Property List file on disk.
    ///
    /// - Parameter url: Local filesystem file URL of the .plist file.
    /// - Returns: `TTZipPlistMetadata` dictionary.
    public func inspectPlist(url: URL) async throws -> TTZipPlistMetadata {
        let data = try Data(contentsOf: url)
        return try await inspectPlist(data: data)
    }

    /// Asynchronously parses an Apple XML Property List from raw bytes.
    ///
    /// - Parameter data: Raw byte buffer of the plist file.
    /// - Returns: `TTZipPlistMetadata` dictionary.
    public func inspectPlist(data: Data) async throws -> TTZipPlistMetadata {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.parsePlist(from: data)
            self.totalDocumentsInspected += 1
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Clears all in-memory parsed metadata and outline caches.
    public func clearCache() {
        lock.withLock {
            $0.metadataCache.removeAll(keepingCapacity: false)
            $0.outlineCache.removeAll(keepingCapacity: false)
        }
        self.lastInspectedMetadata = nil
        self.lastInspectedOutline = nil
        self.latestError = nil
    }

    // MARK: - Private State Synchronization

    private func updateOperationCount(delta: Int) {
        let (newCount, isRunning) = lock.withLock { state -> (Int, Bool) in
            state.activeCount = max(0, state.activeCount + delta)
            return (state.activeCount, state.activeCount > 0)
        }
        self.activeOperationsCount = newCount
        self.isProcessing = isRunning
    }
}
