// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore
#if canImport(WebKit)
import WebKit
#endif

// MARK: - Mock VFS Provider for Scheme Handler Testing

private final class MockVfsResourceProvider: TTZipVfsResourceProvider, @unchecked Sendable {
    var storedResources: [String: (data: Data, mimeType: String)] = [:]

    func loadResource(uri: String) async throws -> (data: Data, mimeType: String)? {
        return storedResources[uri]
    }

    func loadResourceRange(uri: String, byteRange: ClosedRange<Int>?) async throws -> (data: Data, fullSize: Int64, mimeType: String)? {
        guard let entry = storedResources[uri] else { return nil }
        let fullSize = Int64(entry.data.count)

        guard let range = byteRange else {
            return (entry.data, fullSize, entry.mimeType)
        }

        let start = max(0, range.lowerBound)
        let end = min(entry.data.count - 1, range.upperBound)
        guard start <= end else {
            return (Data(), fullSize, entry.mimeType)
        }

        let subdata = entry.data.subdata(in: start..<(end + 1))
        return (subdata, fullSize, entry.mimeType)
    }
}

// MARK: - Mock WKURLSchemeTask for Scheme Handler Verification

#if canImport(WebKit)
private final class MockSchemeTask: NSObject, WKURLSchemeTask, @unchecked Sendable {
    let request: URLRequest
    var receivedResponse: URLResponse?
    var receivedData = Data()
    var isFinished = false
    var errorReceived: Error?

    init(url: URL, headers: [String: String] = [:]) {
        var req = URLRequest(url: url)
        for (k, v) in headers {
            req.setValue(v, forHTTPHeaderField: k)
        }
        self.request = req
        super.init()
    }

    func didReceive(_ response: URLResponse) {
        self.receivedResponse = response
    }

    func didReceive(_ data: Data) {
        self.receivedData.append(data)
    }

    func didFinish() {
        self.isFinished = true
    }

    func didFailWithError(_ error: Error) {
        self.errorReceived = error
    }
}
#endif

// MARK: - Test Suite

final class TTZipHtmlPreviewServiceTests: XCTestCase {

    private var sandbox: IsolatedTempSandbox!
    private let service = TTZipHtmlPreviewService.shared

    override func setUp() async throws {
        try await super.setUp()
        sandbox = try IsolatedTempSandbox(prefix: "HtmlTest")
        service.clearCache()
    }

    override func tearDown() async throws {
        service.clearCache()
        sandbox?.cleanup()
        sandbox = nil
        try await super.tearDown()
    }

    // MARK: - 1. Format Probing Tests

    func testHtmlFormatProbing() async throws {
        let htmlDoc = "<!DOCTYPE html><html><head><title>Test</title></head><body><h1>Hello</h1></body></html>"
        let htmlURL = sandbox.fileURL(named: "index.html")
        try htmlDoc.write(to: htmlURL, atomically: true, encoding: .utf8)

        let probedFile = try await service.probe(url: htmlURL)
        XCTAssertEqual(probedFile, .html)

        let probedMemory = try await service.probe(data: Data(htmlDoc.utf8), fileName: "index.html")
        XCTAssertEqual(probedMemory, TTZipHtmlFormat.html)

        let xhtmlDoc = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>X</title></head></html>"
        let probedXhtml = try await service.probe(data: Data(xhtmlDoc.utf8), fileName: "doc.xhtml")
        XCTAssertEqual(probedXhtml, TTZipHtmlFormat.xhtml)

        let svgDoc = "<svg viewBox=\"0 0 100 100\" xmlns=\"http://www.w3.org/2000/svg\"><circle cx=\"50\" cy=\"50\" r=\"40\"/></svg>"
        let probedSvg = try await service.probe(data: Data(svgDoc.utf8), fileName: "icon.svg")
        XCTAssertEqual(probedSvg, TTZipHtmlFormat.svg)

        let mhtmlDoc = "MIME-Version: 1.0\nContent-Type: multipart/related; boundary=\"----=_NextPart\"\n\n------=_NextPart\nContent-Type: text/html\n\n<h1>MHTML</h1>"
        let probedMhtml = try await service.probe(data: Data(mhtmlDoc.utf8), fileName: "page.mhtml")
        XCTAssertEqual(probedMhtml, TTZipHtmlFormat.mhtml)

        let fragDoc = "<div><p>Paragraph snippet</p><span>Detail</span></div>"
        let probedFrag = try await service.probe(data: Data(fragDoc.utf8), fileName: nil)
        XCTAssertEqual(probedFrag, TTZipHtmlFormat.htmlFragment)

        let unknownData = Data([0x00, 0x01, 0x02, 0x03, 0xFF, 0xFE])
        let probedUnknown = try await service.probe(data: unknownData, fileName: "binary.dat")
        XCTAssertEqual(probedUnknown, TTZipHtmlFormat.unknown)
    }

