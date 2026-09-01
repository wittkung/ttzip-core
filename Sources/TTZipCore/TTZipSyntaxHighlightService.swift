// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation
import os

// MARK: - Enums & Strongly-Typed Highlight Models

/// Syntactic category of a highlight token.
public enum TTZipHighlightCategory: String, Sendable, CaseIterable, Identifiable {
    case keyword = "keyword"
    case string = "string"
    case number = "number"
    case type = "type"
    case function = "function"
    case comment = "comment"
    case `operator` = "operator"
    case variable = "variable"
    case attribute = "attribute"
    case punctuation = "punctuation"
    case unknown = "unknown"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .keyword: return "Keyword"
        case .string: return "String Literal"
        case .number: return "Numeric Constant"
        case .type: return "Type Definition"
        case .function: return "Function / Method"
        case .comment: return "Comment"
        case .operator: return "Operator"
        case .variable: return "Variable"
        case .attribute: return "Attribute / Annotation"
        case .punctuation: return "Punctuation"
        case .unknown: return "General Syntax"
        }
    }

    /// Converts raw UniFFI string to category enum.
    public static func from(raw: String) -> TTZipHighlightCategory {
        let clean = raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return TTZipHighlightCategory(rawValue: clean) ?? .unknown
    }
}

/// Strongly-typed highlight token with UTF-16 NSRange geometry and line coordinates.
public struct TTZipHighlightToken: Sendable, Identifiable, Equatable, Hashable {
    public var id: String { "\(location)_\(length)_\(category.rawValue)" }
    /// Zero-based character offset in UTF-16 code units (NSRange location).
    public let location: Int
    /// Character span length in UTF-16 code units (NSRange length).
    public let length: Int
    /// Classified syntactic category.
    public let category: TTZipHighlightCategory
    /// Original raw category identifier string from engine.
    public let rawCategory: String
    /// 1-based source line number.
    public let lineNumber: Int
    /// 0-based character column offset on line.
    public let column: Int

    /// Apple Cocoa NSRange metric for direct TextKit / NSTextView / AttributedString formatting.
    public var nsRange: NSRange {
        NSRange(location: location, length: length)
    }

    public init(
        location: Int,
        length: Int,
        category: TTZipHighlightCategory,
        rawCategory: String? = nil,
        lineNumber: Int = 1,
        column: Int = 0
    ) {
        self.location = location
        self.length = length
        self.category = category
        self.rawCategory = rawCategory ?? category.rawValue
        self.lineNumber = lineNumber
        self.column = column
    }

    /// Initializes domain token model from UniFFI bridge record.
    public init(from uniffi: UniFfiHighlightToken) {
        self.location = Int(uniffi.location)
        self.length = Int(uniffi.length)
        self.category = TTZipHighlightCategory.from(raw: uniffi.category)
        self.rawCategory = uniffi.category
        self.lineNumber = Int(uniffi.lineNumber)
        self.column = Int(uniffi.column)
    }
}

// MARK: - Enums & Strongly-Typed Symbol Outline Models

/// Structural classification of an AST symbol node.
public enum TTZipSymbolKind: Sendable, Equatable, Hashable {
    case function
    case structure
    case enumeration
    case classDeclaration
    case protocolDeclaration
    case interface
    case trait
    case implementation
    case variable
    case constant
    case module
    case typeAlias
    case macro
    case heading(level: Int)
    case property
    case unknown(String)

    public var displayName: String {
        switch self {
        case .function: return "Function"
        case .structure: return "Struct"
        case .enumeration: return "Enum"
        case .classDeclaration: return "Class"
        case .protocolDeclaration: return "Protocol"
        case .interface: return "Interface"
        case .trait: return "Trait"
        case .implementation: return "Implementation"
        case .variable: return "Variable"
        case .constant: return "Constant"
        case .module: return "Module"
        case .typeAlias: return "Type"
        case .macro: return "Macro"
        case .heading(let lvl): return "Heading \(lvl)"
        case .property: return "Property"
        case .unknown(let s): return s.capitalized
        }
    }

    /// SF Symbol icon representation for macOS / iOS symbol navigation tree.
    public var sfSymbolName: String {
        switch self {
        case .function: return "f.cursive"
        case .structure: return "square.3.layers.3d"
        case .enumeration: return "list.bullet"
        case .classDeclaration: return "c.square"
        case .protocolDeclaration, .interface, .trait: return "p.square"
        case .implementation: return "hammer"
        case .variable, .property: return "v.square"
        case .constant: return "number.square"
        case .module: return "shippingbox"
        case .typeAlias: return "t.square"
        case .macro: return "wand.and.stars"
        case .heading: return "text.quote"
        case .unknown: return "chevron.left.forwardslash.chevron.right"
        }
    }

