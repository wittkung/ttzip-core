// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation
import os

// MARK: - Strongly-Typed Domain Models

/// Supported ebook format enumeration.
public enum TTZipEbookFormat: String, Sendable, Codable, CaseIterable, Equatable, Hashable {
    case unknown
    case epub
    case cbz
    case fb2
    case mobi
    case azw3
    case pdf

    internal init(from uniffi: UniFfiEbookFormat) {
        switch uniffi {
        case .unknown: self = .unknown
        case .epub: self = .epub
        case .cbz: self = .cbz
        case .fb2: self = .fb2
        case .mobi: self = .mobi
        case .azw3: self = .azw3
        case .pdf: self = .pdf
        }
    }
}

/// Strongly-typed publication metadata descriptor providing publishing and structural metrics.
public struct TTZipEbookMetadata: Sendable, Equatable, Hashable, Identifiable {
    public var id: String
    public var title: String
    public var authors: [String]
    public var publisher: String?
    public var language: String?
    public var identifier: String?
    public var descriptionText: String?
    public var publicationDateString: String?
    public var modificationDateString: String?
    public var rights: String?
    public var format: TTZipEbookFormat
    public var totalChapters: Int
    public var totalResources: Int
    public var fileSizeBytes: Int64
    public var hasCover: Bool
    public var coverPath: String?
    public var extraMetadata: [String: String]

    public init(
        id: String = UUID().uuidString,
        title: String,
        authors: [String] = [],
        publisher: String? = nil,
        language: String? = nil,
        identifier: String? = nil,
        descriptionText: String? = nil,
        publicationDateString: String? = nil,
        modificationDateString: String? = nil,
        rights: String? = nil,
        format: TTZipEbookFormat = .unknown,
        totalChapters: Int = 0,
        totalResources: Int = 0,
        fileSizeBytes: Int64 = 0,
        hasCover: Bool = false,
        coverPath: String? = nil,
        extraMetadata: [String: String] = [:]
    ) {
        self.id = id
        self.title = title
        self.authors = authors
        self.publisher = publisher
        self.language = language
        self.identifier = identifier
        self.descriptionText = descriptionText
        self.publicationDateString = publicationDateString
        self.modificationDateString = modificationDateString
        self.rights = rights
        self.format = format
        self.totalChapters = totalChapters
        self.totalResources = totalResources
        self.fileSizeBytes = fileSizeBytes
        self.hasCover = hasCover
        self.coverPath = coverPath
        self.extraMetadata = extraMetadata
    }

    internal init(from uniffi: UniFfiEbookMetadata, sourceId: String) {
        self.id = sourceId
        self.title = uniffi.title
        self.authors = uniffi.authors
        self.publisher = uniffi.publisher
        self.language = uniffi.language
        self.identifier = uniffi.identifier
        self.descriptionText = uniffi.description
        self.publicationDateString = uniffi.publicationDate
        self.modificationDateString = uniffi.modifiedDate
        self.rights = uniffi.rights
        self.format = TTZipEbookFormat(from: uniffi.format)
        self.totalChapters = Int(uniffi.totalChapters)
        self.totalResources = Int(uniffi.totalResources)
        self.fileSizeBytes = Int64(uniffi.fileSizeBytes)
        self.hasCover = uniffi.hasCover
        self.coverPath = uniffi.coverPath
        self.extraMetadata = uniffi.extraMetadata
    }
}

/// Hierarchical Table of Contents (TOC) node within an ebook document.
public struct TTZipEbookTocNode: Sendable, Equatable, Hashable, Identifiable {
    public var id: String
    public var title: String
    public var href: String
    public var playOrder: Int
    public var level: Int
    public var isExpanded: Bool
    public var children: [TTZipEbookTocNode]

    public var totalDescendantsCount: Int {
        children.reduce(children.count) { $0 + $1.totalDescendantsCount }
    }

    public init(
        id: String = UUID().uuidString,
        title: String,
        href: String,
        playOrder: Int = 1,
        level: Int = 0,
        isExpanded: Bool = true,
        children: [TTZipEbookTocNode] = []
    ) {
        self.id = id
        self.title = title
        self.href = href
        self.playOrder = playOrder
        self.level = level
        self.isExpanded = isExpanded
        self.children = children
    }

    internal init(from uniffi: UniFfiEbookTocNode) {
        self.id = uniffi.id.isEmpty ? UUID().uuidString : uniffi.id
        self.title = uniffi.title
        self.href = uniffi.href
        self.playOrder = Int(uniffi.playOrder)
        self.level = Int(uniffi.level)
        self.isExpanded = true
        self.children = uniffi.children.map { TTZipEbookTocNode(from: $0) }
    }
}

