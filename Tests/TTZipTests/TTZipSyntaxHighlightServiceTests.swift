// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipSyntaxHighlightServiceTests: XCTestCase {
    var service: TTZipSyntaxHighlightService!

    override func setUp() {
        super.setUp()
        service = TTZipSyntaxHighlightService(maxCacheEntries: 64)
    }

    override func tearDown() {
        service = nil
        super.tearDown()
    }

    // MARK: - Language Detection Tests

    func testLanguageDetectionByExtension() {
        let rs = service.detectLanguage(filePathOrExtension: "engine.rs")
        XCTAssertEqual(rs.languageId, "rust")
        XCTAssertEqual(rs.displayName, "Rust")
        XCTAssertTrue(rs.isSupported)

        let swift = service.detectLanguage(filePathOrExtension: "Sources/Main.swift")
        XCTAssertEqual(swift.languageId, "swift")
        XCTAssertEqual(swift.displayName, "Swift")
        XCTAssertTrue(swift.isSupported)

        let py = service.detectLanguage(filePathOrExtension: ".py")
        XCTAssertEqual(py.languageId, "python")

        let json = service.detectLanguage(filePathOrExtension: "config.json")
        XCTAssertEqual(json.languageId, "json")

        let md = service.detectLanguage(filePathOrExtension: "README.md")
        XCTAssertEqual(md.languageId, "markdown")
    }

    func testLanguageDetectionBySpecialFilename() {
        let cargo = service.detectLanguage(filePathOrExtension: "Cargo.toml")
        XCTAssertEqual(cargo.languageId, "toml")

        let pkg = service.detectLanguage(filePathOrExtension: "Package.swift")
        XCTAssertEqual(pkg.languageId, "swift")

        let make = service.detectLanguage(filePathOrExtension: "Makefile")
        XCTAssertEqual(make.languageId, "makefile")
    }

    func testLanguageDetectionByShebang() {
        let pyScript = service.detectLanguage(filePathOrExtension: "myscript", firstLine: "#!/usr/bin/env python3")
        XCTAssertEqual(pyScript.languageId, "python")

        let shScript = service.detectLanguage(filePathOrExtension: "deploy", firstLine: "#!/bin/bash -e")
        XCTAssertEqual(shScript.languageId, "shell")
    }

    func testGetSupportedLanguages() {
        let list = service.getSupportedLanguages()
        XCTAssertFalse(list.isEmpty)
        XCTAssertTrue(list.contains(where: { $0.languageId == "rust" }))
        XCTAssertTrue(list.contains(where: { $0.languageId == "swift" }))
        XCTAssertTrue(list.contains(where: { $0.languageId == "python" }))
    }

    // MARK: - Highlighting Tests

    func testHighlightSwiftCode() async {
        let code = """
        // Swift Example
        import Foundation

        public struct ArchiveHeader {
            public let version: Int = 42
        }

        func processHeader() -> Bool {
            return true
        }
        """

        let tokens = await service.highlight(code: code, language: "swift")
        XCTAssertFalse(tokens.isEmpty)

        // Verify comments
        XCTAssertTrue(tokens.contains(where: { $0.category == .comment }))
        // Verify keywords (import, public, struct, let, func, return)
        XCTAssertTrue(tokens.contains(where: { $0.category == .keyword }))
        // Verify number (42)
        XCTAssertTrue(tokens.contains(where: { $0.category == .number }))

        // Verify NSRange metrics validity
        for token in tokens {
            XCTAssertGreaterThanOrEqual(token.location, 0)
            XCTAssertGreaterThan(token.length, 0)
            XCTAssertGreaterThanOrEqual(token.lineNumber, 1)
            XCTAssertGreaterThanOrEqual(token.column, 0)
            XCTAssertEqual(token.nsRange.location, token.location)
            XCTAssertEqual(token.nsRange.length, token.length)
        }
    }

    func testHighlightRustCode() async {
        let code = """
        pub fn compute_sum(a: i32, b: i32) -> i32 {
            /* calculate */
            a + b + 100
        }
        """

        let tokens = await service.highlight(code: code, language: "rs")
        XCTAssertFalse(tokens.isEmpty)
        XCTAssertTrue(tokens.contains(where: { $0.category == .keyword }))
        XCTAssertTrue(tokens.contains(where: { $0.category == .comment }))
        XCTAssertTrue(tokens.contains(where: { $0.category == .number }))
    }

    func testHighlightPythonCode() async {
        let code = """
        # Python Processor
        def calculate(val: int) -> str:
            return f"result: {val}"
        """

        let tokens = await service.highlight(code: code, language: "py")
        XCTAssertFalse(tokens.isEmpty)
        XCTAssertTrue(tokens.contains(where: { $0.category == .comment }))
        XCTAssertTrue(tokens.contains(where: { $0.category == .keyword }))
        XCTAssertTrue(tokens.contains(where: { $0.category == .string }))
    }

    func testHighlightViewportFiltering() async {
        let code = """
        line 1: let a = 1
        line 2: let b = 2
        line 3: let c = 3
        line 4: let d = 4
        line 5: let e = 5
        line 6: let f = 6
        line 7: let g = 7
        """

        let viewportTokens = await service.highlightViewport(code: code, language: "rs", startLine: 3, lineCount: 3)
        XCTAssertFalse(viewportTokens.isEmpty)

        for token in viewportTokens {
            XCTAssertGreaterThanOrEqual(token.lineNumber, 3)
            XCTAssertLessThan(token.lineNumber, 6)
        }
    }

    func testMaxLengthTruncation() async {
        let longCode = "let a = 1; let b = 2; let c = 3; let d = 4; let e = 5; let f = 6;"
        let limitedTokens = await service.highlight(code: longCode, language: "rs", maxLength: 15)
        let allTokens = await service.highlight(code: longCode, language: "rs", maxLength: 0)

        XCTAssertFalse(limitedTokens.isEmpty)
        XCTAssertLessThanOrEqual(limitedTokens.count, allTokens.count)
    }

    // MARK: - Symbol Extraction Tests

    func testExtractSwiftSymbols() async {
        let code = """
        public class EngineManager {
            public struct Config {
                let threads: Int
            }
        }

        public protocol ArchivingProtocol {
            func compress()
        }

        extension EngineManager {
            func reset() {}
        }
        """

        let symbols = await service.extractSymbols(code: code, language: "swift")
        XCTAssertFalse(symbols.isEmpty)

        XCTAssertTrue(symbols.contains(where: { $0.kind == .classDeclaration && $0.name.contains("EngineManager") }), "Symbols: \(symbols)")
        XCTAssertTrue(symbols.contains(where: { $0.kind == .protocolDeclaration && $0.name.contains("ArchivingProtocol") }), "Symbols: \(symbols)")
        XCTAssertTrue(symbols.contains(where: { ($0.kind == .implementation || $0.rawKind == "extension") && $0.name.contains("EngineManager") }), "Symbols: \(symbols)")
    }

    func testExtractRustSymbols() async {
        let code = """
        pub struct VfsTree {
            pub root: String,
        }

        impl VfsTree {
            pub fn new() -> Self {
                Self { root: String::new() }
            }
        }

        pub fn parse_vfs() -> bool {
            true
        }
        """

        let symbols = await service.extractSymbols(code: code, language: "rs")
        XCTAssertFalse(symbols.isEmpty)
        XCTAssertTrue(symbols.contains(where: { $0.kind == .structure && $0.name.contains("VfsTree") }))
        XCTAssertTrue(symbols.contains(where: { $0.kind == .function || $0.kind == .implementation }))
    }

    func testExtractMarkdownHeadings() async {
        let md = """
        # Main Overview
        Summary paragraph.

        ## Technical Architecture
        Architecture description.

        ### Microkernel Engine
        Details.
        """

        let symbols = await service.extractSymbols(code: md, language: "md")
        XCTAssertEqual(symbols.count, 3)
        XCTAssertEqual(symbols[0].name, "Main Overview")
        XCTAssertEqual(symbols[0].kind, .heading(level: 1))
        XCTAssertEqual(symbols[1].name, "Technical Architecture")
        XCTAssertEqual(symbols[1].kind, .heading(level: 2))
        XCTAssertEqual(symbols[2].name, "Microkernel Engine")
        XCTAssertEqual(symbols[2].kind, .heading(level: 3))
    }

    // MARK: - Document Analysis & Caching Tests

    func testAnalyzeDocumentE2E() async {
        let code = """
        import Foundation

        public struct DocumentPayload {
            public let id: UUID
        }
        """

        let doc = await service.analyzeDocument(code: code, filePathOrExtension: "Payload.swift")
        XCTAssertEqual(doc.language.languageId, "swift")
        XCTAssertFalse(doc.tokens.isEmpty)
        XCTAssertFalse(doc.symbols.isEmpty)
        XCTAssertEqual(doc.lineCount, 5)
    }

    func testActorCachingPerformance() async {
        let code = "fn compute() -> i32 { 100 * 2 }"

        let start1 = CFAbsoluteTimeGetCurrent()
        let tokens1 = await service.highlight(code: code, language: "rs")
        let duration1 = CFAbsoluteTimeGetCurrent() - start1

        let start2 = CFAbsoluteTimeGetCurrent()
        let tokens2 = await service.highlight(code: code, language: "rs")
        let duration2 = CFAbsoluteTimeGetCurrent() - start2

        XCTAssertEqual(tokens1, tokens2)
        XCTAssertLessThanOrEqual(duration2, duration1 + 0.05)

        await service.clearCache()
    }

    func testSynchronousApis() {
        let code = "def hello(): pass"
        let tokens = service.highlightSync(code: code, language: "py")
        XCTAssertFalse(tokens.isEmpty)

        let symbols = service.extractSymbolsSync(code: code, language: "py")
        XCTAssertFalse(symbols.isEmpty)

        let lang = service.detectLanguageSync(filePathOrExtension: "test.py")
        XCTAssertEqual(lang.languageId, "python")
    }

    func testEmptyInputs() async {
        let emptyTokens = await service.highlight(code: "", language: "rs")
        XCTAssertTrue(emptyTokens.isEmpty)

        let emptySymbols = await service.extractSymbols(code: "   ", language: "rs")
        XCTAssertTrue(emptySymbols.isEmpty)

        let syncTokens = service.highlightSync(code: "")
        XCTAssertTrue(syncTokens.isEmpty)
    }
}
