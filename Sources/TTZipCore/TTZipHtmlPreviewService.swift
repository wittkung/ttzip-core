// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation
import os
#if canImport(WebKit)
import WebKit
#endif

// MARK: - Strongly-Typed Domain Models

/// Supported HTML-adjacent document and container format classifications.
public enum TTZipHtmlFormat: String, Sendable, Codable, CaseIterable, Equatable, Hashable {
    case unknown
    case html
    case xhtml
    case mhtml
    case htmlFragment
    case svg

    internal init(from uniffi: UniFfiHtmlFormat) {
        switch uniffi {
        case .unknown: self = .unknown
        case .html: self = .html
        case .xhtml: self = .xhtml
        case .mhtml: self = .mhtml
        case .htmlFragment: self = .htmlFragment
        case .svg: self = .svg
        }
    }
}

/// Security sanitization and VFS transformation policy for HTML documents.
public struct TTZipHtmlSanitizationPolicy: Sendable, Codable, Equatable, Hashable {
    public var allowScripts: Bool
    public var allowInlineStyles: Bool
    public var allowExternalResources: Bool
    public var allowForms: Bool
    public var allowIframes: Bool
    public var customAllowedTags: [String]
    public var customBlockedTags: [String]

    public init(
        allowScripts: Bool = false,
        allowInlineStyles: Bool = true,
        allowExternalResources: Bool = false,
        allowForms: Bool = false,
        allowIframes: Bool = false,
        customAllowedTags: [String] = [],
        customBlockedTags: [String] = ["script", "iframe", "frame", "frameset", "object", "embed", "applet"]
    ) {
        self.allowScripts = allowScripts
        self.allowInlineStyles = allowInlineStyles
        self.allowExternalResources = allowExternalResources
        self.allowForms = allowForms
        self.allowIframes = allowIframes
        self.customAllowedTags = customAllowedTags
        self.customBlockedTags = customBlockedTags
    }

    /// Strict security policy stripping all scripts, iframes, and network requests.
    public static let defaultStrict = TTZipHtmlSanitizationPolicy(
        allowScripts: false,
        allowInlineStyles: true,
        allowExternalResources: false,
        allowForms: false,
        allowIframes: false
    )

    /// Permissive policy for trusted local HTML documents.
    public static let defaultPermissive = TTZipHtmlSanitizationPolicy(
        allowScripts: true,
        allowInlineStyles: true,
        allowExternalResources: true,
        allowForms: true,
        allowIframes: true,
        customAllowedTags: [],
        customBlockedTags: []
    )

    /// Safe default policy for standard archive previews.
    public static let defaultSafePreview = TTZipHtmlSanitizationPolicy()

    internal func toUniFfi() -> UniFfiHtmlSanitizationPolicy {
        UniFfiHtmlSanitizationPolicy(
            allowScripts: allowScripts,
            allowInlineStyles: allowInlineStyles,
            allowExternalResources: allowExternalResources,
            allowForms: allowForms,
            allowIframes: allowIframes,
            customAllowedTags: customAllowedTags,
            customBlockedTags: customBlockedTags
        )
    }
}

/// Extracted HTML resource link descriptor for embedded archive assets.
public struct TTZipHtmlResourceLink: Sendable, Equatable, Hashable, Identifiable {
    public var id: String { "\(tagName)_\(attributeName)_\(originalUri)" }
    public let tagName: String
    public let attributeName: String
    public let originalUri: String
    public let resolvedVfsUri: String?
    public let resourceType: String
    public let isExternal: Bool

    public init(
        tagName: String,
        attributeName: String,
        originalUri: String,
        resolvedVfsUri: String? = nil,
        resourceType: String = "other",
        isExternal: Bool = false
    ) {
        self.tagName = tagName
        self.attributeName = attributeName
        self.originalUri = originalUri
        self.resolvedVfsUri = resolvedVfsUri
        self.resourceType = resourceType
        self.isExternal = isExternal
    }

    internal init(from uniffi: UniFfiHtmlResourceLink) {
        self.tagName = uniffi.tagName
        self.attributeName = uniffi.attributeName
        self.originalUri = uniffi.originalUri
        self.resolvedVfsUri = uniffi.resolvedVfsUri
        self.resourceType = uniffi.resourceType
        self.isExternal = uniffi.isExternal
    }
}

/// Transformed HTML preview result with metrics and resolved resource links.
public struct TTZipHtmlTransformResult: Sendable, Equatable, Hashable, Identifiable {
    public var id: String
    public let transformedHtml: String
    public let extractedResources: [TTZipHtmlResourceLink]
    public let title: String?
    public let charset: String?
    public let hasScripts: Bool
    public let hasInlineStyles: Bool
    public let metricsChars: Int
    public let metricsWords: Int