/// Sequential item in the publication reading spine.
public struct TTZipEbookSpineItem: Sendable, Equatable, Hashable, Identifiable {
    public var id: String
    public var href: String
    public var mediaType: String
    public var playOrder: Int
    public var isLinear: Bool

    public init(
        id: String,
        href: String,
        mediaType: String = "application/xhtml+xml",
        playOrder: Int = 1,
        isLinear: Bool = true
    ) {
        self.id = id
        self.href = href
        self.mediaType = mediaType
        self.playOrder = playOrder
        self.isLinear = isLinear
    }

    internal init(from uniffi: UniFfiEbookSpineItem) {
        self.id = uniffi.id
        self.href = uniffi.href
        self.mediaType = uniffi.mediaType
        self.playOrder = Int(uniffi.playOrder)
        self.isLinear = uniffi.isLinear
    }
}

/// Parsed chapter document with XHTML markup and text statistics.
public struct TTZipEbookChapter: Sendable, Equatable, Hashable, Identifiable {
    public var id: String
    public var title: String
    public var href: String
    public var mediaType: String
    public var playOrder: Int
    public var contentString: String
    public var characterCount: Int
    public var wordCount: Int

    public init(
        id: String,
        title: String,
        href: String,
        mediaType: String = "application/xhtml+xml",
        playOrder: Int = 1,
        contentString: String,
        characterCount: Int = 0,
        wordCount: Int = 0
    ) {
        self.id = id
        self.title = title
        self.href = href
        self.mediaType = mediaType
        self.playOrder = playOrder
        self.contentString = contentString
        self.characterCount = characterCount > 0 ? characterCount : contentString.count
        self.wordCount = wordCount > 0 ? wordCount : contentString.split(whereSeparator: \.isWhitespace).count
    }

    internal init(from uniffi: UniFfiEbookChapter) {
        self.id = uniffi.id
        self.title = uniffi.title
        self.href = uniffi.href
        self.mediaType = uniffi.mediaType
        self.playOrder = Int(uniffi.playOrder)
        self.contentString = uniffi.contentString
        self.characterCount = Int(uniffi.characterCount)
        self.wordCount = Int(uniffi.wordCount)
    }
}

/// Embedded binary resource (image, stylesheet, font) extracted from ebook archive.
public struct TTZipEbookResource: Sendable, Equatable, Hashable, Identifiable {
    public var id: String { href }
    public var href: String
    public var mediaType: String
    public var data: Data
    public var sizeBytes: Int64

    public init(
        href: String,
        mediaType: String,
        data: Data
    ) {
        self.href = href
        self.mediaType = mediaType
        self.data = data
        self.sizeBytes = Int64(data.count)
    }

    internal init(from uniffi: UniFfiEbookResource) {
        self.href = uniffi.href
        self.mediaType = uniffi.mediaType
        self.data = uniffi.data
        self.sizeBytes = Int64(uniffi.sizeBytes)
    }
}

// MARK: - Swift 6 Actor-Isolated Background Worker

