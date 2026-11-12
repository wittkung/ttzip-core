// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Fast static lock-free MIME type mapper table.
public enum ArchiveMimeMapper {
    @usableFromInline
    static let staticMimeMap: [String: String] = [
        "swift": "text/x-swift", "js": "application/javascript", "json": "application/json",
        "html": "text/html", "css": "text/css", "png": "image/png", "jpg": "image/jpeg",
        "jpeg": "image/jpeg", "gif": "image/gif", "svg": "image/svg+xml", "pdf": "application/pdf",
        "zip": "application/zip", "7z": "application/x-7z-compressed", "tar": "application/x-tar",
        "gz": "application/gzip", "zst": "application/zstd", "txt": "text/plain", "md": "text/markdown",
        "c": "text/x-c", "cpp": "text/x-c++", "h": "text/x-chdr", "py": "text/x-python",
        "rs": "text/x-rust", "go": "text/x-go", "xml": "application/xml", "mp3": "audio/mpeg",
        "mp4": "video/mp4", "mov": "video/quicktime"
    ]
    
    @inlinable
    public static func mimeType(forExtension ext: String) -> String {
        staticMimeMap[ext] ?? "application/octet-stream"
    }
}
