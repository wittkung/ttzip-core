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

/// Supported Office Open XML and OpenDocument format enumeration.
public enum TTZipOfficeFormat: String, Sendable, Codable, CaseIterable, Equatable, Hashable {
    case unknown
    case docx
    case xlsx
    case pptx
    case odt
    case ods
    case odp

    internal init(from uniffi: UniFfiOfficeFormat) {
        switch uniffi {
        case .unknown: self = .unknown
        case .docx: self = .docx
        case .xlsx: self = .xlsx
        case .pptx: self = .pptx
        case .odt: self = .odt
        case .ods: self = .ods
        case .odp: self = .odp
        }
    }
}

/// Strongly-typed spreadsheet cell value representation.
public enum TTZipCellValue: Sendable, Codable, Equatable, Hashable {
    case empty
    case text(String)
    case number(Double)
    case boolean(Bool)
    case formula(expression: String, cachedValue: String?)
    case error(String)

    public var displayString: String {
        switch self {
        case .empty:
            return ""
        case .text(let str):
            return str
        case .number(let num):
            if num.rounded() == num && !num.isInfinite && !num.isNaN {
                return String(format: "%.0f", num)
            } else {
                return String(num)
            }
        case .boolean(let b):
            return b ? "TRUE" : "FALSE"
        case .formula(let expr, let cached):
            return cached ?? "=\(expr)"
        case .error(let msg):
            return "#\(msg)"
        }
    }

    public var asNumber: Double? {
        switch self {
        case .number(let n):
            return n
        case .text(let s):
            return Double(s.trimmingCharacters(in: .whitespacesAndNewlines))
        case .boolean(let b):
            return b ? 1.0 : 0.0
        case .formula(_, let cached):
            return cached.flatMap { Double($0.trimmingCharacters(in: .whitespacesAndNewlines)) }
        default:
            return nil
        }
    }

    public var asBool: Bool? {
        switch self {
        case .boolean(let b):
            return b
        case .number(let n):
            return n != 0.0
        case .text(let s):
            let lower = s.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if lower == "true" || lower == "1" { return true }
            if lower == "false" || lower == "0" { return false }
            return nil
        default:
            return nil
        }
    }

    internal init(from uniffi: UniFfiCellValue) {
        switch uniffi {
        case .empty:
            self = .empty
        case .text(let value):
            self = .text(value)
        case .number(let value):
            self = .number(value)
        case .boolean(let value):
            self = .boolean(value)
        case .formula(let expression, let cachedValue):
            self = .formula(expression: expression, cachedValue: cachedValue)
        case .error(let message):
            self = .error(message)
        }
    }

    internal func toUniffi() -> UniFfiCellValue {
        switch self {
        case .empty:
            return .empty
        case .text(let val):
            return .text(value: val)
        case .number(let val):
            return .number(value: val)
        case .boolean(let val):
            return .boolean(value: val)
        case .formula(let expr, let cached):
            return .formula(expression: expr, cachedValue: cached)
        case .error(let msg):
            return .error(message: msg)
        }
    }
}

/// Strongly-typed single spreadsheet cell record with coordinates.
public struct TTZipCell: Sendable, Codable, Equatable, Hashable, Identifiable {
    public var id: String { coordinate }
    public var row: Int
    public var col: Int
    public var coordinate: String
    public var value: TTZipCellValue
    public var formula: String?
    public var displayString: String { value.displayString }

    public init(
        row: Int,
        col: Int,
        coordinate: String,
        value: TTZipCellValue,
        formula: String? = nil
    ) {
        self.row = row
        self.col = col
        self.coordinate = coordinate
        self.value = value
        self.formula = formula
    }

    internal init(from uniffi: UniFfiCell) {
        self.row = Int(uniffi.row)
        self.col = Int(uniffi.col)
        self.coordinate = uniffi.coordinate
        self.value = TTZipCellValue(from: uniffi.value)
        self.formula = uniffi.formula
    }

