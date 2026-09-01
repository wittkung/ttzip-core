// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipDocumentMetadataServiceTests: XCTestCase {

    private var sandbox: IsolatedTempSandbox!
    private let service = TTZipDocumentMetadataService.shared

    override func setUp() async throws {
        try await super.setUp()
        sandbox = try IsolatedTempSandbox(prefix: "DocMetaTest")
        service.clearCache()
    }

    override func tearDown() async throws {
        service.clearCache()
        sandbox?.cleanup()
        sandbox = nil
        try await super.tearDown()
    }

    // MARK: - 1. Format Kind Inference Tests

    func testDocumentKindInference() {
        XCTAssertEqual(TTZipDocumentKind.from(pathOrExtension: "document.docx"), .docx)
        XCTAssertEqual(TTZipDocumentKind.from(pathOrExtension: "finance.xlsx"), .xlsx)
        XCTAssertEqual(TTZipDocumentKind.from(pathOrExtension: "deck.pptx"), .pptx)
        XCTAssertEqual(TTZipDocumentKind.from(pathOrExtension: "novel.epub"), .epub)
        XCTAssertEqual(TTZipDocumentKind.from(pathOrExtension: "Info.plist"), .plist)
        XCTAssertEqual(TTZipDocumentKind.from(pathOrExtension: "spec.pdf"), .pdf)
        XCTAssertEqual(TTZipDocumentKind.from(pathOrExtension: "archive.zip"), .unknown)

        XCTAssertEqual(TTZipDocumentKind.docx.displayName, "Word Document (DOCX)")
        XCTAssertEqual(TTZipDocumentKind.xlsx.displayName, "Excel Spreadsheet (XLSX)")
        XCTAssertEqual(TTZipDocumentKind.pptx.displayName, "PowerPoint Presentation (PPTX)")
        XCTAssertEqual(TTZipDocumentKind.epub.displayName, "EPUB Digital Book")
        XCTAssertEqual(TTZipDocumentKind.plist.displayName, "Apple Property List")
    }

    // MARK: - 2. Plist XML Parsing Tests

    func testPlistXmlParsingSynchronousAndAsync() async throws {
        let samplePlist = """
        <?xml version="1.0" encoding="UTF-8"?>
        <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        <plist version="1.0">
        <dict>
            <key>CFBundleIdentifier</key>
            <string>com.ttzip.desktop</string>
            <key>CFBundleName</key>
            <string>TTZip Pro</string>
            <key>CFBundleVersion</key>
            <string>2048</string>
            <key>CFBundleShortVersionString</key>
            <string>2.5.0</string>
            <key>LSMinimumSystemVersion</key>
            <string>14.4</string>
            <key>CFBundleExecutable</key>
            <string>TTZipDesktopApp</string>
            <key>NSHumanReadableCopyright</key>
            <string>Copyright © 2026 Witt Kung</string>
            <key>NSSupportsAutomaticGraphicsSwitching</key>
            <true/>
        </dict>
        </plist>
        """

        // 1. Synchronous string parsing
        let syncPlist = try service.inspectPlist(xml: samplePlist)
        XCTAssertEqual(syncPlist.bundleIdentifier, "com.ttzip.desktop")
        XCTAssertEqual(syncPlist.bundleName, "TTZip Pro")
        XCTAssertEqual(syncPlist.bundleVersion, "2048")
        XCTAssertEqual(syncPlist.bundleShortVersion, "2.5.0")
        XCTAssertEqual(syncPlist.minimumOSVersion, "14.4")
        XCTAssertEqual(syncPlist.executableName, "TTZipDesktopApp")
        XCTAssertEqual(syncPlist.entries["NSHumanReadableCopyright"], "Copyright © 2026 Witt Kung")
        XCTAssertEqual(syncPlist.entries["NSSupportsAutomaticGraphicsSwitching"], "true")

        // 2. Asynchronous file URL parsing
        let plistURL = sandbox.fileURL(named: "Info.plist")
        try samplePlist.write(to: plistURL, atomically: true, encoding: .utf8)

        let filePlist = try await service.inspectPlist(url: plistURL)
        XCTAssertEqual(filePlist.bundleIdentifier, "com.ttzip.desktop")
        XCTAssertEqual(filePlist.bundleName, "TTZip Pro")
        XCTAssertEqual(filePlist.bundleVersion, "2048")
    }

    // MARK: - 3. DOCX Compound Document Tests

    func testDocxMetadataAndOutlineExtraction() async throws {
        let docXml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body>
                <w:p>
                    <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
                    <w:r><w:t>Chapter 1 · High Performance Archiving</w:t></w:r>
                </w:p>
                <w:p>
                    <w:r><w:t>TTZip provides zero-disk-landing streaming decompression and instant metadata inspection for macOS.</w:t></w:r>
                </w:p>
                <w:p>
                    <w:pPr><w:pStyle w:val="Heading2"/></w:pPr>
                    <w:r><w:t>Section 1.1 · UniFFI Architecture</w:t></w:r>
                </w:p>
                <w:p>
                    <w:r><w:t>The Mozilla UniFFI proc macro generates strict Swift 6 Sendable boundaries.</w:t></w:r>
                </w:p>
            </w:body>
        </w:document>
        """

        let coreXml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/">
            <dc:title>TTZip Technical Whitepaper</dc:title>
            <dc:creator>Witt Kung</dc:creator>
            <dc:subject>Systems Architecture</dc:subject>
            <dc:description>Comprehensive design specification for the TTZip engine.</dc:description>
            <cp:keywords>swift, rust, uniffi, compression</cp:keywords>
            <cp:lastModifiedBy>Witt Kung</cp:lastModifiedBy>
            <dcterms:created>2026-09-01T08:00:00Z</dcterms:created>
            <dcterms:modified>2026-09-01T12:00:00Z</dcterms:modified>
        </cp:coreProperties>
        """

        let appXml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
            <Application>TTZip Document Suite</Application>
            <Pages>12</Pages>
            <Words>2450</Words>
            <Characters>16800</Characters>
        </Properties>
        """

        let docxData = Self.createSyntheticZipArchive([
            "word/document.xml": docXml,
            "docProps/core.xml": coreXml,
            "docProps/app.xml": appXml
        ])

        let docxURL = sandbox.fileURL(named: "Whitepaper.docx")
        try docxData.write(to: docxURL)

        // Test metadata inspection
        let meta = try await service.inspect(url: docxURL)
        XCTAssertEqual(meta.formatName, "DOCX")
        XCTAssertEqual(meta.kind, .docx)
        XCTAssertEqual(meta.title, "TTZip Technical Whitepaper")
        XCTAssertEqual(meta.author, "Witt Kung")
        XCTAssertEqual(meta.subject, "Systems Architecture")
        XCTAssertEqual(meta.summary, "Comprehensive design specification for the TTZip engine.")
        XCTAssertEqual(meta.keywords, ["swift", "rust", "uniffi", "compression"])
        XCTAssertEqual(meta.lastModifiedBy, "Witt Kung")
        XCTAssertEqual(meta.application, "TTZip Document Suite")
        XCTAssertEqual(meta.pageCount, 12)
        XCTAssertEqual(meta.wordCount, 2450)
        XCTAssertEqual(meta.characterCount, 16800)

        // Test outline extraction
        let outline = try await service.outline(url: docxURL)
        XCTAssertEqual(outline.documentType, "Word Processing")
        XCTAssertTrue(outline.headings.contains("Chapter 1 · High Performance Archiving"))
        XCTAssertTrue(outline.headings.contains("Section 1.1 · UniFFI Architecture"))
        XCTAssertFalse(outline.summaryPreview.isEmpty)
    }

    // MARK: - 4. XLSX Spreadsheet Tests

    func testXlsxWorkbookMetadataAndSheetsExtraction() async throws {
        let wbXml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
            <sheets>
                <sheet name="Q1 Revenue" sheetId="1"/>
                <sheet name="Operational Costs" sheetId="2"/>
                <sheet name="Forecast 2027" sheetId="3"/>
            </sheets>
        </workbook>
        """

        let coreXml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/">
            <dc:title>Global Financial Model</dc:title>
            <dc:creator>Finance Analytics Team</dc:creator>
        </cp:coreProperties>
        """

        let appXml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
            <Application>Microsoft Excel</Application>
            <TitlesOfParts>
                <vt:vector size="3" baseType="lpstr">
                    <vt:lpstr>Q1 Revenue</vt:lpstr>
                    <vt:lpstr>Operational Costs</vt:lpstr>
                    <vt:lpstr>Forecast 2027</vt:lpstr>
                </vt:vector>
            </TitlesOfParts>
        </Properties>
        """

        let xlsxData = Self.createSyntheticZipArchive([
            "xl/workbook.xml": wbXml,
            "docProps/core.xml": coreXml,
            "docProps/app.xml": appXml
        ])

        let xlsxURL = sandbox.fileURL(named: "Financials.xlsx")
        try xlsxData.write(to: xlsxURL)

        let meta = try await service.inspect(url: xlsxURL)
        XCTAssertEqual(meta.formatName, "XLSX")
        XCTAssertEqual(meta.kind, .xlsx)
        XCTAssertEqual(meta.title, "Global Financial Model")
        XCTAssertEqual(meta.author, "Finance Analytics Team")
        XCTAssertEqual(meta.sheetCount, 3)
        XCTAssertEqual(meta.sheetNames, ["Q1 Revenue", "Operational Costs", "Forecast 2027"])

        let outline = try await service.outline(url: xlsxURL)
        XCTAssertEqual(outline.documentType, "Spreadsheet")
        XCTAssertEqual(outline.sheets, ["Q1 Revenue", "Operational Costs", "Forecast 2027"])
        XCTAssertEqual(outline.totalSections, 3)
    }

    // MARK: - 5. PPTX Presentation Tests

    func testPptxPresentationMetadataAndSlidesExtraction() async throws {
        let slide1Xml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
            <p:cSld>
                <p:spTree>
                    <p:sp>
                        <p:txBody>
                            <a:p><a:r><a:t>Keynote Address: The Future of Native Archiving</a:t></a:r></a:p>
                        </p:txBody>
                    </p:sp>
                </p:spTree>
            </p:cSld>
        </p:sld>
        """

        let slide2Xml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
            <p:cSld>
                <p:spTree>
                    <p:sp>
                        <p:txBody>
                            <a:p><a:r><a:t>Zero-Copy Memory Mapping &amp; POSIX Microkernel</a:t></a:r></a:p>
                        </p:txBody>
                    </p:sp>
                </p:spTree>
            </p:cSld>
        </p:sld>
        """

        let coreXml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/">
            <dc:title>TTZip 2026 Developer Keynote</dc:title>
            <dc:creator>Witt Kung</dc:creator>
        </cp:coreProperties>
        """

        let appXml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
            <Application>Microsoft PowerPoint</Application>
            <Slides>2</Slides>
        </Properties>
        """

        let pptxData = Self.createSyntheticZipArchive([
            "ppt/presentation.xml": "<p:presentation/>",
            "ppt/slides/slide1.xml": slide1Xml,
            "ppt/slides/slide2.xml": slide2Xml,
            "docProps/core.xml": coreXml,
            "docProps/app.xml": appXml
        ])

        let pptxURL = sandbox.fileURL(named: "Keynote.pptx")
        try pptxData.write(to: pptxURL)

        let meta = try await service.inspect(url: pptxURL)
        XCTAssertEqual(meta.formatName, "PPTX")
        XCTAssertEqual(meta.kind, .pptx)
        XCTAssertEqual(meta.title, "TTZip 2026 Developer Keynote")
        XCTAssertEqual(meta.slideCount, 2)
        XCTAssertTrue(meta.slideTitles.contains(where: { $0.contains("Keynote Address") }))

        let outline = try await service.outline(url: pptxURL)
        XCTAssertEqual(outline.documentType, "Presentation")
        XCTAssertEqual(outline.slides.count, 2)
    }

    // MARK: - 6. EPUB Digital Publication Tests

    func testEpubContainerMetadataExtraction() async throws {
        let containerXml = """
        <?xml version="1.0" encoding="UTF-8"?>
        <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
            <rootfiles>
                <rootfile full-path="OEBPS/package.opf" media-type="application/oebps-package+xml"/>
            </rootfiles>
        </container>
        """

        let opfXml = """
        <?xml version="1.0" encoding="utf-8"?>
        <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
            <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                <dc:title>High Performance Swift 6 Systems</dc:title>
                <dc:creator>Witt Kung</dc:creator>
                <dc:publisher>TTZip Press International</dc:publisher>
                <dc:language>en-US</dc:language>
                <dc:identifier>urn:isbn:9781234567890</dc:identifier>
                <dc:description>In-depth architectural patterns for native macOS applications.</dc:description>
                <dc:date>2026-09-01</dc:date>
                <dc:rights>Copyright © 2026 TTZip</dc:rights>
            </metadata>
        </package>
        """

        let epubData = Self.createSyntheticZipArchive([
            "META-INF/container.xml": containerXml,
            "OEBPS/package.opf": opfXml
        ])

        let epubURL = sandbox.fileURL(named: "Book.epub")
        try epubData.write(to: epubURL)

        let pub = try await service.inspectEpub(url: epubURL)
        XCTAssertEqual(pub.title, "High Performance Swift 6 Systems")
        XCTAssertEqual(pub.authors, ["Witt Kung"])
        XCTAssertEqual(pub.publisher, "TTZip Press International")
        XCTAssertEqual(pub.language, "en-US")
        XCTAssertEqual(pub.identifier, "urn:isbn:9781234567890")
        XCTAssertEqual(pub.synopsis, "In-depth architectural patterns for native macOS applications.")
        XCTAssertEqual(pub.publicationDate, "2026-09-01")
        XCTAssertEqual(pub.rights, "Copyright © 2026 TTZip")
    }

    // MARK: - 7. Observable Metrics and Caching Tests

    func testServiceObservableMetricsAndCaching() async throws {
        let plistXml = """
        <?xml version="1.0" encoding="UTF-8"?>
        <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        <plist version="1.0">
        <dict><key>CFBundleIdentifier</key><string>com.test.app</string></dict>
        </plist>
        """
        let plistURL = sandbox.fileURL(named: "Cached.plist")
        try plistXml.write(to: plistURL, atomically: true, encoding: .utf8)

        _ = try await service.inspectPlist(url: plistURL)
        XCTAssertGreaterThan(service.totalDocumentsInspected, 0)

        // Clear cache and verify
        service.clearCache()
        XCTAssertFalse(service.isProcessing)
        XCTAssertEqual(service.activeOperationsCount, 0)
    }

    // MARK: - 8. Corrupted File Resilience Tests

    func testCorruptedAndInvalidArchiveResilience() async {
        let corruptedData = Data([0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03])

        do {
            _ = try await service.inspect(data: corruptedData)
            XCTFail("Corrupted data should fail gracefully")
        } catch {
            XCTAssertNotNil(error)
        }

        do {
            _ = try await service.inspectEpub(data: corruptedData)
            XCTFail("Corrupted EPUB data should fail gracefully")
        } catch {
            XCTAssertNotNil(error)
        }
    }

    // MARK: - Helper: Pure Swift Synthetic ZIP Archive Builder

    private static func createSyntheticZipArchive(_ entries: [String: String]) -> Data {
        var zipBuffer = Data()
        var centralDirectory = Data()
        var centralDirectoryEntriesCount: UInt16 = 0

        for (filename, content) in entries {
            let filenameBytes = Data(filename.utf8)
            let contentBytes = Data(content.utf8)
            let uncompressedSize = UInt32(contentBytes.count)
            let compressedSize = UInt32(contentBytes.count)
            let crc = calculateCRC32(contentBytes)
            let localHeaderOffset = UInt32(zipBuffer.count)

            // Local File Header
            zipBuffer.append(contentsOf: [0x50, 0x4B, 0x03, 0x04]) // Signature
            zipBuffer.append(contentsOf: [0x14, 0x00]) // Version needed (2.0)
            zipBuffer.append(contentsOf: [0x00, 0x00]) // Flags
            zipBuffer.append(contentsOf: [0x00, 0x00]) // Compression method (0 = stored)
            zipBuffer.append(contentsOf: [0x00, 0x00, 0x00, 0x00]) // Mod time & date
            zipBuffer.append(contentsOf: withUnsafeBytes(of: crc.littleEndian, Array.init))
            zipBuffer.append(contentsOf: withUnsafeBytes(of: compressedSize.littleEndian, Array.init))
            zipBuffer.append(contentsOf: withUnsafeBytes(of: uncompressedSize.littleEndian, Array.init))
            zipBuffer.append(contentsOf: withUnsafeBytes(of: UInt16(filenameBytes.count).littleEndian, Array.init))
            zipBuffer.append(contentsOf: [0x00, 0x00]) // Extra field length
            zipBuffer.append(filenameBytes)
            zipBuffer.append(contentBytes)

            // Central Directory Header
            centralDirectory.append(contentsOf: [0x50, 0x4B, 0x01, 0x02]) // Signature
            centralDirectory.append(contentsOf: [0x14, 0x00]) // Version made by
            centralDirectory.append(contentsOf: [0x14, 0x00]) // Version needed
            centralDirectory.append(contentsOf: [0x00, 0x00]) // Flags
            centralDirectory.append(contentsOf: [0x00, 0x00]) // Compression method
            centralDirectory.append(contentsOf: [0x00, 0x00, 0x00, 0x00]) // Mod time & date
            centralDirectory.append(contentsOf: withUnsafeBytes(of: crc.littleEndian, Array.init))
            centralDirectory.append(contentsOf: withUnsafeBytes(of: compressedSize.littleEndian, Array.init))
            centralDirectory.append(contentsOf: withUnsafeBytes(of: uncompressedSize.littleEndian, Array.init))
            centralDirectory.append(contentsOf: withUnsafeBytes(of: UInt16(filenameBytes.count).littleEndian, Array.init))
            centralDirectory.append(contentsOf: [0x00, 0x00]) // Extra field length
            centralDirectory.append(contentsOf: [0x00, 0x00]) // Comment length
            centralDirectory.append(contentsOf: [0x00, 0x00]) // Disk start
            centralDirectory.append(contentsOf: [0x00, 0x00]) // Internal attributes
            centralDirectory.append(contentsOf: [0x00, 0x00, 0x00, 0x00]) // External attributes
            centralDirectory.append(contentsOf: withUnsafeBytes(of: localHeaderOffset.littleEndian, Array.init))
            centralDirectory.append(filenameBytes)

            centralDirectoryEntriesCount += 1
        }

        let centralDirectoryOffset = UInt32(zipBuffer.count)
        let centralDirectorySize = UInt32(centralDirectory.count)
        zipBuffer.append(centralDirectory)

        // End of Central Directory (EOCD)
        zipBuffer.append(contentsOf: [0x50, 0x4B, 0x05, 0x06]) // Signature
        zipBuffer.append(contentsOf: [0x00, 0x00]) // Disk number
        zipBuffer.append(contentsOf: [0x00, 0x00]) // Central dir disk
        zipBuffer.append(contentsOf: withUnsafeBytes(of: centralDirectoryEntriesCount.littleEndian, Array.init))
        zipBuffer.append(contentsOf: withUnsafeBytes(of: centralDirectoryEntriesCount.littleEndian, Array.init))
        zipBuffer.append(contentsOf: withUnsafeBytes(of: centralDirectorySize.littleEndian, Array.init))
        zipBuffer.append(contentsOf: withUnsafeBytes(of: centralDirectoryOffset.littleEndian, Array.init))
        zipBuffer.append(contentsOf: [0x00, 0x00]) // Comment length

        return zipBuffer
    }

    private static func calculateCRC32(_ data: Data) -> UInt32 {
        var crc: UInt32 = 0xFFFF_FFFF
        for byte in data {
            var c = UInt32(byte) ^ (crc & 0xFF)
            for _ in 0..<8 {
                c = (c & 1 != 0) ? (0xEDB8_8320 ^ (c >> 1)) : (c >> 1)
            }
            crc = c ^ (crc >> 8)
        }
        return ~crc
    }
}
