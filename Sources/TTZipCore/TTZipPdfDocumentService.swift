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

/// Strongly-typed PDF document metadata descriptor providing structural and publishing metrics.
public struct TTZipPdfMetadata: Sendable, Equatable, Hashable, Identifiable {
    public var id: String
    public var formatVersion: String
    public var pageCount: Int
    public var title: String?
    public var author: String?
    public var subject: String?
    public var keywords: [String]
    public var creator: String?
    public var producer: String?
    public var creationDateString: String?
    public var modificationDateString: String?
    public var isEncrypted: Bool
    public var fileSizeBytes: Int64
    public var hasOutline: Bool
    public var customProperties: [String: String]

    public init(
        id: String = UUID().uuidString,
        formatVersion: String = "PDF-1.7",
        pageCount: Int = 0,
        title: String? = nil,
        author: String? = nil,
        subject: String? = nil,
        keywords: [String] = [],
        creator: String? = nil,
        producer: String? = nil,
        creationDateString: String? = nil,
        modificationDateString: String? = nil,
        isEncrypted: Bool = false,
        fileSizeBytes: Int64 = 0,
        hasOutline: Bool = false,
        customProperties: [String: String] = [:]
    ) {
        self.id = id
        self.formatVersion = formatVersion
        self.pageCount = pageCount
        self.title = title
        self.author = author
        self.subject = subject
        self.keywords = keywords
        self.creator = creator
        self.producer = producer
        self.creationDateString = creationDateString
        self.modificationDateString = modificationDateString
        self.isEncrypted = isEncrypted
        self.fileSizeBytes = fileSizeBytes
        self.hasOutline = hasOutline
        self.customProperties = customProperties
    }

    internal init(from uniffi: UniFfiPdfMetadata, sourcePath: String) {
        self.id = sourcePath
        self.formatVersion = uniffi.formatVersion
        self.pageCount = Int(uniffi.pageCount)
        self.title = uniffi.title
        self.author = uniffi.author
        self.subject = uniffi.subject
        if let kw = uniffi.keywords {
            self.keywords = kw.components(separatedBy: CharacterSet(charactersIn: ",;"))
                .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                .filter { !$0.isEmpty }
        } else {
            self.keywords = []
        }
        self.creator = uniffi.creator
        self.producer = uniffi.producer
        self.creationDateString = uniffi.creationDate
        self.modificationDateString = uniffi.modificationDate
        self.isEncrypted = uniffi.isEncrypted
        self.fileSizeBytes = Int64(uniffi.fileSizeBytes)
        self.hasOutline = uniffi.hasOutline
        self.customProperties = uniffi.customProperties
    }
}

/// Hierarchical outline bookmark node within a PDF document.
public struct TTZipPdfOutlineNode: Sendable, Equatable, Hashable, Identifiable {
    public var id: String
    public var title: String
    public var pageNumber: Int
    public var destination: String?
    public var isExpanded: Bool
    public var children: [TTZipPdfOutlineNode]

    public var totalDescendantsCount: Int {
        children.reduce(children.count) { $0 + $1.totalDescendantsCount }
    }

    public init(
        id: String = UUID().uuidString,
        title: String,
        pageNumber: Int = 1,
        destination: String? = nil,
        isExpanded: Bool = true,
        children: [TTZipPdfOutlineNode] = []
    ) {
        self.id = id
        self.title = title
        self.pageNumber = pageNumber
        self.destination = destination
        self.isExpanded = isExpanded
        self.children = children
    }

    internal init(from uniffi: UniFfiPdfOutlineNode) {
        self.id = UUID().uuidString
        self.title = uniffi.title
        self.pageNumber = Int(uniffi.pageNumber)
        self.destination = uniffi.dest
        self.isExpanded = uniffi.isExpanded
        self.children = uniffi.children.map { TTZipPdfOutlineNode(from: $0) }
    }
}

/// Extracted text content and metrics for a specific PDF page.
public struct TTZipPdfPageText: Sendable, Equatable, Hashable, Identifiable {
    public var id: String { "\(pageNumber)" }
    public var pageNumber: Int
    public var text: String
    public var characterCount: Int
    public var wordCount: Int