    internal func toUniffi() -> UniFfiCell {
        UniFfiCell(
            row: UInt32(row),
            col: UInt32(col),
            coordinate: coordinate,
            value: value.toUniffi(),
            formula: formula
        )
    }
}

/// Sequential row in a spreadsheet worksheet grid.
public struct TTZipSheetRow: Sendable, Codable, Equatable, Hashable, Identifiable {
    public var id: Int { rowNumber }
    public var rowNumber: Int
    public var cells: [TTZipCell]

    public init(rowNumber: Int, cells: [TTZipCell] = []) {
        self.rowNumber = rowNumber
        self.cells = cells
    }

    internal init(from uniffi: UniFfiSheetRow) {
        self.rowNumber = Int(uniffi.rowNumber)
        self.cells = uniffi.cells.map { TTZipCell(from: $0) }
    }
}

/// Extracted worksheet grid data structure with metrics.
public struct TTZipSheetData: Sendable, Codable, Equatable, Hashable, Identifiable {
    public var id: String { "\(sheetIndex)_\(sheetName)" }
    public var sheetName: String
    public var sheetIndex: Int
    public var totalRows: Int
    public var totalCols: Int
    public var dimensionRef: String?
    public var rows: [TTZipSheetRow]
    public var sharedStringsCount: Int

    public init(
        sheetName: String,
        sheetIndex: Int = 1,
        totalRows: Int = 0,
        totalCols: Int = 0,
        dimensionRef: String? = nil,
        rows: [TTZipSheetRow] = [],
        sharedStringsCount: Int = 0
    ) {
        self.sheetName = sheetName
        self.sheetIndex = sheetIndex
        self.totalRows = totalRows
        self.totalCols = totalCols
        self.dimensionRef = dimensionRef
        self.rows = rows
        self.sharedStringsCount = sharedStringsCount
    }

    internal init(from uniffi: UniFfiSheetData) {
        self.sheetName = uniffi.sheetName
        self.sheetIndex = Int(uniffi.sheetIndex)
        self.totalRows = Int(uniffi.totalRows)
        self.totalCols = Int(uniffi.totalCols)
        self.dimensionRef = uniffi.dimensionRef
        self.rows = uniffi.rows.map { TTZipSheetRow(from: $0) }
        self.sharedStringsCount = Int(uniffi.sharedStringsCount)
    }
}

/// A structured paragraph inside a DOCX document.
public struct TTZipDocxParagraph: Sendable, Codable, Equatable, Hashable, Identifiable {
    public var id: String
    public var style: String
    public var text: String
    public var headingLevel: Int?
    public var isListItem: Bool
    public var listLevel: Int?

    public init(
        id: String = UUID().uuidString,
        style: String = "Normal",
        text: String,
        headingLevel: Int? = nil,
        isListItem: Bool = false,
        listLevel: Int? = nil
    ) {
        self.id = id
        self.style = style
        self.text = text
        self.headingLevel = headingLevel
        self.isListItem = isListItem
        self.listLevel = listLevel
    }

    internal init(from uniffi: UniFfiDocxParagraph) {
        self.id = UUID().uuidString
        self.style = uniffi.style
        self.text = uniffi.text
        self.headingLevel = uniffi.headingLevel.map { Int($0) }
        self.isListItem = uniffi.isListItem
        self.listLevel = uniffi.listLevel.map { Int($0) }
    }
}

/// A structured table extracted from a DOCX document.
public struct TTZipDocxTable: Sendable, Codable, Equatable, Hashable, Identifiable {
    public var id: String
    public var totalRows: Int
    public var totalCols: Int
    public var headers: [String]
    public var rows: [[String]]

    public init(
        id: String = UUID().uuidString,
        totalRows: Int = 0,
        totalCols: Int = 0,
        headers: [String] = [],
        rows: [[String]] = []
    ) {
        self.id = id
        self.totalRows = totalRows
        self.totalCols = totalCols
        self.headers = headers
        self.rows = rows
    }

