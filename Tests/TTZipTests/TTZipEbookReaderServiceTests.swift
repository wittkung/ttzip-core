// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipEbookReaderServiceTests: XCTestCase {

    private var sandbox: IsolatedTempSandbox!
    private let service = TTZipEbookReaderService.shared

    override func setUp() async throws {
        try await super.setUp()
        sandbox = try IsolatedTempSandbox(prefix: "EbookTest")
        service.clearCache()
    }

    override func tearDown() async throws {
        service.clearCache()
        sandbox?.cleanup()
        sandbox = nil
        try await super.tearDown()
    }

    // MARK: - 1. Format Probing Tests

    func testEbookFormatProbing() async throws {
        let epubURL = try await createSyntheticEpub(named: "ProbeSample.epub")
        let epubData = try Data(contentsOf: epubURL)

        let probedFromFile = try await service.probe(url: epubURL)
        XCTAssertEqual(probedFromFile, .epub)

        let probedFromMemory = try await service.probe(data: epubData, fileName: "test.epub")
        XCTAssertEqual(probedFromMemory, .epub)

        let cbzURL = try await createSyntheticCbz(named: "ComicProbe.cbz")
        let probedCbz = try await service.probe(url: cbzURL)
        XCTAssertEqual(probedCbz, .cbz)

        let pdfData = Data("%PDF-1.7\n1 0 obj\n<<>>\nendobj\n%%EOF".utf8)
        let probedPdf = try await service.probe(data: pdfData, fileName: "doc.pdf")
        XCTAssertEqual(probedPdf, .pdf)

        let unknownData = Data([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77])
        let probedUnknown = try await service.probe(data: unknownData, fileName: "random.bin")
        XCTAssertEqual(probedUnknown, .unknown)
    }

    // MARK: - 2. Metadata Inspection Tests

    func testEbookMetadataExtractionFromMemoryAndFile() async throws {
        let epubURL = try await createSyntheticEpub(
            named: "ArchitectureGuide.epub",
            title: "TTZip High-Performance Systems Guide",
            authors: ["Witt Kung", "Co-Author Name"],
            publisher: "TTZip Architecture Press",
            language: "en"
        )
        let epubData = try Data(contentsOf: epubURL)

        // 1. Inspect in-memory Data
        let memMeta = try await service.inspect(data: epubData, fileName: "ArchitectureGuide.epub")
        XCTAssertEqual(memMeta.title, "TTZip High-Performance Systems Guide")
        XCTAssertEqual(memMeta.authors, ["Witt Kung", "Co-Author Name"])
        XCTAssertEqual(memMeta.publisher, "TTZip Architecture Press")
        XCTAssertEqual(memMeta.language, "en")
        XCTAssertEqual(memMeta.format, .epub)
        XCTAssertEqual(memMeta.totalChapters, 3)
        XCTAssertTrue(memMeta.hasCover)
        XCTAssertEqual(memMeta.coverPath, "EPUB/images/cover.jpg")
        XCTAssertGreaterThan(memMeta.fileSizeBytes, 0)
        XCTAssertEqual(service.lastInspectedMetadata?.title, "TTZip High-Performance Systems Guide")

        // 2. Inspect filesystem file URL
        let fileMeta = try await service.inspect(url: epubURL)
        XCTAssertEqual(fileMeta.title, "TTZip High-Performance Systems Guide")
        XCTAssertEqual(fileMeta.authors.count, 2)

        // 3. Cached inspection
        let cachedMeta = try await service.inspect(url: epubURL)
        XCTAssertEqual(cachedMeta.title, fileMeta.title)
    }

    // MARK: - 3. Hierarchical Table of Contents Tests

    func testEbookHierarchicalTocTree() async throws {
        let epubURL = try await createSyntheticEpub(named: "TocSample.epub")
        let epubData = try Data(contentsOf: epubURL)

        // 1. Extract from file URL
        let toc = try await service.tableOfContents(url: epubURL)
        XCTAssertEqual(toc.count, 2)
        XCTAssertEqual(toc[0].title, "1. Microkernel Architecture")
        XCTAssertEqual(toc[0].href, "EPUB/text/ch1.xhtml")
        XCTAssertEqual(toc[0].level, 0)
        XCTAssertEqual(toc[0].children.count, 1)

        // Nested sub-section
        let child = toc[0].children[0]
        XCTAssertEqual(child.title, "1.1 Memory Bounds")
        XCTAssertEqual(child.href, "EPUB/text/ch1.xhtml#bounds")
        XCTAssertEqual(child.level, 1)
        XCTAssertEqual(child.playOrder, 2)
        XCTAssertEqual(toc[0].totalDescendantsCount, 1)

        // Root section 2
        XCTAssertEqual(toc[1].title, "2. Streaming Pipelines")
        XCTAssertEqual(toc[1].href, "EPUB/text/ch2.xhtml")
        XCTAssertEqual(toc[1].children.count, 0)

        // 2. Extract from memory Data
        let memToc = try await service.tableOfContents(data: epubData)
        XCTAssertEqual(memToc.count, 2)
        XCTAssertEqual(service.lastInspectedToc.count, 2)
    }

    // MARK: - 4. Spine and Chapter Content Tests

    func testEbookSpineAndChapterReading() async throws {
        let epubURL = try await createSyntheticEpub(named: "SpineSample.epub")
        let epubData = try Data(contentsOf: epubURL)

        // 1. Reading Spine
        let spine = try await service.spine(url: epubURL)
        XCTAssertEqual(spine.count, 3)
        XCTAssertEqual(spine[0].id, "ch1")
        XCTAssertEqual(spine[0].href, "EPUB/text/ch1.xhtml")
        XCTAssertTrue(spine[0].isLinear)
        XCTAssertEqual(spine[1].id, "ch2")
        XCTAssertTrue(spine[1].isLinear)
        XCTAssertEqual(spine[2].id, "ch3")
        XCTAssertFalse(spine[2].isLinear)

        // 2. Chapter Extraction from file URL
        let ch1 = try await service.chapter(url: epubURL, href: "EPUB/text/ch1.xhtml")
        XCTAssertEqual(ch1.title, "1. Microkernel Architecture")
        XCTAssertTrue(ch1.contentString.contains("Memory bounds are strictly enforced"))
        XCTAssertGreaterThan(ch1.characterCount, 0)
        XCTAssertGreaterThan(ch1.wordCount, 0)

        // 3. Chapter Extraction from memory buffer
        let ch2 = try await service.chapter(data: epubData, href: "EPUB/text/ch2.xhtml")
        XCTAssertEqual(ch2.title, "2. Streaming Pipelines")
        XCTAssertTrue(ch2.contentString.contains("600 MB/s"))
        XCTAssertEqual(service.lastExtractedChapter?.title, "2. Streaming Pipelines")
    }

    // MARK: - 5. Resource and Cover Artwork Tests

    func testEbookResourceAndCoverExtraction() async throws {
        let epubURL = try await createSyntheticEpub(named: "ResourceSample.epub")
        let epubData = try Data(contentsOf: epubURL)

        // 1. Extract cover artwork
        let cover = try await service.cover(url: epubURL)
        XCTAssertNotNil(cover)
        XCTAssertEqual(cover?.href, "EPUB/images/cover.jpg")
        XCTAssertEqual(cover?.mediaType, "image/jpeg")
        XCTAssertGreaterThan(cover?.sizeBytes ?? 0, 0)

        // 2. Extract stylesheet resource
        let style = try await service.resource(data: epubData, href: "EPUB/styles/main.css")
        XCTAssertEqual(style.mediaType, "text/css")
        let cssText = String(data: style.data, encoding: .utf8)
        XCTAssertTrue(cssText?.contains("font-family") == true)
    }

    // MARK: - 6. CBZ Comic Book Tests

    func testCbzComicBookReadingPipeline() async throws {
        let cbzURL = try await createSyntheticCbz(named: "Superman.cbz")
        let cbzData = try Data(contentsOf: cbzURL)

        // 1. Metadata
        let meta = try await service.inspect(url: cbzURL)
        XCTAssertEqual(meta.title, "Superman")
        XCTAssertEqual(meta.format, .cbz)
        XCTAssertEqual(meta.totalChapters, 3)
        XCTAssertTrue(meta.hasCover)

        // 2. TOC
        let toc = try await service.tableOfContents(data: cbzData, fileName: "Superman.cbz")
        XCTAssertEqual(toc.count, 3)
        XCTAssertEqual(toc[0].title, "Page 1")
        XCTAssertEqual(toc[0].href, "001_page.jpg")

        // 3. Spine
        let spine = try await service.spine(url: cbzURL)
        XCTAssertEqual(spine.count, 3)
        XCTAssertEqual(spine[0].href, "001_page.jpg")

        // 4. Cover
        let cover = try await service.cover(url: cbzURL)
        XCTAssertNotNil(cover)
        XCTAssertEqual(cover?.href, "001_page.jpg")

        // 5. Chapter page
        let page = try await service.chapter(url: cbzURL, href: "001_page.jpg")
        XCTAssertTrue(page.contentString.contains("001_page.jpg"))
    }

    // MARK: - 7. Observable State and Worker Isolation Tests

    func testObservableStateLifecycleAndWorker() async throws {
        let worker = TTZipEbookReaderWorker()
        let epubURL = try await createSyntheticEpub(named: "WorkerSample.epub")

        let meta = try await worker.extractMetadata(at: epubURL.path)
        XCTAssertEqual(meta.title, "TTZip High-Performance Systems Guide")

        let toc = try await worker.extractToc(at: epubURL.path)
        XCTAssertEqual(toc.count, 2)

        let spine = try await worker.getSpine(at: epubURL.path)
        XCTAssertEqual(spine.count, 3)

        // Service observable state checks
        XCTAssertFalse(service.isProcessing)
        XCTAssertEqual(service.activeOperationsCount, 0)

        service.clearCache()
        XCTAssertNil(service.lastInspectedMetadata)
        XCTAssertTrue(service.lastInspectedToc.isEmpty)
        XCTAssertTrue(service.lastInspectedSpine.isEmpty)
        XCTAssertNil(service.lastExtractedChapter)
        XCTAssertNil(service.latestError)
    }

    // MARK: - 8. Error Resilience Tests

    func testCorruptedEbookErrorHandling() async {
        let corruptedData = Data([0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, 0xFF, 0xEE, 0xDD])

        do {
            _ = try await service.inspect(data: corruptedData)
            XCTFail("Corrupted ebook data must throw an error")
        } catch {
            XCTAssertNotNil(error)
            XCTAssertNotNil(service.latestError)
        }

        do {
            _ = try await service.tableOfContents(data: corruptedData)
            XCTFail("Corrupted ebook TOC must throw an error")
        } catch {
            XCTAssertNotNil(error)
        }
    }

    // MARK: - Helper Methods

    private func createSyntheticEpub(
        named name: String,
        title: String = "TTZip High-Performance Systems Guide",
        authors: [String] = ["Witt Kung", "Co-Author Name"],
        publisher: String = "TTZip Architecture Press",
        language: String = "en"
    ) async throws -> URL {
        let epubDir = sandbox.fileURL(named: "epub_build_\(UUID().uuidString)")
        let metaInfDir = epubDir.appendingPathComponent("META-INF")
        let oebpsDir = epubDir.appendingPathComponent("EPUB")
        let imgDir = oebpsDir.appendingPathComponent("images")
        let stylesDir = oebpsDir.appendingPathComponent("styles")
        let textDir = oebpsDir.appendingPathComponent("text")

        try FileManager.default.createDirectory(at: metaInfDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: imgDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: stylesDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: textDir, withIntermediateDirectories: true)

        // 1. mimetype
        try "application/epub+zip".write(to: epubDir.appendingPathComponent("mimetype"), atomically: true, encoding: .utf8)

        // 2. container.xml
        let containerXml = """
        <?xml version="1.0"?>
        <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
          <rootfiles>
            <rootfile full-path="EPUB/package.opf" media-type="application/oebps-package+xml"/>
          </rootfiles>
        </container>
        """
        try containerXml.write(to: metaInfDir.appendingPathComponent("container.xml"), atomically: true, encoding: .utf8)

        // 3. package.opf
        let creatorsXml = authors.map { "<dc:creator>\($0)</dc:creator>" }.joined(separator: "\n    ")
        let opfXml = """
        <?xml version="1.0" encoding="utf-8"?>
        <package version="3.0" xmlns="http://www.idpf.org/2007/opf" unique-identifier="pub-id">
          <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
            <dc:identifier id="pub-id">urn:uuid:12345-67890-abcdef</dc:identifier>
            <dc:title>\(title)</dc:title>
            \(creatorsXml)
            <dc:publisher>\(publisher)</dc:publisher>
            <dc:language>\(language)</dc:language>
            <dc:description>Comprehensive guide to zero-disk streaming archiving.</dc:description>
            <dc:date>2026-09-01</dc:date>
            <dc:rights>BSD-3-Clause OR Apache-2.0</dc:rights>
            <meta name="cover" content="cover-image-id"/>
            <meta property="dcterms:modified">2026-09-01T12:00:00Z</meta>
          </metadata>
          <manifest>
            <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
            <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
            <item id="cover-image-id" href="images/cover.jpg" media-type="image/jpeg" properties="cover-image"/>
            <item id="style" href="styles/main.css" media-type="text/css"/>
            <item id="ch1" href="text/ch1.xhtml" media-type="application/xhtml+xml"/>
            <item id="ch2" href="text/ch2.xhtml" media-type="application/xhtml+xml"/>
            <item id="ch3" href="text/ch3.xhtml" media-type="application/xhtml+xml"/>
          </manifest>
          <spine toc="ncx">
            <itemref idref="ch1" linear="yes"/>
            <itemref idref="ch2" linear="yes"/>
            <itemref idref="ch3" linear="no"/>
          </spine>
        </package>
        """
        try opfXml.write(to: oebpsDir.appendingPathComponent("package.opf"), atomically: true, encoding: .utf8)

        // 4. toc.ncx
        let ncxXml = """
        <?xml version="1.0" encoding="UTF-8"?>
        <ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
          <navMap>
            <navPoint id="np-1" playOrder="1">
              <navLabel><text>1. Microkernel Architecture</text></navLabel>
              <content src="text/ch1.xhtml"/>
              <navPoint id="np-1-1" playOrder="2">
                <navLabel><text>1.1 Memory Bounds</text></navLabel>
                <content src="text/ch1.xhtml#bounds"/>
              </navPoint>
            </navPoint>
            <navPoint id="np-2" playOrder="3">
              <navLabel><text>2. Streaming Pipelines</text></navLabel>
              <content src="text/ch2.xhtml"/>
            </navPoint>
          </navMap>
        </ncx>
        """
        try ncxXml.write(to: oebpsDir.appendingPathComponent("toc.ncx"), atomically: true, encoding: .utf8)

        // 5. nav.xhtml
        let navXml = """
        <?xml version="1.0" encoding="utf-8"?>
        <!DOCTYPE html>
        <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
        <head><title>TOC</title></head>
        <body>
          <nav epub:type="toc">
            <ol>
              <li><a href="text/ch1.xhtml">1. Microkernel Architecture</a></li>
              <li><a href="text/ch2.xhtml">2. Streaming Pipelines</a></li>
            </ol>
          </nav>
        </body>
        </html>
        """
        try navXml.write(to: oebpsDir.appendingPathComponent("nav.xhtml"), atomically: true, encoding: .utf8)

        // 6. Cover image
        let dummyJpg = Data([0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01, 0x00, 0x60, 0x00, 0x60, 0x00, 0x00, 0xFF, 0xDB])
        try dummyJpg.write(to: imgDir.appendingPathComponent("cover.jpg"))

        // 7. Style CSS
        try "body { font-family: -apple-system, sans-serif; margin: 2rem; }".write(
            to: stylesDir.appendingPathComponent("main.css"),
            atomically: true,
            encoding: .utf8
        )

        // 8. Chapters
        try "<!DOCTYPE html><html><head><title>Ch1</title></head><body><h1>1. Microkernel Architecture</h1><p id=\"bounds\">Memory bounds are strictly enforced to 64MB.</p></body></html>"
            .write(to: textDir.appendingPathComponent("ch1.xhtml"), atomically: true, encoding: .utf8)
        try "<!DOCTYPE html><html><head><title>Ch2</title></head><body><h1>2. Streaming Pipelines</h1><p>Throughput exceeds 600 MB/s.</p></body></html>"
            .write(to: textDir.appendingPathComponent("ch2.xhtml"), atomically: true, encoding: .utf8)
        try "<!DOCTYPE html><html><head><title>Ch3</title></head><body><h1>3. Appendix</h1><p>Extra technical notes.</p></body></html>"
            .write(to: textDir.appendingPathComponent("ch3.xhtml"), atomically: true, encoding: .utf8)

        // Archive into .epub
        let outEpubURL = sandbox.fileURL(named: name)
        let writer = ArchiveWriter()
        try await writer.createArchive(
            outputPath: outEpubURL.path,
            format: .zip,
            level: .fast,
            inputPaths: [
                epubDir.appendingPathComponent("mimetype").path,
                metaInfDir.path,
                oebpsDir.path,
            ]
        )

        return outEpubURL
    }

    private func createSyntheticCbz(named name: String) async throws -> URL {
        let cbzDir = sandbox.fileURL(named: "cbz_build_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: cbzDir, withIntermediateDirectories: true)

        let p1 = Data([0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x01])
        let p2 = Data([0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x02])
        let p3 = Data([0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x03])

        let f1 = cbzDir.appendingPathComponent("001_page.jpg")
        let f2 = cbzDir.appendingPathComponent("002_page.jpg")
        let f3 = cbzDir.appendingPathComponent("003_page.jpg")

        try p1.write(to: f1)
        try p2.write(to: f2)
        try p3.write(to: f3)

        let outCbzURL = sandbox.fileURL(named: name)
        let writer = ArchiveWriter()
        try await writer.createArchive(
            outputPath: outCbzURL.path,
            format: .zip,
            level: .fast,
            inputPaths: [f1.path, f2.path, f3.path]
        )

        return outCbzURL
    }
}