    public init(
        pageNumber: Int,
        text: String,
        characterCount: Int = 0,
        wordCount: Int = 0
    ) {
        self.pageNumber = pageNumber
        self.text = text
        self.characterCount = characterCount > 0 ? characterCount : text.count
        self.wordCount = wordCount > 0 ? wordCount : text.split(whereSeparator: \.isWhitespace).count
    }

    internal init(from uniffi: UniFfiPdfPageText) {
        self.pageNumber = Int(uniffi.pageNumber)
        self.text = uniffi.text
        self.characterCount = Int(uniffi.characterCount)
        self.wordCount = Int(uniffi.wordCount)
    }
}

/// Search result entry for a full-text query match inside a PDF.
public struct TTZipPdfSearchResult: Sendable, Equatable, Hashable, Identifiable {
    public var id: String
    public var pageNumber: Int
    public var matchText: String
    public var charOffset: Int
    public var matchLength: Int

    public init(
        id: String = UUID().uuidString,
        pageNumber: Int,
        matchText: String,
        charOffset: Int,
        matchLength: Int
    ) {
        self.id = id
        self.pageNumber = pageNumber
        self.matchText = matchText
        self.charOffset = charOffset
        self.matchLength = matchLength
    }

    internal init(from uniffi: UniFfiPdfSearchResult) {
        self.id = UUID().uuidString
        self.pageNumber = Int(uniffi.pageNumber)
        self.matchText = uniffi.matchText
        self.charOffset = Int(uniffi.charOffset)
        self.matchLength = Int(uniffi.matchLength)
    }
}

// MARK: - Swift 6 Actor-Isolated Background Worker

/// Actor-isolated background worker executing UniFFI C-ABI PDF extraction and search pipelines.
public actor TTZipPdfDocumentWorker {
    private let nativeService: UniFfiPdfService

    public init() {
        self.nativeService = UniFfiPdfService()
    }

    /// Extracts PDF metadata properties at the specified POSIX filesystem path.
    public func extractMetadata(at path: String) throws -> TTZipPdfMetadata {
        let uniffi = try nativeService.extractMetadata(filePath: path)
        return TTZipPdfMetadata(from: uniffi, sourcePath: path)
    }

    /// Extracts PDF metadata properties directly from in-memory bytes.
    public func extractMetadata(from data: Data) throws -> TTZipPdfMetadata {
        let uniffi = try nativeService.extractMetadataFromBytes(data: data)
        return TTZipPdfMetadata(from: uniffi, sourcePath: "memory://pdf")
    }

    /// Extracts the full hierarchical outline bookmark tree from a PDF on disk.
    public func extractOutline(at path: String) throws -> [TTZipPdfOutlineNode] {
        let uniffiNodes = try nativeService.extractOutline(filePath: path)
        return uniffiNodes.map { TTZipPdfOutlineNode(from: $0) }
    }

    /// Extracts the full hierarchical outline bookmark tree from in-memory PDF bytes.
    public func extractOutline(from data: Data) throws -> [TTZipPdfOutlineNode] {
        let uniffiNodes = try nativeService.extractOutlineFromBytes(data: data)
        return uniffiNodes.map { TTZipPdfOutlineNode(from: $0) }
    }

    /// Extracts text content and metrics for a specific 1-based page on disk.
    public func extractPageText(at path: String, pageNumber: Int) throws -> TTZipPdfPageText {
        let uniffi = try nativeService.extractPageText(filePath: path, pageNumber: UInt32(pageNumber))
        return TTZipPdfPageText(from: uniffi)
    }

    /// Extracts text content and metrics for a specific 1-based page from in-memory bytes.
    public func extractPageText(from data: Data, pageNumber: Int) throws -> TTZipPdfPageText {
        let uniffi = try nativeService.extractPageTextFromBytes(data: data, pageNumber: UInt32(pageNumber))
        return TTZipPdfPageText(from: uniffi)
    }

    /// Extracts text for all pages (or up to `maxPages`) from a PDF file on disk.
    public func extractAllPagesText(at path: String, maxPages: Int? = nil) throws -> [TTZipPdfPageText] {
        let limit = maxPages.map { UInt32($0) }
        let uniffiList = try nativeService.extractAllPagesText(filePath: path, maxPages: limit)
        return uniffiList.map { TTZipPdfPageText(from: $0) }
    }

    /// Extracts text for all pages (or up to `maxPages`) from an in-memory PDF byte buffer.
    public func extractAllPagesText(from data: Data, maxPages: Int? = nil) throws -> [TTZipPdfPageText] {
        let limit = maxPages.map { UInt32($0) }
        let uniffiList = try nativeService.extractAllPagesTextFromBytes(data: data, maxPages: limit)
        return uniffiList.map { TTZipPdfPageText(from: $0) }
    }

    /// Performs full-text keyword search across all pages of a PDF on disk.
    public func searchText(
        at path: String,
        query: String,
        maxResults: Int = 100,
        caseSensitive: Bool = false
    ) throws -> [TTZipPdfSearchResult] {
        let uniffiResults = try nativeService.searchText(
            filePath: path,
            query: query,
            maxResults: UInt32(maxResults),
            caseSensitive: caseSensitive
        )
        return uniffiResults.map { TTZipPdfSearchResult(from: $0) }
    }

    /// Performs full-text keyword search across all pages of an in-memory PDF byte buffer.
    public func searchText(
        from data: Data,
        query: String,
        maxResults: Int = 100,
        caseSensitive: Bool = false
    ) throws -> [TTZipPdfSearchResult] {
        let uniffiResults = try nativeService.searchTextFromBytes(
            data: data,
            query: query,
            maxResults: UInt32(maxResults),
            caseSensitive: caseSensitive
        )
        return uniffiResults.map { TTZipPdfSearchResult(from: $0) }
    }
}