/// Actor-isolated background worker executing UniFFI C-ABI Ebook parsing and streaming pipelines.
public actor TTZipEbookReaderWorker {
    private let nativeService: UniFfiEbookService

    public init() {
        self.nativeService = UniFfiEbookService()
    }

    /// Probes the ebook format at the specified filesystem path.
    public func probe(at path: String) throws -> TTZipEbookFormat {
        let uniffi = try nativeService.probeFile(filePath: path)
        return TTZipEbookFormat(from: uniffi)
    }

    /// Probes the ebook format directly from in-memory bytes.
    public func probe(from data: Data, fileName: String? = nil) throws -> TTZipEbookFormat {
        let uniffi = try nativeService.probeBytes(data: data, fileName: fileName)
        return TTZipEbookFormat(from: uniffi)
    }

    /// Extracts publication metadata at the specified filesystem path.
    public func extractMetadata(at path: String) throws -> TTZipEbookMetadata {
        let uniffi = try nativeService.extractMetadataFromFile(filePath: path)
        return TTZipEbookMetadata(from: uniffi, sourceId: path)
    }

    /// Extracts publication metadata directly from in-memory bytes.
    public func extractMetadata(from data: Data, fileName: String? = nil) throws -> TTZipEbookMetadata {
        let uniffi = try nativeService.extractMetadata(data: data, fileName: fileName)
        let sourceId = fileName ?? "memory://ebook"
        return TTZipEbookMetadata(from: uniffi, sourceId: sourceId)
    }

    /// Extracts hierarchical Table of Contents (TOC) at the specified filesystem path.
    public func extractToc(at path: String) throws -> [TTZipEbookTocNode] {
        let uniffiList = try nativeService.extractTocFromFile(filePath: path)
        return uniffiList.map { TTZipEbookTocNode(from: $0) }
    }

    /// Extracts hierarchical Table of Contents (TOC) from in-memory bytes.
    public func extractToc(from data: Data, fileName: String? = nil) throws -> [TTZipEbookTocNode] {
        let uniffiList = try nativeService.extractToc(data: data, fileName: fileName)
        return uniffiList.map { TTZipEbookTocNode(from: $0) }
    }

    /// Retrieves ordered reading spine items at the specified filesystem path.
    public func getSpine(at path: String) throws -> [TTZipEbookSpineItem] {
        let uniffiList = try nativeService.getSpineFromFile(filePath: path)
        return uniffiList.map { TTZipEbookSpineItem(from: $0) }
    }

    /// Retrieves ordered reading spine items from in-memory bytes.
    public func getSpine(from data: Data, fileName: String? = nil) throws -> [TTZipEbookSpineItem] {
        let uniffiList = try nativeService.getSpine(data: data, fileName: fileName)
        return uniffiList.map { TTZipEbookSpineItem(from: $0) }
    }

    /// Extracts a single chapter document at the specified filesystem path.
    public func extractChapter(at path: String, href: String) throws -> TTZipEbookChapter {
        let uniffi = try nativeService.extractChapterFromFile(filePath: path, href: href)
        return TTZipEbookChapter(from: uniffi)
    }

    /// Extracts a single chapter document from in-memory bytes.
    public func extractChapter(from data: Data, href: String, fileName: String? = nil) throws -> TTZipEbookChapter {
        let uniffi = try nativeService.extractChapter(data: data, href: href, fileName: fileName)
        return TTZipEbookChapter(from: uniffi)
    }

    /// Extracts an embedded asset resource at the specified filesystem path.
    public func extractResource(at path: String, href: String) throws -> TTZipEbookResource {
        let uniffi = try nativeService.extractResourceFromFile(filePath: path, href: href)
        return TTZipEbookResource(from: uniffi)
    }

    /// Extracts an embedded asset resource from in-memory bytes.
    public func extractResource(from data: Data, href: String, fileName: String? = nil) throws -> TTZipEbookResource {
        let uniffi = try nativeService.extractResource(data: data, href: href, fileName: fileName)
        return TTZipEbookResource(from: uniffi)
    }

    /// Extracts cover artwork at the specified filesystem path.
    public func extractCover(at path: String) throws -> TTZipEbookResource? {
        guard let uniffi = try nativeService.extractCoverFromFile(filePath: path) else {
            return nil
        }
        return TTZipEbookResource(from: uniffi)
    }

    /// Extracts cover artwork from in-memory bytes.
    public func extractCover(from data: Data, fileName: String? = nil) throws -> TTZipEbookResource? {
        guard let uniffi = try nativeService.extractCover(data: data, fileName: fileName) else {
            return nil
        }
        return TTZipEbookResource(from: uniffi)
    }
}

// MARK: - Swift 6 Observable Facade Service

/// Swift 6 `@Observable` and `Sendable` Ebook reader and metadata inspection service.
///
/// Provides zero-extraction streaming introspection of EPUB, CBZ, and ebook publications
/// for UI reader panels, QuickLook preview generators, and document indexers without disk landing.
@Observable
public final class TTZipEbookReaderService: @unchecked Sendable {

    // MARK: - Shared Singleton

    public static let shared = TTZipEbookReaderService()

    // MARK: - Published Observable State

    /// Indicates whether one or more ebook inspection or reading operations are in flight.
    public private(set) var isProcessing: Bool = false

    /// Number of concurrent operations currently executing.
    public private(set) var activeOperationsCount: Int = 0

    /// Cumulative total count of ebook files inspected by this service instance.
    public private(set) var totalBooksInspected: Int = 0

    /// Most recently inspected ebook publication metadata record.
    public private(set) var lastInspectedMetadata: TTZipEbookMetadata? = nil

    /// Most recently extracted hierarchical Table of Contents (TOC).
    public private(set) var lastInspectedToc: [TTZipEbookTocNode] = []

    /// Most recently extracted reading spine items.
    public private(set) var lastInspectedSpine: [TTZipEbookSpineItem] = []

    /// Most recently extracted chapter document.
    public private(set) var lastExtractedChapter: TTZipEbookChapter? = nil

    /// Most recent localized error encountered during ebook processing.
    public private(set) var latestError: String? = nil

    // MARK: - Internal Storage & Actor Worker

