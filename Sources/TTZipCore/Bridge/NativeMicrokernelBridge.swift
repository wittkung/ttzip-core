// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

public enum TTZipFileKind: UInt32, Sendable {
    case unknown = 0
    case archive = 1
    case image = 2
    case audio = 3
    case video = 4
    case binary = 5
}

public typealias ttzip_file_kind_t = TTZipFileKind
public let TTZIP_KIND_UNKNOWN = TTZipFileKind.unknown
public let TTZIP_KIND_ARCHIVE = TTZipFileKind.archive
public let TTZIP_KIND_IMAGE = TTZipFileKind.image
public let TTZIP_KIND_AUDIO = TTZipFileKind.audio
public let TTZIP_KIND_VIDEO = TTZipFileKind.video
public let TTZIP_KIND_BINARY = TTZipFileKind.binary

// MARK: - Native Microkernel Bridge

/// High-performance thin Swift bridge for format sniffing and natural string sorting.
public enum NativeMicrokernelBridge {
    
    /// Sniffs file format magic numbers in constant time using Rust SIMD sniffer.
    public static func sniffMagic(data: Data) -> (kind: TTZipFileKind, format: String, mime: String) {
        guard data.count >= 2 else {
            return (.unknown, "UNKNOWN", "application/octet-stream")
        }
        let meta = sniffFormatBuffer(data: data, filenameHint: nil)
        let kind: TTZipFileKind
        if meta.isArchive {
            kind = .archive
        } else if meta.mimeType.starts(with: "image/") {
            kind = .image
        } else if meta.mimeType.starts(with: "audio/") {
            kind = .audio
        } else if meta.mimeType.starts(with: "video/") {
            kind = .video
        } else if meta.mimeType == "application/pdf" {
            kind = .binary
        } else {
            kind = .unknown
        }
        return (kind, meta.formatName, meta.mimeType)
    }
    
    /// Fast natural sort on paths backed by pure Rust UniFFI kernel.
    public static func naturalSort(_ paths: [String]) -> [String] {
        return naturalSortPaths(items: paths)
    }

    /// Natural string comparator backed by pure Rust UniFFI kernel.
    public static func naturalCompare(_ a: String, _ b: String) -> ComparisonResult {
        let cmp = TTZipCore.naturalCompare(a: a, b: b)
        if cmp < 0 { return .orderedAscending }
        if cmp > 0 { return .orderedDescending }
        return .orderedSame
    }
    
    /// Extracts normalized audio waveform amplitudes [0.08 ... 1.0] from a file path using pure Rust UniFFI kernel.
    public static func extractAudioWaveform(path: String, bucketCount: Int = 36) -> [Float] {
        if let result = try? TTZipCore.extractAudioWaveform(path: path, bucketCount: UInt32(bucketCount)), !result.isEmpty {
            return result
        }
        return (0..<bucketCount).map { idx in
            let p = Float(idx) / Float(bucketCount)
            let curve = sin(p * Float.pi * 3.2) * 0.4 + cos(p * Float.pi * 1.8) * 0.3
            return max(0.12, min(0.9, 0.35 + abs(curve)))
        }
    }
    
    /// Extracts normalized audio waveform amplitudes [0.08 ... 1.0] from memory data using pure Rust UniFFI kernel.
    public static func extractAudioWaveformFromMemory(data: Data, bucketCount: Int = 36) -> [Float] {
        if let result = try? TTZipCore.extractAudioWaveformFromMemory(data: data, bucketCount: UInt32(bucketCount)), !result.isEmpty {
            return result
        }
        return (0..<bucketCount).map { idx in
            let p = Float(idx) / Float(bucketCount)
            let curve = sin(p * Float.pi * 3.2) * 0.4 + cos(p * Float.pi * 1.8) * 0.3
            return max(0.12, min(0.9, 0.35 + abs(curve)))
        }
    }
}