    internal init(from uniffi: UniFfiDocxTable) {
        self.id = UUID().uuidString
        self.totalRows = Int(uniffi.totalRows)
        self.totalCols = Int(uniffi.totalCols)
        self.headers = uniffi.headers
        self.rows = uniffi.rows.map { $0.cells }
    }
}

/// Comprehensive DOCX structured document representation.
public struct TTZipDocxDocument: Sendable, Codable, Equatable, Hashable, Identifiable {
    public var id: String
    public var title: String?
    public var paragraphs: [TTZipDocxParagraph]
    public var tables: [TTZipDocxTable]
    public var totalWords: Int
    public var totalCharacters: Int
    public var markdownContent: String

    public init(
        id: String = UUID().uuidString,
        title: String? = nil,
        paragraphs: [TTZipDocxParagraph] = [],
        tables: [TTZipDocxTable] = [],
        totalWords: Int = 0,
        totalCharacters: Int = 0,
        markdownContent: String = ""
    ) {
        self.id = id
        self.title = title
        self.paragraphs = paragraphs
        self.tables = tables
        self.totalWords = totalWords
        self.totalCharacters = totalCharacters
        self.markdownContent = markdownContent
    }

    internal init(from uniffi: UniFfiDocxDocument, sourceId: String) {
        self.id = sourceId
        self.title = uniffi.title
        self.paragraphs = uniffi.paragraphs.map { TTZipDocxParagraph(from: $0) }
        self.tables = uniffi.tables.map { TTZipDocxTable(from: $0) }
        self.totalWords = Int(uniffi.totalWords)
        self.totalCharacters = Int(uniffi.totalCharacters)
        self.markdownContent = uniffi.markdownContent
    }
}

// MARK: - Swift 6 Actor-Isolated Background Worker

/// Actor-isolated background worker executing UniFFI C-ABI Office parsing and streaming pipelines.
public actor TTZipOfficeDocumentWorker {
    private let nativeService: UniFfiOfficeService

    public init() {
        self.nativeService = UniFfiOfficeService()
    }

    /// Probes the Office format at the specified filesystem path.
    public func probe(at path: String) throws -> TTZipOfficeFormat {
        let uniffi = try nativeService.probeFile(filePath: path)
        return TTZipOfficeFormat(from: uniffi)
    }

    /// Probes the Office format directly from in-memory bytes.
    public func probe(from data: Data, fileName: String? = nil) throws -> TTZipOfficeFormat {
        let uniffi = try nativeService.probeBytes(data: data, fileName: fileName)
        return TTZipOfficeFormat(from: uniffi)
    }

    /// Extracts worksheet names from an XLSX file on disk.
    public func extractSheetNames(at path: String) throws -> [String] {
        try nativeService.extractSheetNamesFromFile(filePath: path)
    }

    /// Extracts worksheet names from an in-memory XLSX byte buffer.
    public func extractSheetNames(from data: Data, fileName: String? = nil) throws -> [String] {
        try nativeService.extractSheetNames(data: data, fileName: fileName)
    }

    /// Extracts worksheet data at the specified filesystem path.
    public func extractSheetData(
        at path: String,
        sheetNameOrIndex: String = "1",
        maxRows: Int? = nil
    ) throws -> TTZipSheetData {
        let limit = maxRows.map { UInt32($0) }
        let uniffi = try nativeService.extractSheetDataFromFile(
            filePath: path,
            sheetNameOrIndex: sheetNameOrIndex,
            maxRows: limit
        )
        return TTZipSheetData(from: uniffi)
    }

    /// Extracts worksheet data from an in-memory byte buffer.
    public func extractSheetData(
        from data: Data,
        sheetNameOrIndex: String = "1",
        maxRows: Int? = nil,
        fileName: String? = nil
    ) throws -> TTZipSheetData {
        let limit = maxRows.map { UInt32($0) }
        let uniffi = try nativeService.extractSheetData(
            data: data,
            sheetNameOrIndex: sheetNameOrIndex,
            maxRows: limit,
            fileName: fileName
        )
        return TTZipSheetData(from: uniffi)
    }

    /// Dynamically evaluates a formula with optional context cells.
    public func evaluateFormula(
        formula: String,
        contextCells: [TTZipCell]? = nil
    ) throws -> TTZipCellValue {
        let uniffiCells = contextCells?.map { $0.toUniffi() }
        let uniffiRes = try nativeService.evaluateFormula(formula: formula, contextCells: uniffiCells)
        return TTZipCellValue(from: uniffiRes)
    }

    /// Extracts structured DOCX document model from a file on disk.
    public func extractDocx(at path: String) throws -> TTZipDocxDocument {
        let uniffi = try nativeService.extractDocxDocumentFromFile(filePath: path)
        return TTZipDocxDocument(from: uniffi, sourceId: path)
    }

    /// Extracts structured DOCX document model from an in-memory byte buffer.
    public func extractDocx(from data: Data, fileName: String? = nil) throws -> TTZipDocxDocument {
        let uniffi = try nativeService.extractDocxDocument(data: data, fileName: fileName)
        let sourceId = fileName ?? "memory://docx"
        return TTZipDocxDocument(from: uniffi, sourceId: sourceId)
    }

    /// Converts a DOCX file on disk into GitHub-Flavored Markdown.
    public func convertDocxToMarkdown(at path: String) throws -> String {
        try nativeService.convertDocxToMarkdownFromFile(filePath: path)
    }

    /// Converts an in-memory DOCX byte buffer into GitHub-Flavored Markdown.
    public func convertDocxToMarkdown(from data: Data, fileName: String? = nil) throws -> String {
        try nativeService.convertDocxToMarkdown(data: data, fileName: fileName)
    }
}