    public init(
        id: String = UUID().uuidString,
        transformedHtml: String,
        extractedResources: [TTZipHtmlResourceLink] = [],
        title: String? = nil,
        charset: String? = nil,
        hasScripts: Bool = false,
        hasInlineStyles: Bool = false,
        metricsChars: Int = 0,
        metricsWords: Int = 0
    ) {
        self.id = id
        self.transformedHtml = transformedHtml
        self.extractedResources = extractedResources
        self.title = title
        self.charset = charset
        self.hasScripts = hasScripts
        self.hasInlineStyles = hasInlineStyles
        self.metricsChars = metricsChars
        self.metricsWords = metricsWords
    }

    internal init(from uniffi: UniFfiHtmlTransformResult, id: String = UUID().uuidString) {
        self.id = id
        self.transformedHtml = uniffi.transformedHtml
        self.extractedResources = uniffi.extractedResources.map { TTZipHtmlResourceLink(from: $0) }
        self.title = uniffi.title
        self.charset = uniffi.charset
        self.hasScripts = uniffi.hasScripts
        self.hasInlineStyles = uniffi.hasInlineStyles
        self.metricsChars = Int(uniffi.metricsChars)
        self.metricsWords = Int(uniffi.metricsWords)
    }
}

// MARK: - VFS Resource Provider Protocol

/// Asynchronous resource resolver providing zero-extraction streaming for `ttzip-vfs://` requests.
public protocol TTZipVfsResourceProvider: Sendable {
    /// Loads the complete resource data and MIME type for a resolved VFS URI.
    func loadResource(uri: String) async throws -> (data: Data, mimeType: String)?

    /// Loads a partial byte range of the resource for HTTP 206 streaming requests.
    func loadResourceRange(uri: String, byteRange: ClosedRange<Int>?) async throws -> (data: Data, fullSize: Int64, mimeType: String)?
}

// MARK: - WebKit URL Scheme Handler

#if canImport(WebKit)
/// High-performance WebKit custom URL scheme handler intercepting `ttzip-vfs://` protocol requests.
///
/// Features:
/// - Zero-extraction direct streaming from archive memory buffers.
/// - Full HTTP 206 Partial Content Range support for large images, audio, and video seek operations.
/// - Non-blocking async resource resolution with task cancellation safeguards.
public final class TTZipVfsSchemeHandler: NSObject, WKURLSchemeHandler, @unchecked Sendable {
    public nonisolated static let scheme = "ttzip-vfs"

    private let provider: TTZipVfsResourceProvider
    private let activeTasksLock = OSAllocatedUnfairLock(initialState: [ObjectIdentifier: Task<Void, Never>]())

    public init(provider: TTZipVfsResourceProvider) {
        self.provider = provider
        super.init()
    }

    public func webView(_ webView: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        guard let url = urlSchemeTask.request.url else {
            urlSchemeTask.didFailWithError(URLError(.badURL))
            return
        }

        let taskId = ObjectIdentifier(urlSchemeTask)
        let requestUri = url.absoluteString

        let rangeHeader = urlSchemeTask.request.value(forHTTPHeaderField: "Range")
        let parsedRange = parseRangeHeader(rangeHeader)

        let executionTask = Task { [provider] in
            do {
                if let (data, fullSize, mimeType) = try await provider.loadResourceRange(uri: requestUri, byteRange: parsedRange) {
                    guard !Task.isCancelled else { return }

                    let isPartial = parsedRange != nil
                    let statusCode = isPartial ? 206 : 200

                    var headers: [String: String] = [
                        "Content-Type": mimeType,
                        "Content-Length": "\(data.count)",
                        "Accept-Ranges": "bytes",
                        "Access-Control-Allow-Origin": "*",
                        "Cache-Control": "max-age=3600, immutable"
                    ]

                    if let range = parsedRange {
                        let rangeEnd = min(Int64(range.upperBound), fullSize - 1)
                        headers["Content-Range"] = "bytes \(range.lowerBound)-\(rangeEnd)/\(fullSize)"
                    }

                    guard let response = HTTPURLResponse(
                        url: url,
                        statusCode: statusCode,
                        httpVersion: "HTTP/1.1",
                        headerFields: headers
                    ) else {
                        urlSchemeTask.didFailWithError(URLError(.cannotParseResponse))
                        return
                    }

                    urlSchemeTask.didReceive(response)
                    urlSchemeTask.didReceive(data)
                    urlSchemeTask.didFinish()
                } else if let (data, mimeType) = try await provider.loadResource(uri: requestUri) {
                    guard !Task.isCancelled else { return }

                    let headers: [String: String] = [
                        "Content-Type": mimeType,
                        "Content-Length": "\(data.count)",
                        "Accept-Ranges": "bytes",
                        "Access-Control-Allow-Origin": "*",
                        "Cache-Control": "max-age=3600, immutable"
                    ]

                    guard let response = HTTPURLResponse(
                        url: url,
                        statusCode: 200,
                        httpVersion: "HTTP/1.1",
                        headerFields: headers
                    ) else {
                        urlSchemeTask.didFailWithError(URLError(.cannotParseResponse))
                        return
                    }

                    urlSchemeTask.didReceive(response)
                    urlSchemeTask.didReceive(data)
                    urlSchemeTask.didFinish()
                } else {
                    guard !Task.isCancelled else { return }
                    if let notFoundResponse = HTTPURLResponse(
                        url: url,
                        statusCode: 404,
                        httpVersion: "HTTP/1.1",
                        headerFields: ["Content-Type": "text/plain; charset=utf-8"]
                    ) {
                        urlSchemeTask.didReceive(notFoundResponse)
                        urlSchemeTask.didReceive(Data("404 VFS Resource Not Found".utf8))
                        urlSchemeTask.didFinish()
                    } else {
                        urlSchemeTask.didFailWithError(URLError(.fileDoesNotExist))
                    }
                }
            } catch {
                guard !Task.isCancelled else { return }
                urlSchemeTask.didFailWithError(error)
            }

            activeTasksLock.withLock { _ = $0.removeValue(forKey: taskId) }
        }

        activeTasksLock.withLock { $0[taskId] = executionTask }
    }