    // MARK: - 2. Resource Extraction Tests

    func testHtmlResourceExtraction() async throws {
        let markup = """
        <!DOCTYPE html>
        <html>
        <head>
            <title>Resource Extraction Test</title>
            <link rel="stylesheet" href="styles/theme.css">
            <link rel="icon" href="favicon.ico">
            <script src="scripts/app.js"></script>
        </head>
        <body>
            <img src="images/logo.png" alt="Logo">
            <audio src="media/audio.mp3"></audio>
            <video src="media/video.mp4" poster="images/poster.jpg"></video>
            <a href="https://apple.com">External Link</a>
        </body>
        </html>
        """

        let resources = try await service.extractResources(html: markup)
        XCTAssertGreaterThanOrEqual(resources.count, 6)

        XCTAssertTrue(resources.contains { $0.tagName == "img" && $0.originalUri == "images/logo.png" && $0.resourceType == "image" })
        XCTAssertTrue(resources.contains { $0.tagName == "link" && $0.originalUri == "styles/theme.css" && $0.resourceType == "stylesheet" })
        XCTAssertTrue(resources.contains { $0.tagName == "script" && $0.originalUri == "scripts/app.js" && $0.resourceType == "script" })
        XCTAssertTrue(resources.contains { $0.tagName == "audio" && $0.originalUri == "media/audio.mp3" && $0.resourceType == "audio" })
        XCTAssertTrue(resources.contains { $0.tagName == "video" && $0.originalUri == "media/video.mp4" && $0.resourceType == "video" })
        XCTAssertTrue(resources.contains { $0.tagName == "a" && $0.originalUri == "https://apple.com" && $0.isExternal })
    }

    // MARK: - 3. VFS Transformation & Sanitization Tests

    func testHtmlVfsTransformationAndSanitization() async throws {
        let rawMarkup = """
        <!DOCTYPE html>
        <html>
        <head>
            <meta charset="utf-8">
            <title>TTZip HTML Preview</title>
            <link rel="stylesheet" href="assets/style.css">
            <script>alert('dangerous script')</script>
        </head>
        <body onload="doEvil()">
            <h1>Archive Documentation</h1>
            <p>Welcome to high-performance archiving.</p>
            <img src="./figures/arch.png" alt="Architecture">
            <iframe src="http://phishing.com"></iframe>
        </body>
        </html>
        """

        let result = try await service.transform(
            html: rawMarkup,
            baseVfsPrefix: "manual.zip/docs/index.html",
            policy: .defaultSafePreview
        )

        XCTAssertEqual(result.title, "TTZip HTML Preview")
        XCTAssertEqual(result.charset, "utf-8")
        XCTAssertTrue(result.hasScripts)
        XCTAssertGreaterThan(result.metricsChars, 0)
        XCTAssertGreaterThan(result.metricsWords, 0)

        // Verify scripts and dangerous attributes stripped
        XCTAssertFalse(result.transformedHtml.contains("<script"))
        XCTAssertFalse(result.transformedHtml.contains("onload="))
        XCTAssertFalse(result.transformedHtml.contains("<iframe"))

        // Verify VFS rewriting
        XCTAssertTrue(result.transformedHtml.contains("ttzip-vfs://manual.zip/docs/assets/style.css"))
        XCTAssertTrue(result.transformedHtml.contains("ttzip-vfs://manual.zip/docs/figures/arch.png"))
    }

    // MARK: - 4. Sanitization Only Tests

    func testHtmlSanitizeOnly() async throws {
        let maliciousHtml = "<p>Clean text</p><script>evil()</script><a href=\"javascript:void(0)\" onclick=\"hack()\">Click</a>"
        let sanitized = try await service.sanitize(html: maliciousHtml, policy: .defaultStrict)

        XCTAssertTrue(sanitized.contains("<p>Clean text</p>"))
        XCTAssertFalse(sanitized.contains("<script"))
        XCTAssertFalse(sanitized.contains("onclick"))
        XCTAssertFalse(sanitized.contains("evil()"))
    }