    private let worker = TTZipEbookReaderWorker()

    private struct CacheState {
        var metadataCache: [String: TTZipEbookMetadata] = [:]
        var tocCache: [String: [TTZipEbookTocNode]] = [:]
        var spineCache: [String: [TTZipEbookSpineItem]] = [:]
        var activeCount: Int = 0
        var totalCount: Int = 0
    }

    private let lock = OSAllocatedUnfairLock(initialState: CacheState())

    // MARK: - Initialization

    public init() {}

    // MARK: - Public Inspection APIs

    /// Probes the format of an ebook file at a filesystem URL.
    public func probe(url: URL) async throws -> TTZipEbookFormat {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.probe(at: url.path)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Probes the format of an in-memory ebook byte buffer.
    public func probe(data: Data, fileName: String? = nil) async throws -> TTZipEbookFormat {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.probe(from: data, fileName: fileName)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Inspects publication metadata with in-memory caching.
    public func inspect(url: URL) async throws -> TTZipEbookMetadata {
        let path = url.path
        if let cached = lock.withLock({ $0.metadataCache[path] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let meta = try await worker.extractMetadata(at: path)
            lock.withLock {
                $0.metadataCache[path] = meta
                $0.totalCount += 1
            }
            self.lastInspectedMetadata = meta
            self.totalBooksInspected += 1
            return meta
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Inspects publication metadata directly from an in-memory byte buffer.
    public func inspect(data: Data, fileName: String? = nil) async throws -> TTZipEbookMetadata {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let meta = try await worker.extractMetadata(from: data, fileName: fileName)
            self.lastInspectedMetadata = meta
            self.totalBooksInspected += 1
            return meta
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts the complete hierarchical Table of Contents (TOC) with in-memory caching.
    public func tableOfContents(url: URL) async throws -> [TTZipEbookTocNode] {
        let path = url.path
        if let cached = lock.withLock({ $0.tocCache[path] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.extractToc(at: path)
            lock.withLock {
                $0.tocCache[path] = res
            }
            self.lastInspectedToc = res
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts the complete hierarchical Table of Contents (TOC) from an in-memory byte buffer.
    public func tableOfContents(data: Data, fileName: String? = nil) async throws -> [TTZipEbookTocNode] {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.extractToc(from: data, fileName: fileName)
            self.lastInspectedToc = res
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Retrieves ordered reading spine items with in-memory caching.
    public func spine(url: URL) async throws -> [TTZipEbookSpineItem] {
        let path = url.path
        if let cached = lock.withLock({ $0.spineCache[path] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.getSpine(at: path)
            lock.withLock {
                $0.spineCache[path] = res
            }
            self.lastInspectedSpine = res
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Retrieves ordered reading spine items from an in-memory byte buffer.
    public func spine(data: Data, fileName: String? = nil) async throws -> [TTZipEbookSpineItem] {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.getSpine(from: data, fileName: fileName)
            self.lastInspectedSpine = res
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts a chapter document at a filesystem URL by relative href path.
    public func chapter(url: URL, href: String) async throws -> TTZipEbookChapter {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.extractChapter(at: url.path, href: href)
            self.lastExtractedChapter = res
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts a chapter document from an in-memory byte buffer by relative href path.
    public func chapter(data: Data, href: String, fileName: String? = nil) async throws -> TTZipEbookChapter {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.extractChapter(from: data, href: href, fileName: fileName)
            self.lastExtractedChapter = res
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts an embedded asset resource at a filesystem URL.
    public func resource(url: URL, href: String) async throws -> TTZipEbookResource {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.extractResource(at: url.path, href: href)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts an embedded asset resource from an in-memory byte buffer.
    public func resource(data: Data, href: String, fileName: String? = nil) async throws -> TTZipEbookResource {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.extractResource(from: data, href: href, fileName: fileName)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts cover artwork from a filesystem URL.
    public func cover(url: URL) async throws -> TTZipEbookResource? {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.extractCover(at: url.path)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts cover artwork from an in-memory byte buffer.
    public func cover(data: Data, fileName: String? = nil) async throws -> TTZipEbookResource? {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.extractCover(from: data, fileName: fileName)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Clears all in-memory metadata, TOC, and spine caches.
    public func clearCache() {
        lock.withLock {
            $0.metadataCache.removeAll(keepingCapacity: false)
            $0.tocCache.removeAll(keepingCapacity: false)
            $0.spineCache.removeAll(keepingCapacity: false)
        }
        self.lastInspectedMetadata = nil
        self.lastInspectedToc = []
        self.lastInspectedSpine = []
        self.lastExtractedChapter = nil
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