    public func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {
        let taskId = ObjectIdentifier(urlSchemeTask)
        let runningTask = activeTasksLock.withLock { $0.removeValue(forKey: taskId) }
        runningTask?.cancel()
    }

    // MARK: - Internal HTTP Range Parser

    private func parseRangeHeader(_ header: String?) -> ClosedRange<Int>? {
        guard let header = header, header.hasPrefix("bytes=") else { return nil }
        let spec = header.dropFirst(6).trimmingCharacters(in: .whitespaces)
        let parts = spec.split(separator: "-", omittingEmptySubsequences: false)
        guard parts.count == 2 else { return nil }

        if let start = Int(parts[0]), let end = Int(parts[1]), start <= end {
            return start...end
        } else if let start = Int(parts[0]), parts[1].isEmpty {
            return start...Int.max - 1
        }
        return nil
    }
}
#endif

// MARK: - Swift 6 Actor-Isolated Background Worker

/// Actor-isolated background worker executing UniFFI C-ABI HTML transformation pipelines.
public actor TTZipHtmlWorker {
    private let nativeService: UniFfiHtmlService

    public init() {
        self.nativeService = UniFfiHtmlService()
    }

    /// Probes HTML format at a filesystem path.
    public func probe(at path: String) throws -> TTZipHtmlFormat {
        let uniffi = try nativeService.probeFile(filePath: path)
        return TTZipHtmlFormat(from: uniffi)
    }

    /// Probes HTML format directly from in-memory bytes.
    public func probe(from data: Data, fileName: String? = nil) throws -> TTZipHtmlFormat {
        let uniffi = try nativeService.probeBytes(bytes: data, fileName: fileName)
        return TTZipHtmlFormat(from: uniffi)
    }

    /// Rewrites relative resource links to `ttzip-vfs://` and sanitizes markup.
    public func transform(
        html: String,
        baseVfsPrefix: String,
        policy: TTZipHtmlSanitizationPolicy
    ) throws -> TTZipHtmlTransformResult {
        let uniffi = try nativeService.rewriteVfs(
            htmlContent: html,
            baseVfsPrefix: baseVfsPrefix,
            policy: policy.toUniFfi()
        )
        return TTZipHtmlTransformResult(from: uniffi)
    }

    /// Sanitizes HTML markup according to policy without altering relative links.
    public func sanitize(
        html: String,
        policy: TTZipHtmlSanitizationPolicy
    ) throws -> String {
        try nativeService.sanitize(htmlContent: html, policy: policy.toUniFfi())
    }

    /// Extracts all resource links from HTML markup.
    public func extractResources(html: String) throws -> [TTZipHtmlResourceLink] {
        let uniffiList = try nativeService.extractResources(htmlContent: html)
        return uniffiList.map { TTZipHtmlResourceLink(from: $0) }
    }
}

// MARK: - Swift 6 Observable Facade Service