// MARK: - Swift 6 Observable Facade Service

/// Swift 6 `@Observable` and `Sendable` Office document inspection and formula recalculation service.
///
/// Provides zero-extraction streaming inspection of DOCX, XLSX, and Office publications
/// for UI spreadsheet inspector panels, QuickLook preview generators, and formula engines.
@Observable
public final class TTZipOfficeDocumentService: @unchecked Sendable {

    // MARK: - Shared Singleton

    public static let shared = TTZipOfficeDocumentService()

    // MARK: - Published Observable State

    /// Indicates whether one or more office inspection or extraction operations are in flight.
    public private(set) var isProcessing: Bool = false

    /// Number of concurrent operations currently executing.
    public private(set) var activeOperationsCount: Int = 0

    /// Cumulative total count of office files inspected by this service instance.
    public private(set) var totalDocumentsInspected: Int = 0

    /// Most recently extracted spreadsheet sheet data.
    public private(set) var lastInspectedSheetData: TTZipSheetData? = nil

    /// Most recently extracted structured DOCX document model.
    public private(set) var lastInspectedDocx: TTZipDocxDocument? = nil

    /// Most recent localized error encountered during office document processing.
    public private(set) var latestError: String? = nil

    // MARK: - Internal Storage & Actor Worker

    private let worker = TTZipOfficeDocumentWorker()

    private struct CacheState {
        var sheetNamesCache: [String: [String]] = [:]
        var sheetDataCache: [String: TTZipSheetData] = [:]
        var docxCache: [String: TTZipDocxDocument] = [:]
        var markdownCache: [String: String] = [:]
        var activeCount: Int = 0
        var totalCount: Int = 0
    }

    private let lock = OSAllocatedUnfairLock(initialState: CacheState())

    // MARK: - Initialization

    public init() {}

    // MARK: - Public Inspection APIs

