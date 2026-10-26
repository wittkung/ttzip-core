// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
#if canImport(WebKit)
import WebKit
#endif

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
/// - Isolated to `@MainActor` to ensure all WKURLSchemeTask lifecycle mutations run on the main thread.
/// - Atomic task cancellation and stopped-state tracking to eliminate WebKit internal assertion crashes.
/// - Zero-extraction direct streaming from archive memory buffers.
/// - Full HTTP 206 Partial Content Range support for large images, audio, and video seek operations.
@MainActor
public final class TTZipVfsSchemeHandler: NSObject, WKURLSchemeHandler {
    public nonisolated static let scheme = "ttzip-vfs"

    private let provider: TTZipVfsResourceProvider
    private var activeTasks: [ObjectIdentifier: Task<Void, Never>] = [:]
    private var stoppedTasks = Set<ObjectIdentifier>()

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

        let executionTask = Task { @MainActor [weak self, provider] in
            let fetchResult = await Task.detached(priority: .userInitiated) { () -> Result<(HTTPURLResponse, Data), Error> in
                do {
                    if let (data, fullSize, mimeType) = try await provider.loadResourceRange(uri: requestUri, byteRange: parsedRange) {
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
                            return .failure(URLError(.cannotParseResponse))
                        }

                        return .success((response, data))
                    } else if let (data, mimeType) = try await provider.loadResource(uri: requestUri) {
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
                            return .failure(URLError(.cannotParseResponse))
                        }

                        return .success((response, data))
                    } else {
                        if let notFoundResponse = HTTPURLResponse(
                            url: url,
                            statusCode: 404,
                            httpVersion: "HTTP/1.1",
                            headerFields: ["Content-Type": "text/plain; charset=utf-8"]
                        ) {
                            return .success((notFoundResponse, Data("404 VFS Resource Not Found".utf8)))
                        } else {
                            return .failure(URLError(.fileDoesNotExist))
                        }
                    }
                } catch {
                    return .failure(error)
                }
            }.value

            guard let self else { return }

            guard !self.stoppedTasks.contains(taskId), self.activeTasks[taskId] != nil else {
                self.stoppedTasks.remove(taskId)
                return
            }
            self.activeTasks.removeValue(forKey: taskId)

            switch fetchResult {
            case .success(let (response, data)):
                urlSchemeTask.didReceive(response)
                urlSchemeTask.didReceive(data)
                urlSchemeTask.didFinish()
            case .failure(let error):
                urlSchemeTask.didFailWithError(error)
            }
        }

        activeTasks[taskId] = executionTask
    }

    public func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {
        let taskId = ObjectIdentifier(urlSchemeTask)
        stoppedTasks.insert(taskId)
        if let runningTask = activeTasks.removeValue(forKey: taskId) {
            runningTask.cancel()
        }
    }

    // MARK: - Internal HTTP Range Parser

    private nonisolated func parseRangeHeader(_ header: String?) -> ClosedRange<Int>? {
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