/// Swift 6 `@Observable` and `Sendable` HTML preview, transformation, and VFS routing service.
///
/// Features:
/// - Zero-copy VFS URI transformation (`ttzip-vfs://`) for WebKit safe rendering.
/// - Tree-sitter incremental syntax highlight hot-sync for live source code editing.
/// - WebKit scheme registration and partial byte range streaming.
@Observable
public final class TTZipHtmlPreviewService: @unchecked Sendable {

    // MARK: - Shared Singleton

    public static let shared = TTZipHtmlPreviewService()

    // MARK: - Published Observable State

    /// Indicates whether one or more HTML transformation operations are currently in flight.
    public private(set) var isProcessing: Bool = false

    /// Number of concurrent background operations currently running.
    public private(set) var activeOperationsCount: Int = 0

    /// Most recently transformed HTML preview result.
    public private(set) var lastTransformResult: TTZipHtmlTransformResult? = nil

    /// Most recently extracted resource links.
    public private(set) var lastExtractedResources: [TTZipHtmlResourceLink] = []

    /// Most recent localized error encountered during HTML processing.
    public private(set) var latestError: String? = nil

    // MARK: - Internal Storage & Actor Worker

    private let worker = TTZipHtmlWorker()

    private struct CacheState {
        var transformCache: [String: TTZipHtmlTransformResult] = [:]
        var activeCount: Int = 0
    }

    private let lock = OSAllocatedUnfairLock(initialState: CacheState())

    // MARK: - Initialization

    public init() {}

    // MARK: - Public Transformation APIs

    /// Probes the format classification of an HTML file at a filesystem URL.
    public func probe(url: URL) async throws -> TTZipHtmlFormat {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.probe(at: url.path)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Probes the format classification of an in-memory HTML byte buffer.
    public func probe(data: Data, fileName: String? = nil) async throws -> TTZipHtmlFormat {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.probe(from: data, fileName: fileName)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Transforms and sanitizes HTML markup with `ttzip-vfs://` URI rewriting.
    public func transform(
        html: String,
        baseVfsPrefix: String,
        policy: TTZipHtmlSanitizationPolicy = .defaultSafePreview
    ) async throws -> TTZipHtmlTransformResult {
        let cacheKey = "\(baseVfsPrefix)_\(policy.hashValue)_\(html.hashValue)"
        if let cached = lock.withLock({ $0.transformCache[cacheKey] }) {
            self.lastTransformResult = cached
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let result = try await worker.transform(
                html: html,
                baseVfsPrefix: baseVfsPrefix,
                policy: policy
            )
            lock.withLock {
                $0.transformCache[cacheKey] = result
            }
            self.lastTransformResult = result
            self.lastExtractedResources = result.extractedResources
            return result
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Sanitizes HTML markup according to policy without altering relative resource paths.
    public func sanitize(
        html: String,
        policy: TTZipHtmlSanitizationPolicy = .defaultSafePreview
    ) async throws -> String {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.sanitize(html: html, policy: policy)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts all asset and resource links from HTML markup.
    public func extractResources(html: String) async throws -> [TTZipHtmlResourceLink] {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let resources = try await worker.extractResources(html: html)
            self.lastExtractedResources = resources
            return resources
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    // MARK: - Tree-sitter Editor & WebKit Live Sync

    /// Tokenizes HTML source code into UTF-16 highlight tokens for live code editor views.
    public func highlightSource(code: String) async -> [TTZipHighlightToken] {
        await TTZipSyntaxHighlightService.shared.highlight(code: code, language: "html")
    }

    /// Synchronizes source editor content with WebKit DOM preview in real time.
    public func syncPreview(
        sourceCode: String,
        baseVfsPrefix: String,
        policy: TTZipHtmlSanitizationPolicy? = nil
    ) async throws -> TTZipHtmlTransformResult {
        let activePolicy = policy ?? .defaultSafePreview
        return try await transform(html: sourceCode, baseVfsPrefix: baseVfsPrefix, policy: activePolicy)
    }

    #if canImport(WebKit)
    /// Registers the `ttzip-vfs://` scheme handler on a `WKWebViewConfiguration`.
    @MainActor
    public func configureSchemeHandler(
        for configuration: WKWebViewConfiguration,
        provider: TTZipVfsResourceProvider
    ) {
        let handler = TTZipVfsSchemeHandler(provider: provider)
        configuration.setURLSchemeHandler(handler, forURLScheme: TTZipVfsSchemeHandler.scheme)
    }
    #endif

    /// Clears all cached transformation results.
    public func clearCache() {
        lock.withLock {
            $0.transformCache.removeAll(keepingCapacity: false)
        }
        self.lastTransformResult = nil
        self.lastExtractedResources = []
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
