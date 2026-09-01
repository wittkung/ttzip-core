// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipPdfDocumentServiceTests: XCTestCase {

    private var sandbox: IsolatedTempSandbox!
    private let service = TTZipPdfDocumentService.shared

    override func setUp() async throws {
        try await super.setUp()
        sandbox = try IsolatedTempSandbox(prefix: "PdfDocTest")
        service.clearCache()
    }

    override func tearDown() async throws {
        service.clearCache()
        sandbox?.cleanup()
        sandbox = nil
        try await super.tearDown()
    }

    // MARK: - 1. Metadata Inspection Tests

    func testPdfMetadataInspectionFromMemoryAndFile() async throws {
        let pdfData = Self.createSyntheticPdf(
            title: "TTZip PDF Architecture",
            author: "Witt Kung",
            subject: "Swift 6 Concurrency & UniFFI",
            keywords: "rust, swift, pdf, metadata",
            bodyText: "High performance zero-disk-landing PDF inspection engine."
        )

        // 1. Inspect in-memory bytes
        let memMeta = try await service.inspect(data: pdfData)
        XCTAssertEqual(memMeta.formatVersion, "PDF-1.7")
        XCTAssertEqual(memMeta.pageCount, 1)
        XCTAssertEqual(memMeta.title, "TTZip PDF Architecture")
        XCTAssertEqual(memMeta.author, "Witt Kung")
        XCTAssertEqual(memMeta.subject, "Swift 6 Concurrency & UniFFI")
        XCTAssertEqual(memMeta.keywords, ["rust", "swift", "pdf", "metadata"])
        XCTAssertFalse(memMeta.isEncrypted)
        XCTAssertTrue(memMeta.hasOutline)
        XCTAssertGreaterThan(memMeta.fileSizeBytes, 0)

        // 2. Inspect filesystem file URL with caching
        let pdfURL = sandbox.fileURL(named: "Architecture.pdf")
        try pdfData.write(to: pdfURL)

        let fileMeta = try await service.inspect(url: pdfURL)
        XCTAssertEqual(fileMeta.title, "TTZip PDF Architecture")
        XCTAssertEqual(fileMeta.author, "Witt Kung")

        // 3. Second inspect hits in-memory cache
        let cachedMeta = try await service.inspect(url: pdfURL)
        XCTAssertEqual(cachedMeta.title, fileMeta.title)
        XCTAssertEqual(service.lastInspectedMetadata?.title, "TTZip PDF Architecture")
    }

    // MARK: - 2. Outline Bookmark Hierarchy Tests

    func testPdfOutlineExtraction() async throws {
        let pdfData = Self.createSyntheticPdfWithMultiLevelOutline()
        let pdfURL = sandbox.fileURL(named: "DocumentWithOutline.pdf")
        try pdfData.write(to: pdfURL)

        // 1. Extract from file URL
        let outlines = try await service.outline(url: pdfURL)
        XCTAssertEqual(outlines.count, 2)
        XCTAssertEqual(outlines[0].title, "1. Introduction")
        XCTAssertEqual(outlines[0].pageNumber, 1)
        XCTAssertEqual(outlines[0].children.count, 0)

        XCTAssertEqual(outlines[1].title, "2. Deep Dive")
        XCTAssertEqual(outlines[1].pageNumber, 2)
        XCTAssertEqual(outlines[1].children.count, 1)
        XCTAssertEqual(outlines[1].children[0].title, "2.1 Microkernel Details")
        XCTAssertEqual(outlines[1].children[0].pageNumber, 2)
        XCTAssertEqual(outlines[1].totalDescendantsCount, 1)

        // 2. Extract from memory Data
        let memOutlines = try await service.outline(data: pdfData)
        XCTAssertEqual(memOutlines.count, 2)
        XCTAssertEqual(memOutlines[1].children.count, 1)
    }

    // MARK: - 3. Page Text and Multi-page Extraction Tests

    func testPdfPageTextAndAllPagesExtraction() async throws {
        let pdfData = Self.createSyntheticPdfWithMultiLevelOutline()
        let pdfURL = sandbox.fileURL(named: "MultiPage.pdf")
        try pdfData.write(to: pdfURL)

        // 1. Single page text from file URL
        let page1 = try await service.pageText(url: pdfURL, pageNumber: 1)
        XCTAssertEqual(page1.pageNumber, 1)
        XCTAssertTrue(page1.text.contains("Page 1 Content"))
        XCTAssertGreaterThan(page1.characterCount, 0)
        XCTAssertGreaterThan(page1.wordCount, 0)

        // 2. Single page text from memory buffer
        let page2 = try await service.pageText(data: pdfData, pageNumber: 2)
        XCTAssertEqual(page2.pageNumber, 2)
        XCTAssertTrue(page2.text.contains("Page 2 Content"))

        // 3. All pages extraction
        let allPages = try await service.allPagesText(url: pdfURL)
        XCTAssertEqual(allPages.count, 2)
        XCTAssertEqual(allPages[0].pageNumber, 1)
        XCTAssertEqual(allPages[1].pageNumber, 2)

        // 4. Max pages constraint
        let limitedPages = try await service.allPagesText(data: pdfData, maxPages: 1)
        XCTAssertEqual(limitedPages.count, 1)
    }

    // MARK: - 4. Full-Text Search Tests

    func testPdfFullTextSearch() async throws {
        let pdfData = Self.createSyntheticPdfWithMultiLevelOutline()
        let pdfURL = sandbox.fileURL(named: "Searchable.pdf")
        try pdfData.write(to: pdfURL)

        // 1. Case-insensitive search
        let results = try await service.search(url: pdfURL, query: "content", maxResults: 10, caseSensitive: false)
        XCTAssertEqual(results.count, 2)
        XCTAssertEqual(results[0].pageNumber, 1)
        XCTAssertEqual(results[1].pageNumber, 2)
        XCTAssertTrue(results[0].matchText.contains("Page 1 Content"))

        // 2. Case-sensitive search
        let caseResults = try await service.search(data: pdfData, query: "page 1", maxResults: 10, caseSensitive: true)
        XCTAssertEqual(caseResults.count, 0)

        let exactResults = try await service.search(data: pdfData, query: "Page 1", maxResults: 10, caseSensitive: true)
        XCTAssertEqual(exactResults.count, 1)
        XCTAssertEqual(exactResults[0].pageNumber, 1)

        // 3. Search results state property
        XCTAssertFalse(service.searchResults.isEmpty)
        service.clearSearchResults()
        XCTAssertTrue(service.searchResults.isEmpty)
    }

    // MARK: - 5. Observable State and Worker Isolation Tests

    func testObservableStateLifecycleAndWorker() async throws {
        let worker = TTZipPdfDocumentWorker()
        let pdfData = Self.createSyntheticPdf(bodyText: "Actor worker direct test payload.")

        let meta = try await worker.extractMetadata(from: pdfData)
        XCTAssertEqual(meta.title, "Test Document")

        let page = try await worker.extractPageText(from: pdfData, pageNumber: 1)
        XCTAssertTrue(page.text.contains("Actor worker direct test"))

        let search = try await worker.searchText(from: pdfData, query: "direct test", maxResults: 5, caseSensitive: false)
        XCTAssertEqual(search.count, 1)

        // Verify service state metrics
        XCTAssertFalse(service.isProcessing)
        XCTAssertEqual(service.activeOperationsCount, 0)

        service.clearCache()
        XCTAssertNil(service.lastInspectedMetadata)
        XCTAssertTrue(service.lastInspectedOutline.isEmpty)
        XCTAssertNil(service.latestError)
    }

    // MARK: - 6. Error Handling & Resilience Tests

    func testCorruptedPdfHandling() async {
        let corruptedData = Data([0x25, 0x50, 0x44, 0x46, 0x00, 0xFF, 0xEE, 0xDD])

        do {
            _ = try await service.inspect(data: corruptedData)
            XCTFail("Corrupted PDF data must throw an error")
        } catch {
            XCTAssertNotNil(error)
            XCTAssertNotNil(service.latestError)
        }

        do {
            _ = try await service.pageText(data: corruptedData, pageNumber: 1)
            XCTFail("Corrupted PDF pageText must throw an error")
        } catch {
            XCTAssertNotNil(error)
        }
    }

    // MARK: - Helper Methods

    private static func createSyntheticPdf(
        title: String = "Test Document",
        author: String = "Test Author",
        subject: String = "Test Subject",
        keywords: String = "unit, test",
        bodyText: String = "Hello TTZip World"
    ) -> Data {
        var data = Data()
        data.append(Data("%PDF-1.7\n%\u{E2}\u{E3}\u{CF}\u{D3}\n".utf8))

        var offsets: [Int] = [0]

        // 1 0 obj: Catalog
        offsets.append(data.count)
        data.append(Data("1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Outlines 6 0 R >>\nendobj\n".utf8))

        // 2 0 obj: Pages
        offsets.append(data.count)
        data.append(Data("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".utf8))

        // 3 0 obj: Page
        offsets.append(data.count)
        data.append(Data("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n".utf8))

        // 4 0 obj: Content Stream
        let streamStr = "BT /F1 12 Tf 50 750 Td (\(bodyText)) Tj ET\n"
        let streamBytes = Data(streamStr.utf8)
        offsets.append(data.count)
        data.append(Data("4 0 obj\n<< /Length \(streamBytes.count) >>\nstream\n".utf8))
        data.append(streamBytes)
        data.append(Data("endstream\nendobj\n".utf8))

        // 5 0 obj: Font
        offsets.append(data.count)
        data.append(Data("5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".utf8))

        // 6 0 obj: Outlines
        offsets.append(data.count)
        data.append(Data("6 0 obj\n<< /Type /Outlines /First 7 0 R /Last 7 0 R /Count 1 >>\nendobj\n".utf8))

        // 7 0 obj: Outline Item 1
        offsets.append(data.count)
        data.append(Data("7 0 obj\n<< /Title (Chapter 1) /Parent 6 0 R /Dest [3 0 R /XYZ 0 842 0] >>\nendobj\n".utf8))

        // 8 0 obj: Info
        offsets.append(data.count)
        data.append(Data("8 0 obj\n<< /Title (\(title)) /Author (\(author)) /Subject (\(subject)) /Keywords (\(keywords)) /Creator (TTZip Unit Tests) >>\nendobj\n".utf8))

        // XRef table
        let xrefOffset = data.count
        data.append(Data("xref\n0 \(offsets.count)\n0000000000 65535 f \n".utf8))
        for i in 1..<offsets.count {
            let line = String(format: "%010d 00000 n \n", offsets[i])
            data.append(Data(line.utf8))
        }

        // Trailer
        data.append(Data("trailer\n<< /Size \(offsets.count) /Root 1 0 R /Info 8 0 R >>\nstartxref\n\(xrefOffset)\n%%EOF\n".utf8))

        return data
    }

    private static func createSyntheticPdfWithMultiLevelOutline() -> Data {
        var data = Data()
        data.append(Data("%PDF-1.7\n%\u{E2}\u{E3}\u{CF}\u{D3}\n".utf8))

        var offsets: [Int] = [0]

        // 1 0 obj: Catalog
        offsets.append(data.count)
        data.append(Data("1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Outlines 7 0 R >>\nendobj\n".utf8))

        // 2 0 obj: Pages
        offsets.append(data.count)
        data.append(Data("2 0 obj\n<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>\nendobj\n".utf8))

        // 3 0 obj: Page 1
        offsets.append(data.count)
        data.append(Data("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 5 0 R /Resources << /Font << /F1 9 0 R >> >> >>\nendobj\n".utf8))

        // 4 0 obj: Page 2
        offsets.append(data.count)
        data.append(Data("4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 6 0 R /Resources << /Font << /F1 9 0 R >> >> >>\nendobj\n".utf8))

        // 5 0 obj: Stream Page 1
        let s1 = "BT /F1 12 Tf 50 750 Td (Page 1 Content: Introduction to TTZip High-Performance PDF Engine) Tj ET\n"
        let s1Bytes = Data(s1.utf8)
        offsets.append(data.count)
        data.append(Data("5 0 obj\n<< /Length \(s1Bytes.count) >>\nstream\n".utf8))
        data.append(s1Bytes)
        data.append(Data("endstream\nendobj\n".utf8))

        // 6 0 obj: Stream Page 2
        let s2 = "BT /F1 12 Tf 50 750 Td (Page 2 Content: Architecture and Microkernel Stream Design Details) Tj ET\n"
        let s2Bytes = Data(s2.utf8)
        offsets.append(data.count)
        data.append(Data("6 0 obj\n<< /Length \(s2Bytes.count) >>\nstream\n".utf8))
        data.append(s2Bytes)
        data.append(Data("endstream\nendobj\n".utf8))

        // 7 0 obj: Outlines root
        offsets.append(data.count)
        data.append(Data("7 0 obj\n<< /Type /Outlines /First 8 0 R /Last 10 0 R /Count 2 >>\nendobj\n".utf8))

        // 8 0 obj: Outline Item 1 -> Page 1
        offsets.append(data.count)
        data.append(Data("8 0 obj\n<< /Title (1. Introduction) /Parent 7 0 R /Next 10 0 R /Dest [3 0 R /XYZ 0 842 0] /Count 0 >>\nendobj\n".utf8))

        // 9 0 obj: Font
        offsets.append(data.count)
        data.append(Data("9 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".utf8))

        // 10 0 obj: Outline Item 2 -> Page 2
        offsets.append(data.count)
        data.append(Data("10 0 obj\n<< /Title (2. Deep Dive) /Parent 7 0 R /Prev 8 0 R /First 11 0 R /Last 11 0 R /Dest [4 0 R /XYZ 0 842 0] /Count 1 >>\nendobj\n".utf8))

        // 11 0 obj: Outline Sub-item 2.1 -> Page 2
        offsets.append(data.count)
        data.append(Data("11 0 obj\n<< /Title (2.1 Microkernel Details) /Parent 10 0 R /Dest [4 0 R /XYZ 0 842 0] /Count 0 >>\nendobj\n".utf8))

        // 12 0 obj: Info
        offsets.append(data.count)
        data.append(Data("12 0 obj\n<< /Title (Multi-page Test Document) /Author (Witt Kung) >>\nendobj\n".utf8))

        // XRef table
        let xrefOffset = data.count
        data.append(Data("xref\n0 \(offsets.count)\n0000000000 65535 f \n".utf8))
        for i in 1..<offsets.count {
            let line = String(format: "%010d 00000 n \n", offsets[i])
            data.append(Data(line.utf8))
        }

        // Trailer
        data.append(Data("trailer\n<< /Size \(offsets.count) /Root 1 0 R /Info 12 0 R >>\nstartxref\n\(xrefOffset)\n%%EOF\n".utf8))

        return data
    }
}
