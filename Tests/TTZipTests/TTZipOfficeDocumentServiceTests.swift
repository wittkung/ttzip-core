// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipOfficeDocumentServiceTests: XCTestCase {

    private var sandbox: IsolatedTempSandbox!
    private let service = TTZipOfficeDocumentService.shared

    override func setUp() async throws {
        try await super.setUp()
        sandbox = try IsolatedTempSandbox(prefix: "OfficeTest")
        service.clearCache()
    }

    override func tearDown() async throws {
        service.clearCache()
        sandbox?.cleanup()
        sandbox = nil
        try await super.tearDown()
    }

    // MARK: - 1. Format Probing Tests

    func testOfficeFormatProbing() async throws {
        let xlsxURL = try await createSyntheticXlsx(named: "ProbeSample.xlsx")
        let xlsxData = try Data(contentsOf: xlsxURL)

        let probedXlsxFile = try await service.probe(url: xlsxURL)
        XCTAssertEqual(probedXlsxFile, .xlsx)

        let probedXlsxMem = try await service.probe(data: xlsxData, fileName: "test.xlsx")
        XCTAssertEqual(probedXlsxMem, .xlsx)

        let docxURL = try await createSyntheticDocx(named: "ProbeSample.docx")
        let docxData = try Data(contentsOf: docxURL)

        let probedDocxFile = try await service.probe(url: docxURL)
        XCTAssertEqual(probedDocxFile, .docx)

        let probedDocxMem = try await service.probe(data: docxData, fileName: "paper.docx")
        XCTAssertEqual(probedDocxMem, .docx)

        let unknownData = Data([0x00, 0x11, 0x22, 0x33, 0x44, 0x55])
        let probedUnknown = try await service.probe(data: unknownData, fileName: "data.bin")
        XCTAssertEqual(probedUnknown, .unknown)
    }

    // MARK: - 2. XLSX Sheet Names & Grid Data Extraction Tests

    func testXlsxSheetNamesAndSheetData() async throws {
        let xlsxURL = try await createSyntheticXlsx(named: "FinancialReport.xlsx")
        let xlsxData = try Data(contentsOf: xlsxURL)

        // 1. Sheet names
        let names = try await service.sheetNames(url: xlsxURL)
        XCTAssertEqual(names, ["Summary", "Q1_Expenses"])

        let memNames = try await service.sheetNames(data: xlsxData)
        XCTAssertEqual(memNames, ["Summary", "Q1_Expenses"])

        // 2. Sheet grid data from file URL
        let data = try await service.sheetData(url: xlsxURL, sheetNameOrIndex: "Summary")
        XCTAssertEqual(data.sheetName, "Summary")
        XCTAssertEqual(data.totalRows, 3)
        XCTAssertEqual(data.totalCols, 3)
        XCTAssertEqual(data.dimensionRef, "A1:C3")
        XCTAssertEqual(data.sharedStringsCount, 3)
        XCTAssertEqual(service.lastInspectedSheetData?.sheetName, "Summary")

        // Validate Row 1: A1 = "Revenue", B1 = 10000
        let r1 = data.rows[0]
        XCTAssertEqual(r1.rowNumber, 1)
        XCTAssertEqual(r1.cells[0].coordinate, "A1")
        XCTAssertEqual(r1.cells[0].value, .text("Revenue"))
        XCTAssertEqual(r1.cells[0].displayString, "Revenue")
        XCTAssertEqual(r1.cells[1].value, .number(10000.0))
        XCTAssertEqual(r1.cells[1].displayString, "10000")

        // Validate Row 3: A3 = "Net Profit", B3 = formula "B1-B2" with cached 6000, C3 = boolean TRUE
        let r3 = data.rows[2]
        XCTAssertEqual(r3.cells[0].value, .text("Net Profit"))
        XCTAssertEqual(r3.cells[1].value, .formula(expression: "B1-B2", cachedValue: "6000"))
        XCTAssertEqual(r3.cells[1].displayString, "6000")
        XCTAssertEqual(r3.cells[2].value, .boolean(true))
        XCTAssertEqual(r3.cells[2].displayString, "TRUE")

        // 3. Sheet grid data from memory buffer by index
        let sheet2Data = try await service.sheetData(data: xlsxData, sheetNameOrIndex: "2")
        XCTAssertEqual(sheet2Data.sheetName, "Q1_Expenses")
        XCTAssertEqual(sheet2Data.rows.count, 1)
    }

    // MARK: - 3. Dynamic Formula Evaluation Tests

    func testDynamicFormulaEvaluation() async throws {
        // 1. Standalone arithmetic expressions
        let res1 = try await service.evaluateFormula(formula: "=(10 + 20) * 3.5")
        XCTAssertEqual(res1, .number(105.0))
        XCTAssertEqual(res1.displayString, "105")

        let res2 = try await service.evaluateFormula(formula: "=100 / 4 - 5")
        XCTAssertEqual(res2, .number(20.0))

        let res3 = try await service.evaluateFormula(formula: "=2 ^ 4")
        XCTAssertEqual(res3, .number(16.0))

        // 2. Math functions: SUM, AVERAGE, MIN, MAX, COUNT
        let sumRes = try await service.evaluateFormula(formula: "=SUM(10, 20, 30, 40)")
        XCTAssertEqual(sumRes, .number(100.0))

        let avgRes = try await service.evaluateFormula(formula: "=AVERAGE(10, 20, 30)")
        XCTAssertEqual(avgRes, .number(20.0))

        let minRes = try await service.evaluateFormula(formula: "=MIN(15, 3, 99)")
        XCTAssertEqual(minRes, .number(3.0))

        let maxRes = try await service.evaluateFormula(formula: "=MAX(15, 3, 99)")
        XCTAssertEqual(maxRes, .number(99.0))

        let cntRes = try await service.evaluateFormula(formula: "=COUNT(1, 2, 3, 4, 5)")
        XCTAssertEqual(cntRes, .number(5.0))

        // 3. Logic: IF and CONCAT
        let ifTrue = try await service.evaluateFormula(formula: "=IF(10 > 5, 100, 200)")
        XCTAssertEqual(ifTrue, .number(100.0))

        let ifFalse = try await service.evaluateFormula(formula: "=IF(10 < 5, \"Yes\", \"No\")")
        XCTAssertEqual(ifFalse, .text("No"))

        let concatRes = try await service.evaluateFormula(formula: "=CONCAT(\"Hello \", \"TTZip!\")")
        XCTAssertEqual(concatRes, .text("Hello TTZip!"))

        // 4. Formula with context cells
        let contextCells = [
            TTZipCell(row: 1, col: 1, coordinate: "A1", value: .number(50.0)),
            TTZipCell(row: 2, col: 1, coordinate: "A2", value: .number(150.0)),
            TTZipCell(row: 1, col: 2, coordinate: "B1", value: .number(2.0)),
        ]

        let rangeSum = try await service.evaluateFormula(formula: "=SUM(A1:A2)", contextCells: contextCells)
        XCTAssertEqual(rangeSum, .number(200.0))

        let cellArith = try await service.evaluateFormula(formula: "=(A1 + A2) * B1", contextCells: contextCells)
        XCTAssertEqual(cellArith, .number(400.0))
    }

    // MARK: - 4. DOCX Document Extraction & Markdown Tests

    func testDocxDocumentExtractionAndMarkdown() async throws {
        let docxURL = try await createSyntheticDocx(named: "ArchitecturePaper.docx")
        let docxData = try Data(contentsOf: docxURL)

        // 1. Structured Document model from file URL
        let doc = try await service.docxDocument(url: docxURL)
        XCTAssertEqual(doc.title, "TTZip Systems Architecture")
        XCTAssertEqual(doc.paragraphs.count, 4)
        XCTAssertEqual(doc.paragraphs[0].text, "Introduction to Microkernel")
        XCTAssertEqual(doc.paragraphs[0].headingLevel, 1)
        XCTAssertTrue(doc.paragraphs[2].isListItem)

        // Table verification
        XCTAssertEqual(doc.tables.count, 1)
        XCTAssertEqual(doc.tables[0].totalRows, 2)
        XCTAssertEqual(doc.tables[0].headers, ["Component", "Throughput"])
        XCTAssertEqual(doc.tables[0].rows[1], ["Rust Core", "850 MB/s"])

        // Metrics
        XCTAssertGreaterThan(doc.totalWords, 0)
        XCTAssertGreaterThan(doc.totalCharacters, 0)
        XCTAssertEqual(service.lastInspectedDocx?.title, "TTZip Systems Architecture")

        // 2. Markdown output from memory buffer
        let md = try await service.docxToMarkdown(data: docxData)
        XCTAssertTrue(md.contains("# TTZip Systems Architecture"))
        XCTAssertTrue(md.contains("# Introduction to Microkernel"))
        XCTAssertTrue(md.contains("- First key benefit: low memory"))
        XCTAssertTrue(md.contains("| Component | Throughput |"))
        XCTAssertTrue(md.contains("| --- | --- |"))
        XCTAssertTrue(md.contains("| Rust Core | 850 MB/s |"))

        // 3. Markdown from file URL
        let fileMd = try await service.docxToMarkdown(url: docxURL)
        XCTAssertEqual(fileMd, md)
    }

    // MARK: - 5. Observable State & Actor Worker Tests

    func testObservableStateLifecycleAndWorker() async throws {
        let worker = TTZipOfficeDocumentWorker()
        let xlsxURL = try await createSyntheticXlsx(named: "WorkerSample.xlsx")

        let names = try await worker.extractSheetNames(at: xlsxURL.path)
        XCTAssertEqual(names.count, 2)

        let data = try await worker.extractSheetData(at: xlsxURL.path, sheetNameOrIndex: "Summary")
        XCTAssertEqual(data.sheetName, "Summary")

        // Service observable state checks
        XCTAssertFalse(service.isProcessing)
        XCTAssertEqual(service.activeOperationsCount, 0)

        service.clearCache()
        XCTAssertNil(service.lastInspectedSheetData)
        XCTAssertNil(service.lastInspectedDocx)
        XCTAssertNil(service.latestError)
    }

    // MARK: - 6. Error Handling Tests

    func testCorruptedOfficeArchiveErrorHandling() async {
        let corruptedData = Data([0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, 0xFF, 0xEE, 0xDD])

        do {
            _ = try await service.sheetNames(data: corruptedData)
            XCTFail("Corrupted XLSX data must throw an error")
        } catch {
            XCTAssertNotNil(error)
            XCTAssertNotNil(service.latestError)
        }

        do {
            _ = try await service.docxDocument(data: corruptedData)
            XCTFail("Corrupted DOCX data must throw an error")
        } catch {
            XCTAssertNotNil(error)
        }
    }

    // MARK: - Synthetic Fixture Generators

    private func createSyntheticXlsx(named name: String) async throws -> URL {
        let xlsxDir = sandbox.fileURL(named: "xlsx_build_\(UUID().uuidString)")
        let xlDir = xlsxDir.appendingPathComponent("xl")
        let wsDir = xlDir.appendingPathComponent("worksheets")

        try FileManager.default.createDirectory(at: wsDir, withIntermediateDirectories: true)

        let wbXml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
          <sheets>
            <sheet name="Summary" sheetId="1" r:id="rId1"/>
            <sheet name="Q1_Expenses" sheetId="2" r:id="rId2"/>
          </sheets>
        </workbook>
        """
        try wbXml.write(to: xlDir.appendingPathComponent("workbook.xml"), atomically: true, encoding: .utf8)

        let sstXml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3" uniqueCount="3">
          <si><t>Revenue</t></si>
          <si><t>Cost of Goods</t></si>
          <si><t>Net Profit</t></si>
        </sst>
        """
        try sstXml.write(to: xlDir.appendingPathComponent("sharedStrings.xml"), atomically: true, encoding: .utf8)

        let s1Xml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
          <dimension ref="A1:C3"/>
          <sheetData>
            <row r="1">
              <c r="A1" t="s"><v>0</v></c>
              <c r="B1"><v>10000</v></c>
            </row>
            <row r="2">
              <c r="A2" t="s"><v>1</v></c>
              <c r="B2"><v>4000</v></c>
            </row>
            <row r="3">
              <c r="A3" t="s"><v>2</v></c>
              <c r="B3"><f>B1-B2</f><v>6000</v></c>
              <c r="C3" t="b"><v>1</v></c>
            </row>
          </sheetData>
        </worksheet>
        """
        try s1Xml.write(to: wsDir.appendingPathComponent("sheet1.xml"), atomically: true, encoding: .utf8)

        let s2Xml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
          <dimension ref="A1:B2"/>
          <sheetData>
            <row r="1">
              <c r="A1"><v>250</v></c>
            </row>
          </sheetData>
        </worksheet>
        """
        try s2Xml.write(to: wsDir.appendingPathComponent("sheet2.xml"), atomically: true, encoding: .utf8)

        let outURL = sandbox.fileURL(named: name)
        let writer = ArchiveWriter()
        try await writer.createArchive(
            outputPath: outURL.path,
            format: .zip,
            level: .fast,
            inputPaths: [xlDir.path]
        )
        return outURL
    }

    private func createSyntheticDocx(named name: String) async throws -> URL {
        let docxDir = sandbox.fileURL(named: "docx_build_\(UUID().uuidString)")
        let docPropsDir = docxDir.appendingPathComponent("docProps")
        let wordDir = docxDir.appendingPathComponent("word")

        try FileManager.default.createDirectory(at: docPropsDir, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: wordDir, withIntermediateDirectories: true)

        let coreXml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/">
          <dc:title>TTZip Systems Architecture</dc:title>
          <dc:creator>Witt Kung</dc:creator>
        </cp:coreProperties>
        """
        try coreXml.write(to: docPropsDir.appendingPathComponent("core.xml"), atomically: true, encoding: .utf8)

        let docXml = """
        <?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
          <w:body>
            <w:p>
              <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
              <w:r><w:t>Introduction to Microkernel</w:t></w:r>
            </w:p>
            <w:p>
              <w:r><w:t>The TTZip engine utilizes zero-disk streaming architecture.</w:t></w:r>
            </w:p>
            <w:p>
              <w:pPr><w:numPr><w:ilvl w:val="0"/></w:numPr></w:pPr>
              <w:r><w:t>First key benefit: low memory</w:t></w:r>
            </w:p>
            <w:p>
              <w:pPr><w:numPr><w:ilvl w:val="0"/></w:numPr></w:pPr>
              <w:r><w:t>Second key benefit: high throughput</w:t></w:r>
            </w:p>
            <w:tbl>
              <w:tr>
                <w:tc><w:p><w:r><w:t>Component</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>Throughput</w:t></w:r></w:p></w:tc>
              </w:tr>
              <w:tr>
                <w:tc><w:p><w:r><w:t>Rust Core</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>850 MB/s</w:t></w:r></w:p></w:tc>
              </w:tr>
            </w:tbl>
          </w:body>
        </w:document>
        """
        try docXml.write(to: wordDir.appendingPathComponent("document.xml"), atomically: true, encoding: .utf8)

        let outURL = sandbox.fileURL(named: name)
        let writer = ArchiveWriter()
        try await writer.createArchive(
            outputPath: outURL.path,
            format: .zip,
            level: .fast,
            inputPaths: [
                docPropsDir.path,
                wordDir.path,
            ]
        )
        return outURL
    }
}
