// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

#if canImport(WebKit)
import WebKit

@MainActor
public final class VfsSchemeHandler: NSObject, WKURLSchemeHandler {
    private let read: @Sendable (String) async throws -> (data: Data, mimeType: String)?
    private let readRange: @Sendable (String, ClosedRange<Int>?) async throws -> (data: Data, fullSize: Int64, mimeType: String)?
    private var activeTasks: [ObjectIdentifier: Task<Void, Never>] = [:]
    private var stoppedTasks = Set<ObjectIdentifier>()

    public init(
        read: @escaping @Sendable (String) async throws -> (data: Data, mimeType: String)?,
        readRange: @escaping @Sendable (String, ClosedRange<Int>?) async throws -> (data: Data, fullSize: Int64, mimeType: String)?
    ) {
        self.read = read
        self.readRange = readRange
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

        let executionTask = Task { @MainActor [weak self, read, readRange] in
            let fetchResult = await Task.detached(priority: .userInitiated) {
                await Self.fetchVfsResource(
                    read: read,
                    readRange: readRange,
                    url: url,
                    requestUri: requestUri,
                    parsedRange: parsedRange
                )
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

    private nonisolated static func fetchVfsResource(
        read: @Sendable (String) async throws -> (data: Data, mimeType: String)?,
        readRange: @Sendable (String, ClosedRange<Int>?) async throws -> (data: Data, fullSize: Int64, mimeType: String)?,
        url: URL,
        requestUri: String,
        parsedRange: ClosedRange<Int>?
    ) async -> Result<(HTTPURLResponse, Data), Error> {
        do {
            if let (data, fullSize, mimeType) = try await readRange(requestUri, parsedRange) {
                guard let range = parsedRange else {
                    return buildStreamingResponse(url: url, statusCode: 200, data: data, mimeType: mimeType, contentRange: nil)
                }
                let start = Int64(range.lowerBound)
                guard start < fullSize, !data.isEmpty else {
                    return buildStreamingResponse(
                        url: url,
                        statusCode: 416,
                        data: Data(),
                        mimeType: mimeType,
                        contentRange: "bytes */\(fullSize)"
                    )
                }
                let maximumEnd = min(Int64(range.upperBound), fullSize - 1)
                let lastByteOffset = Int64(data.count) - 1
                guard lastByteOffset <= maximumEnd - start else {
                    return .failure(URLError(.cannotParseResponse))
                }
                return buildStreamingResponse(
                    url: url,
                    statusCode: 206,
                    data: data,
                    mimeType: mimeType,
                    contentRange: "bytes \(start)-\(start + lastByteOffset)/\(fullSize)"
                )
            }
            if let (data, mimeType) = try await read(requestUri) {
                return buildStreamingResponse(
                    url: url,
                    statusCode: 200,
                    data: data,
                    mimeType: mimeType,
                    contentRange: nil
                )
            }
            guard let notFoundResponse = HTTPURLResponse(
                url: url,
                statusCode: 404,
                httpVersion: "HTTP/1.1",
                headerFields: ["Content-Type": "text/plain; charset=utf-8"]
            ) else {
                return .failure(URLError(.fileDoesNotExist))
            }
            return .success((notFoundResponse, Data("404 VFS Resource Not Found".utf8)))
        } catch {
            return .failure(error)
        }
    }

    private nonisolated static func buildStreamingResponse(
        url: URL,
        statusCode: Int,
        data: Data,
        mimeType: String,
        contentRange: String?
    ) -> Result<(HTTPURLResponse, Data), Error> {
        var headers: [String: String] = [
            "Content-Type": mimeType,
            "Content-Length": "\(data.count)",
            "Accept-Ranges": "bytes",
            "Access-Control-Allow-Origin": "*",
            "Cache-Control": "max-age=3600, immutable"
        ]
        if let contentRange {
            headers["Content-Range"] = contentRange
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
    }

    public func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {
        let taskId = ObjectIdentifier(urlSchemeTask)
        stoppedTasks.insert(taskId)
        if let runningTask = activeTasks.removeValue(forKey: taskId) {
            runningTask.cancel()
        }
    }

    private nonisolated func parseRangeHeader(_ header: String?) -> ClosedRange<Int>? {
        guard let header = header, header.hasPrefix("bytes=") else { return nil }
        let spec = header.dropFirst(6).trimmingCharacters(in: .whitespaces)
        let parts = spec.split(separator: "-", omittingEmptySubsequences: false)
        guard parts.count == 2 else { return nil }

        if let start = Int(parts[0]), let end = Int(parts[1]), start <= end {
            return start...end
        } else if let start = Int(parts[0]), parts[1].isEmpty {
            return start...max(start, Int.max - 1)
        }
        return nil
    }
}
#endif
