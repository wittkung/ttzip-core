// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Canonical alias mapping office outline domain entity.
public typealias TTZipOfficeOutline = TTZipDocumentOutline

extension TTZipDocumentMetadataService {
    /// Synchronously extracts office document outline from a filesystem path.
    public nonisolated func extractOfficeOutlineSync(at path: String) throws -> TTZipOfficeOutline {
        let uniffi = try UniFfiXmlMetaService().extractOfficeOutline(filePath: path)
        let kind = TTZipDocumentKind.from(pathOrExtension: path)
        return TTZipOfficeOutline(from: uniffi, kind: kind)
    }

    /// Synchronously extracts office document outline from an in-memory byte buffer.
    public nonisolated func extractOfficeOutlineSync(from data: Data, kind: TTZipDocumentKind = .unknown) throws -> TTZipOfficeOutline {
        let uniffi = try UniFfiXmlMetaService().extractOfficeOutlineFromBytes(bytes: data)
        return TTZipOfficeOutline(from: uniffi, kind: kind)
    }
}
