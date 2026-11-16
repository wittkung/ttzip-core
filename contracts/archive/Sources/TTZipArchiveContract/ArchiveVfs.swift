// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

public enum ArchiveVfs {
    public static let scheme = "ttzip-vfs"
}

/// Asynchronous resource resolver providing zero-extraction streaming for `ttzip-vfs://` requests.
public protocol TTZipVfsResourceProvider: Sendable {
    /// Loads the complete resource data and MIME type for a resolved VFS URI.
    func loadResource(uri: String) async throws -> (data: Data, mimeType: String)?

    /// Loads a partial byte range of the resource for HTTP 206 streaming requests.
    func loadResourceRange(uri: String, byteRange: ClosedRange<Int>?) async throws -> (data: Data, fullSize: Int64, mimeType: String)?
}