    public static func from(rawKind: String) -> TTZipSymbolKind {
        let clean = rawKind.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if clean.starts_with_heading {
            let level = Int(clean.dropFirst()) ?? 1
            return .heading(level: level)
        }
        switch clean {
        case "function", "func", "fn", "def", "method": return .function
        case "struct", "structure": return .structure
        case "enum", "enumeration": return .enumeration
        case "class": return .classDeclaration
        case "protocol": return .protocolDeclaration
        case "interface": return .interface
        case "trait": return .trait
        case "impl", "implementation", "extension": return .implementation
        case "variable", "var", "let": return .variable
        case "constant", "const": return .constant
        case "module", "mod", "package", "namespace": return .module
        case "type", "typedef", "typealias": return .typeAlias
        case "macro": return .macro
        case "property": return .property
        default: return .unknown(rawKind)
        }
    }
}

private extension String {
    var starts_with_heading: Bool {
        if hasPrefix("h") && count == 2 {
            let second = self[index(after: startIndex)]
            return second >= "1" && second <= "6"
        }
        return false
    }
}

/// Structural outline node representing an AST declaration (function, class, heading, etc.).
public struct TTZipSymbolNode: Sendable, Identifiable, Equatable, Hashable {
    public var id: String { "\(lineNumber)_\(location)_\(name)" }
    /// Identifier name (e.g. "func decompress()", "struct Header").
    public let name: String
    /// Structural semantic classification.
    public let kind: TTZipSymbolKind
    /// Original raw kind string from engine.
    public let rawKind: String
    /// Zero-based character start index in UTF-16 code units.
    public let location: Int
    /// Character span length in UTF-16 code units.
    public let length: Int
    /// 1-based source line number.
    public let lineNumber: Int
    /// Optional signature detail or preview snippet.
    public let detail: String?
    /// Hierarchical child symbols.
    public let children: [TTZipSymbolNode]

    /// Apple Cocoa NSRange metric for text selection and jump.
    public var nsRange: NSRange {
        NSRange(location: location, length: length)
    }

    public init(
        name: String,
        kind: TTZipSymbolKind,
        rawKind: String? = nil,
        location: Int,
        length: Int,
        lineNumber: Int,
        detail: String? = nil,
        children: [TTZipSymbolNode] = []
    ) {
        self.name = name
        self.kind = kind
        self.rawKind = rawKind ?? kind.displayName.lowercased()
        self.location = location
        self.length = length
        self.lineNumber = lineNumber
        self.detail = detail
        self.children = children
    }

    /// Initializes domain symbol node from UniFFI bridge record.
    public init(from uniffi: UniFfiSymbolNode) {
        self.name = uniffi.name
        self.kind = TTZipSymbolKind.from(rawKind: uniffi.kind)
        self.rawKind = uniffi.kind
        self.location = Int(uniffi.location)
        self.length = Int(uniffi.length)
        self.lineNumber = Int(uniffi.lineNumber)
        self.detail = uniffi.detail
        self.children = uniffi.children.map(TTZipSymbolNode.init)
    }
}

// MARK: - Language Information Descriptor

/// Metadata descriptor of a recognized programming or document format.
public struct TTZipLanguageInfo: Sendable, Identifiable, Equatable, Hashable {
    public var id: String { languageId }
    /// Canonical language identifier string (e.g. "rust", "swift", "python").
    public let languageId: String
    /// Human-readable display name.
    public let displayName: String
    /// Associated file extensions without leading dot.
    public let fileExtensions: [String]
    /// MIME content types.
    public let mimeTypes: [String]
    /// Whether high-precision native AST parsing is supported.
    public let isSupported: Bool

    public init(
        languageId: String,
        displayName: String,
        fileExtensions: [String] = [],
        mimeTypes: [String] = [],
        isSupported: Bool = true
    ) {
        self.languageId = languageId
        self.displayName = displayName
        self.fileExtensions = fileExtensions
        self.mimeTypes = mimeTypes
        self.isSupported = isSupported
    }

    /// Initializes domain language model from UniFFI bridge record.
    public init(from uniffi: UniFfiLanguageInfo) {
        self.languageId = uniffi.languageId
        self.displayName = uniffi.displayName
        self.fileExtensions = uniffi.fileExtensions
        self.mimeTypes = uniffi.mimeTypes
        self.isSupported = uniffi.isSupported
    }
}

/// Comprehensive analysis result of a source code document.
public struct TTZipHighlightedDocument: Sendable, Equatable, Hashable {
    public let sourceCode: String
    public let language: TTZipLanguageInfo
    public let tokens: [TTZipHighlightToken]
    public let symbols: [TTZipSymbolNode]
    public let lineCount: Int

