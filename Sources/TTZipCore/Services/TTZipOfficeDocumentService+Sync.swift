// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

extension TTZipOfficeDocumentService {
    /// Synchronously extracts sheet names from a filesystem path.
    public nonisolated func extractSheetNamesSync(at path: String) throws -> [String] {
        try UniFfiOfficeService().extractSheetNamesFromFile(filePath: path)
    }

    /// Synchronously extracts sheet names from an in-memory byte buffer.
    public nonisolated func extractSheetNamesSync(from data: Data, fileName: String? = nil) throws -> [String] {
        try UniFfiOfficeService().extractSheetNames(data: data, fileName: fileName)
    }

    /// Synchronously extracts sheet data from a filesystem path.
    public nonisolated func extractSheetDataSync(
        at path: String,
        sheetNameOrIndex: String = "1",
        maxRows: Int? = nil
    ) throws -> TTZipSheetData {
        let uniffi = try UniFfiOfficeService().extractSheetDataFromFile(
            filePath: path,
            sheetNameOrIndex: sheetNameOrIndex,
            maxRows: maxRows.map { UInt32($0) }
        )
        return TTZipSheetData(from: uniffi)
    }

    /// Synchronously extracts sheet data from an in-memory byte buffer.
    public nonisolated func extractSheetDataSync(
        from data: Data,
        sheetNameOrIndex: String = "1",
        maxRows: Int? = nil,
        fileName: String? = nil
    ) throws -> TTZipSheetData {
        let uniffi = try UniFfiOfficeService().extractSheetData(
            data: data,
            sheetNameOrIndex: sheetNameOrIndex,
            maxRows: maxRows.map { UInt32($0) },
            fileName: fileName
        )
        return TTZipSheetData(from: uniffi)
    }

    /// Synchronously converts DOCX to Markdown from a filesystem path.
    public nonisolated func convertDocxToMarkdownSync(at path: String) throws -> String {
        try UniFfiOfficeService().convertDocxToMarkdownFromFile(filePath: path)
    }

    /// Synchronously converts DOCX to Markdown from an in-memory byte buffer.
    public nonisolated func convertDocxToMarkdownSync(from data: Data, fileName: String? = nil) throws -> String {
        try UniFfiOfficeService().convertDocxToMarkdown(data: data, fileName: fileName)
    }
}