    // MARK: - 5. Tree-sitter Syntax Highlight Hot-Sync Tests

    func testTreeSitterSyntaxHighlightingHotSync() async throws {
        let sampleCode = "<div class=\"container\"><h1 id=\"title\">Header</h1></div>"
        let tokens = await service.highlightSource(code: sampleCode)

        XCTAssertFalse(tokens.isEmpty)
        XCTAssertTrue(tokens.contains { $0.category == .string || $0.category == .operator })

        let syncResult = try await service.syncPreview(sourceCode: sampleCode, baseVfsPrefix: "bundle.zip/")
        XCTAssertTrue(syncResult.transformedHtml.contains("<div class=\"container\">"))
    }

    // MARK: - 6. WebKit Scheme Handler Range Streaming Tests

    #if canImport(WebKit)
    @MainActor
    func testVfsSchemeHandlerPartialContentStreaming() async throws {
        let mockProvider = MockVfsResourceProvider()
        let sampleData = Data((0..<2048).map { UInt8($0 % 256) })
        let uri = "ttzip-vfs://archive.zip/media/video.mp4"
        mockProvider.storedResources[uri] = (sampleData, "video/mp4")

        let handler = TTZipVfsSchemeHandler(provider: mockProvider)
        guard let targetURL = URL(string: uri) else {
            XCTFail("Invalid URL")
            return
        }

        func waitForTask(_ task: MockSchemeTask) async throws {
            for _ in 0..<100 {
                if task.isFinished { return }
                try await Task.sleep(nanoseconds: 10_000_000)
            }
        }

        // Test 1: Full content (no Range header)
        let fullTask = MockSchemeTask(url: targetURL)
        handler.webView(WKWebView(), start: fullTask)
        try await waitForTask(fullTask)

        XCTAssertTrue(fullTask.isFinished)
        XCTAssertEqual(fullTask.receivedData.count, 2048)
        if let httpResp = fullTask.receivedResponse as? HTTPURLResponse {
            XCTAssertEqual(httpResp.statusCode, 200)
            XCTAssertEqual(httpResp.value(forHTTPHeaderField: "Content-Type"), "video/mp4")
            XCTAssertEqual(httpResp.value(forHTTPHeaderField: "Accept-Ranges"), "bytes")
        } else {
            XCTFail("Expected HTTPURLResponse")
        }

        // Test 2: Partial Content Range (bytes=100-299)
        let rangeTask = MockSchemeTask(url: targetURL, headers: ["Range": "bytes=100-299"])
        handler.webView(WKWebView(), start: rangeTask)
        try await waitForTask(rangeTask)

        XCTAssertTrue(rangeTask.isFinished)
        XCTAssertEqual(rangeTask.receivedData.count, 200)
        XCTAssertEqual(rangeTask.receivedData, sampleData.subdata(in: 100..<300))
        if let httpResp = rangeTask.receivedResponse as? HTTPURLResponse {
            XCTAssertEqual(httpResp.statusCode, 206)
            XCTAssertEqual(httpResp.value(forHTTPHeaderField: "Content-Range"), "bytes 100-299/2048")
            XCTAssertEqual(httpResp.value(forHTTPHeaderField: "Content-Length"), "200")
        } else {
            XCTFail("Expected HTTPURLResponse for partial content")
        }

        // Test 3: Not found 404
        let missingURL = URL(string: "ttzip-vfs://archive.zip/missing.png")!
        let missingTask = MockSchemeTask(url: missingURL)
        handler.webView(WKWebView(), start: missingTask)
        try await waitForTask(missingTask)
        XCTAssertTrue(missingTask.isFinished)
        if let httpResp = missingTask.receivedResponse as? HTTPURLResponse {
            XCTAssertEqual(httpResp.statusCode, 404)
        }
    }
    #endif

    // MARK: - 7. Cache Management Tests

    func testHtmlServiceCacheAndClear() async throws {
        let html = "<html><body>Cached Page</body></html>"
        let res1 = try await service.transform(html: html, baseVfsPrefix: "test.zip/")
        let res2 = try await service.transform(html: html, baseVfsPrefix: "test.zip/")

        XCTAssertEqual(res1.transformedHtml, res2.transformedHtml)
        service.clearCache()
        XCTAssertNil(service.lastTransformResult)
    }
}
