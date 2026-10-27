// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

extension TTZipEbookReaderService {
    /// Synchronously extracts ebook publication metadata from a filesystem path.
    public nonisolated func extractMetadataSync(at path: String) throws -> TTZipEbookMetadata {
        let uniffi = try UniFfiEbookService().extractMetadataFromFile(filePath: path)
        return TTZipEbookMetadata(from: uniffi, sourceId: path)
    }

    /// Synchronously extracts ebook publication metadata from an in-memory byte buffer.
    public nonisolated func extractMetadataSync(from data: Data, fileName: String? = nil) throws -> TTZipEbookMetadata {
        let uniffi = try UniFfiEbookService().extractMetadata(data: data, fileName: fileName)
        return TTZipEbookMetadata(from: uniffi, sourceId: fileName ?? "memory://ebook")
    }

    /// Synchronously extracts hierarchical Table of Contents from a filesystem path.
    public nonisolated func extractTocSync(at path: String) throws -> [TTZipEbookTocNode] {
        let uniffi = try UniFfiEbookService().extractTocFromFile(filePath: path)
        return uniffi.map { TTZipEbookTocNode(from: $0) }
    }

    /// Synchronously extracts hierarchical Table of Contents from an in-memory byte buffer.
    public nonisolated func extractTocSync(from data: Data, fileName: String? = nil) throws -> [TTZipEbookTocNode] {
        let uniffi = try UniFfiEbookService().extractToc(data: data, fileName: fileName)
        return uniffi.map { TTZipEbookTocNode(from: $0) }
    }

    /// Synchronously extracts sequential spine reading items from a filesystem path.
    public nonisolated func getSpineSync(at path: String) throws -> [TTZipEbookSpineItem] {
        let uniffi = try UniFfiEbookService().getSpineFromFile(filePath: path)
        return uniffi.map { TTZipEbookSpineItem(from: $0) }
    }

    /// Synchronously extracts sequential spine reading items from an in-memory byte buffer.
    public nonisolated func getSpineSync(from data: Data, fileName: String? = nil) throws -> [TTZipEbookSpineItem] {
        let uniffi = try UniFfiEbookService().getSpine(data: data, fileName: fileName)
        return uniffi.map { TTZipEbookSpineItem(from: $0) }
    }
}