// MARK: - Swift 6 Observable Facade Service

/// Swift 6 `@Observable` and `Sendable` PDF document introspection, outline navigation, and text search service.
///
/// Provides zero-extraction streaming inspection of PDF documents for UI inspector panels,
/// QuickLook preview engines, and full-text keyword search within archives without disk landing.
@Observable
public final class TTZipPdfDocumentService: @unchecked Sendable {

    // MARK: - Shared Singleton

    public static let shared = TTZipPdfDocumentService()

    // MARK: - Published Observable State

    /// Indicates whether one or more PDF inspection or search tasks are actively executing.
    public private(set) var isProcessing: Bool = false

    /// Number of concurrent operations currently in flight.
    public private(set) var activeOperationsCount: Int = 0

    /// Cumulative total count of PDF documents inspected by this service.
    public private(set) var totalDocumentsInspected: Int = 0

    /// Most recently inspected PDF document metadata record.
    public private(set) var lastInspectedMetadata: TTZipPdfMetadata? = nil

    /// Most recently extracted hierarchical outline bookmark tree.
    public private(set) var lastInspectedOutline: [TTZipPdfOutlineNode] = []

    /// Search results from the most recent full-text search operation.
    public private(set) var searchResults: [TTZipPdfSearchResult] = []

    /// Most recent localized error encountered during PDF processing.
    public private(set) var latestError: String? = nil

    // MARK: - Internal Storage & Actor Worker

    private let worker = TTZipPdfDocumentWorker()

    private struct CacheState {
        var metadataCache: [String: TTZipPdfMetadata] = [:]
        var outlineCache: [String: [TTZipPdfOutlineNode]] = [:]
        var activeCount: Int = 0
        var totalCount: Int = 0
    }

    private let lock = OSAllocatedUnfairLock(initialState: CacheState())

    // MARK: - Initialization

    public init() {}

    // MARK: - Public Inspection APIs