    /// Probes the Office format at a filesystem URL.
    public func probe(url: URL) async throws -> TTZipOfficeFormat {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.probe(at: url.path)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Probes the Office format from an in-memory byte buffer.
    public func probe(data: Data, fileName: String? = nil) async throws -> TTZipOfficeFormat {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.probe(from: data, fileName: fileName)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts spreadsheet worksheet names with caching.
    public func sheetNames(url: URL) async throws -> [String] {
        let path = url.path
        if let cached = lock.withLock({ $0.sheetNamesCache[path] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let names = try await worker.extractSheetNames(at: path)
            lock.withLock {
                $0.sheetNamesCache[path] = names
                $0.totalCount += 1
            }
            self.totalDocumentsInspected += 1
            return names
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts spreadsheet worksheet names from an in-memory byte buffer.
    public func sheetNames(data: Data, fileName: String? = nil) async throws -> [String] {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let names = try await worker.extractSheetNames(from: data, fileName: fileName)
            self.totalDocumentsInspected += 1
            return names
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts parsed worksheet grid data with cell values and formulas from a filesystem URL.
    public func sheetData(
        url: URL,
        sheetNameOrIndex: String = "1",
        maxRows: Int? = nil
    ) async throws -> TTZipSheetData {
        let cacheKey = "\(url.path)#\(sheetNameOrIndex)#\(maxRows ?? 0)"
        if let cached = lock.withLock({ $0.sheetDataCache[cacheKey] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let data = try await worker.extractSheetData(
                at: url.path,
                sheetNameOrIndex: sheetNameOrIndex,
                maxRows: maxRows
            )
            lock.withLock {
                $0.sheetDataCache[cacheKey] = data
            }
            self.lastInspectedSheetData = data
            return data
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts parsed worksheet grid data from an in-memory byte buffer.
    public func sheetData(
        data: Data,
        sheetNameOrIndex: String = "1",
        maxRows: Int? = nil,
        fileName: String? = nil
    ) async throws -> TTZipSheetData {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let res = try await worker.extractSheetData(
                from: data,
                sheetNameOrIndex: sheetNameOrIndex,
                maxRows: maxRows,
                fileName: fileName
            )
            self.lastInspectedSheetData = res
            return res
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Dynamically evaluates a spreadsheet formula (SUM, AVERAGE, MIN, MAX, COUNT, IF, CONCAT, arithmetic).
    public func evaluateFormula(
        formula: String,
        contextCells: [TTZipCell]? = nil
    ) async throws -> TTZipCellValue {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.evaluateFormula(formula: formula, contextCells: contextCells)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts structured DOCX document model with paragraphs, tables, and metrics from a file URL.
    public func docxDocument(url: URL) async throws -> TTZipDocxDocument {
        let path = url.path
        if let cached = lock.withLock({ $0.docxCache[path] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let doc = try await worker.extractDocx(at: path)
            lock.withLock {
                $0.docxCache[path] = doc
                $0.totalCount += 1
            }
            self.lastInspectedDocx = doc
            self.totalDocumentsInspected += 1
            return doc
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts structured DOCX document model from an in-memory byte buffer.
    public func docxDocument(data: Data, fileName: String? = nil) async throws -> TTZipDocxDocument {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let doc = try await worker.extractDocx(from: data, fileName: fileName)
            self.lastInspectedDocx = doc
            self.totalDocumentsInspected += 1
            return doc
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Converts a DOCX file at a filesystem URL into GitHub-Flavored Markdown.
    public func docxToMarkdown(url: URL) async throws -> String {
        let path = url.path
        if let cached = lock.withLock({ $0.markdownCache[path] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let md = try await worker.convertDocxToMarkdown(at: path)
            lock.withLock {
                $0.markdownCache[path] = md
            }
            return md
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Converts an in-memory DOCX byte buffer into GitHub-Flavored Markdown.
    public func docxToMarkdown(data: Data, fileName: String? = nil) async throws -> String {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.convertDocxToMarkdown(from: data, fileName: fileName)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Clears all in-memory caches.
    public func clearCache() {
        lock.withLock {
            $0.sheetNamesCache.removeAll(keepingCapacity: false)
            $0.sheetDataCache.removeAll(keepingCapacity: false)
            $0.docxCache.removeAll(keepingCapacity: false)
            $0.markdownCache.removeAll(keepingCapacity: false)
        }
        self.lastInspectedSheetData = nil
        self.lastInspectedDocx = nil
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