    public init(
        sourceCode: String,
        language: TTZipLanguageInfo,
        tokens: [TTZipHighlightToken],
        symbols: [TTZipSymbolNode],
        lineCount: Int
    ) {
        self.sourceCode = sourceCode
        self.language = language
        self.tokens = tokens
        self.symbols = symbols
        self.lineCount = lineCount
    }
}

// MARK: - Actor-Isolated Cache

/// High-speed actor managing in-memory token and symbol cache to optimize virtual scrolling.
public actor TTZipSyntaxCacheActor {
    private var tokenCache: [String: [TTZipHighlightToken]] = [:]
    private var symbolCache: [String: [TTZipSymbolNode]] = [:]
    private let maxEntries: Int

    public init(maxEntries: Int = 128) {
        self.maxEntries = maxEntries
    }

    public func getTokens(key: String) -> [TTZipHighlightToken]? {
        tokenCache[key]
    }

    public func setTokens(key: String, tokens: [TTZipHighlightToken]) {
        if tokenCache.count >= maxEntries {
            tokenCache.removeAll(keepingCapacity: true)
        }
        tokenCache[key] = tokens
    }

    public func getSymbols(key: String) -> [TTZipSymbolNode]? {
        symbolCache[key]
    }

    public func setSymbols(key: String, symbols: [TTZipSymbolNode]) {
        if symbolCache.count >= maxEntries {
            symbolCache.removeAll(keepingCapacity: true)
        }
        symbolCache[key] = symbols
    }

    public func clear() {
        tokenCache.removeAll()
        symbolCache.removeAll()
    }
}

// MARK: - TTZipSyntaxHighlightService Facade

/// Strongly-typed Swift 6 syntax highlighting and symbol outline service.
///
/// Exposes high-throughput AST tokenization, viewport coloring, and hierarchical
/// outline extraction backed by the Rust microkernel without uncompressing to disk.
@Observable
public final class TTZipSyntaxHighlightService: @unchecked Sendable {
    /// Shared singleton instance.
    public static let shared = TTZipSyntaxHighlightService()

    @ObservationIgnored
    private let engine: UniFfiSyntaxService

    @ObservationIgnored
    private let cache: TTZipSyntaxCacheActor

    private struct ServiceState {
        var isProcessing: Bool = false
        var lastDetectedLanguage: TTZipLanguageInfo? = nil
        var currentSymbols: [TTZipSymbolNode] = []
    }

    @ObservationIgnored
    private let stateLock = OSAllocatedUnfairLock(initialState: ServiceState())

    public var isProcessing: Bool {
        stateLock.withLock { $0.isProcessing }
    }

    public var lastDetectedLanguage: TTZipLanguageInfo? {
        stateLock.withLock { $0.lastDetectedLanguage }
    }

    public var currentSymbols: [TTZipSymbolNode] {
        stateLock.withLock { $0.currentSymbols }
    }

    public init(maxCacheEntries: Int = 128) {
        self.engine = UniFfiSyntaxService()
        self.cache = TTZipSyntaxCacheActor(maxEntries: maxCacheEntries)
    }

    // MARK: - Language Detection

    /// Detects language from filename, extension, or content snippet hint.
    public func detectLanguage(filePathOrExtension: String, firstLine: String? = nil) -> TTZipLanguageInfo {
        let uniffiRes = engine.detectLanguage(filePathOrExt: filePathOrExtension, firstLineHint: firstLine)
        let info = TTZipLanguageInfo(from: uniffiRes)

        stateLock.withLock {
            $0.lastDetectedLanguage = info
        }

        return info
    }

    /// Returns list of all supported programming and markup languages.
    public func getSupportedLanguages() -> [TTZipLanguageInfo] {
        engine.getSupportedLanguages().map(TTZipLanguageInfo.init)
    }

    // MARK: - Highlighting APIs

    /// Tokenizes source code into UTF-16 highlight tokens with optional max length truncation.
    public func highlight(
        code: String,
        language: String? = nil,
        maxLength: Int = 0
    ) async -> [TTZipHighlightToken] {
        guard !code.isEmpty else { return [] }

        let effectiveLang = language ?? detectLanguage(filePathOrExtension: code).languageId
        let cacheKey = "hl_\(effectiveLang)_\(maxLength)_\(code.hashValue)"

        if let cached = await cache.getTokens(key: cacheKey) {
            return cached
        }

        setProcessing(true)
        defer { setProcessing(false) }

        let uniffiTokens = engine.highlightCode(
            code: code,
            languageHint: effectiveLang,
            maxLength: UInt32(max(0, maxLength))
        )
        let domainTokens = uniffiTokens.map(TTZipHighlightToken.init)

        await cache.setTokens(key: cacheKey, tokens: domainTokens)
        return domainTokens
    }