    /// Inspects PDF metadata from a local filesystem file URL with in-memory caching.
    ///
    /// - Parameter url: Local filesystem URL pointing to a PDF file.
    /// - Returns: Strongly-typed `TTZipPdfMetadata` record.
    public func inspect(url: URL) async throws -> TTZipPdfMetadata {
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
            self.totalDocumentsInspected += 1
            return meta
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Inspects PDF metadata directly from an in-memory byte buffer.
    ///
    /// - Parameter data: Raw bytes of the PDF file.
    /// - Returns: Strongly-typed `TTZipPdfMetadata` record.
    public func inspect(data: Data) async throws -> TTZipPdfMetadata {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let meta = try await worker.extractMetadata(from: data)
            self.lastInspectedMetadata = meta
            self.totalDocumentsInspected += 1
            return meta
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts the complete hierarchical outline bookmark tree from a local PDF file URL.
    ///
    /// - Parameter url: Local filesystem URL pointing to a PDF file.
    /// - Returns: Array of root `TTZipPdfOutlineNode` items.
    public func outline(url: URL) async throws -> [TTZipPdfOutlineNode] {
        let path = url.path
        if let cached = lock.withLock({ $0.outlineCache[path] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.extractOutline(at: path)
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

    /// Extracts the hierarchical outline bookmark tree from an in-memory PDF byte buffer.
    ///
    /// - Parameter data: Raw bytes of the PDF file.
    /// - Returns: Array of root `TTZipPdfOutlineNode` items.
    public func outline(data: Data) async throws -> [TTZipPdfOutlineNode] {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.extractOutline(from: data)
            self.lastInspectedOutline = res
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts plain text from a specific 1-based page of a local PDF file.
    ///
    /// - Parameters:
    ///   - url: Local filesystem URL.
    ///   - pageNumber: 1-based page index.
    /// - Returns: `TTZipPdfPageText` descriptor.
    public func pageText(url: URL, pageNumber: Int) async throws -> TTZipPdfPageText {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.extractPageText(at: url.path, pageNumber: pageNumber)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts plain text from a specific 1-based page of an in-memory PDF buffer.
    ///
    /// - Parameters:
    ///   - data: Raw bytes of the PDF file.
    ///   - pageNumber: 1-based page index.
    /// - Returns: `TTZipPdfPageText` descriptor.
    public func pageText(data: Data, pageNumber: Int) async throws -> TTZipPdfPageText {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.extractPageText(from: data, pageNumber: pageNumber)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts text for all pages in a document up to `maxPages`.
    ///
    /// - Parameters:
    ///   - url: Local filesystem URL.
    ///   - maxPages: Optional page ceiling limit.
    /// - Returns: Array of `TTZipPdfPageText` records.
    public func allPagesText(url: URL, maxPages: Int? = nil) async throws -> [TTZipPdfPageText] {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.extractAllPagesText(at: url.path, maxPages: maxPages)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts text for all pages in an in-memory PDF buffer up to `maxPages`.
    ///
    /// - Parameters:
    ///   - data: Raw bytes of the PDF file.
    ///   - maxPages: Optional page ceiling limit.
    /// - Returns: Array of `TTZipPdfPageText` records.
    public func allPagesText(data: Data, maxPages: Int? = nil) async throws -> [TTZipPdfPageText] {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.extractAllPagesText(from: data, maxPages: maxPages)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Searches for query occurrences across all pages of a PDF on disk.
    ///
    /// - Parameters:
    ///   - url: Local filesystem URL.
    ///   - query: Keyword search query string.
    ///   - maxResults: Maximum match count limit.
    ///   - caseSensitive: Whether match is case sensitive.
    /// - Returns: Array of `TTZipPdfSearchResult` records.
    public func search(
        url: URL,
        query: String,
        maxResults: Int = 100,
        caseSensitive: Bool = false
    ) async throws -> [TTZipPdfSearchResult] {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.searchText(
                at: url.path,
                query: query,
                maxResults: maxResults,
                caseSensitive: caseSensitive
            )
            self.searchResults = res
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Searches for query occurrences across all pages of an in-memory PDF byte buffer.
    ///
    /// - Parameters:
    ///   - data: Raw bytes of the PDF file.
    ///   - query: Keyword search query string.
    ///   - maxResults: Maximum match count limit.
    ///   - caseSensitive: Whether match is case sensitive.
    /// - Returns: Array of `TTZipPdfSearchResult` records.
    public func search(
        data: Data,
        query: String,
        maxResults: Int = 100,
        caseSensitive: Bool = false
    ) async throws -> [TTZipPdfSearchResult] {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.searchText(
                from: data,
                query: query,
                maxResults: maxResults,
                caseSensitive: caseSensitive
            )
            self.searchResults = res
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Clears all in-memory metadata and outline caches.
    public func clearCache() {
        lock.withLock {
            $0.metadataCache.removeAll(keepingCapacity: false)
            $0.outlineCache.removeAll(keepingCapacity: false)
        }
        self.lastInspectedMetadata = nil
        self.lastInspectedOutline = []
        self.searchResults = []
        self.latestError = nil
    }

    /// Clears the current active search results.
    public func clearSearchResults() {
        self.searchResults = []
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