    /// Tokenizes source code restricted to a specific line viewport for high-performance virtualized rendering.
    public func highlightViewport(
        code: String,
        language: String? = nil,
        startLine: Int,
        lineCount: Int
    ) async -> [TTZipHighlightToken] {
        guard !code.isEmpty else { return [] }

        let effectiveLang = language ?? "plaintext"
        let cacheKey = "vp_\(effectiveLang)_\(startLine)_\(lineCount)_\(code.hashValue)"

        if let cached = await cache.getTokens(key: cacheKey) {
            return cached
        }

        setProcessing(true)
        defer { setProcessing(false) }

        let uniffiTokens = engine.highlightCodeViewport(
            code: code,
            languageHint: effectiveLang,
            startLine: UInt32(max(1, startLine)),
            lineCount: UInt32(max(0, lineCount))
        )
        let domainTokens = uniffiTokens.map(TTZipHighlightToken.init)

        await cache.setTokens(key: cacheKey, tokens: domainTokens)
        return domainTokens
    }

    // MARK: - Symbol Outline APIs

    /// Extracts structural outline symbol tree from source code.
    public func extractSymbols(code: String, language: String? = nil) async -> [TTZipSymbolNode] {
        guard !code.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return [] }

        let effectiveLang = language ?? detectLanguage(filePathOrExtension: code).languageId
        let cacheKey = "sym_\(effectiveLang)_\(code.hashValue)"

        if let cached = await cache.getSymbols(key: cacheKey) {
            return cached
        }

        setProcessing(true)
        defer { setProcessing(false) }

        let uniffiSymbols = engine.extractSymbols(code: code, languageHint: effectiveLang)
        let domainSymbols = uniffiSymbols.map(TTZipSymbolNode.init)

        stateLock.withLock {
            $0.currentSymbols = domainSymbols
        }

        await cache.setSymbols(key: cacheKey, symbols: domainSymbols)
        return domainSymbols
    }

    /// Complete document analysis producing language metadata, tokens, and symbol tree.
    public func analyzeDocument(
        code: String,
        filePathOrExtension: String
    ) async -> TTZipHighlightedDocument {
        let langInfo = detectLanguage(filePathOrExtension: filePathOrExtension, firstLine: code.components(separatedBy: .newlines).first)
        let tokens = await highlight(code: code, language: langInfo.languageId)
        let symbols = await extractSymbols(code: code, language: langInfo.languageId)
        let lineCount = code.components(separatedBy: .newlines).count

        return TTZipHighlightedDocument(
            sourceCode: code,
            language: langInfo,
            tokens: tokens,
            symbols: symbols,
            lineCount: lineCount
        )
    }

    // MARK: - Synchronous Non-Isolated Methods

    /// Synchronously tokenizes source code for non-async or immediate UI contexts.
    public nonisolated func highlightSync(
        code: String,
        language: String? = nil,
        maxLength: Int = 0
    ) -> [TTZipHighlightToken] {
        guard !code.isEmpty else { return [] }
        let effectiveLang = language ?? "plaintext"
        let uniffiTokens = engine.highlightCode(
            code: code,
            languageHint: effectiveLang,
            maxLength: UInt32(max(0, maxLength))
        )
        return uniffiTokens.map(TTZipHighlightToken.init)
    }

    /// Synchronously tokenizes viewport code for non-async contexts.
    public nonisolated func highlightViewportSync(
        code: String,
        language: String? = nil,
        startLine: Int,
        lineCount: Int
    ) -> [TTZipHighlightToken] {
        guard !code.isEmpty else { return [] }
        let effectiveLang = language ?? "plaintext"
        let uniffiTokens = engine.highlightCodeViewport(
            code: code,
            languageHint: effectiveLang,
            startLine: UInt32(max(1, startLine)),
            lineCount: UInt32(max(0, lineCount))
        )
        return uniffiTokens.map(TTZipHighlightToken.init)
    }

    /// Synchronously extracts structural symbols for immediate UI tree building.
    public nonisolated func extractSymbolsSync(code: String, language: String? = nil) -> [TTZipSymbolNode] {
        guard !code.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return [] }
        let effectiveLang = language ?? "plaintext"
        let uniffiSymbols = engine.extractSymbols(code: code, languageHint: effectiveLang)
        return uniffiSymbols.map(TTZipSymbolNode.init)
    }

    /// Synchronously detects language.
    public nonisolated func detectLanguageSync(filePathOrExtension: String, firstLine: String? = nil) -> TTZipLanguageInfo {
        let uniffiRes = engine.detectLanguage(filePathOrExt: filePathOrExtension, firstLineHint: firstLine)
        return TTZipLanguageInfo(from: uniffiRes)
    }

    /// Clears internal actor token and symbol caches.
    public func clearCache() async {
        await cache.clear()
    }

    // MARK: - Private Helpers

    private func setProcessing(_ processing: Bool) {
        stateLock.withLock {
            $0.isProcessing = processing
        }
    }
}
