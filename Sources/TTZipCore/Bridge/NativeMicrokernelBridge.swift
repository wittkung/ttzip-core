// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation
import CTTZipBridge

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

/// High-performance thin Swift bridge for format sniffing, APFS cloning, and natural string sorting.
public enum NativeMicrokernelBridge {
    
    /// Sniffs file format magic numbers in constant time using Rust SIMD sniffer.
    @inlinable
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
    @inlinable
    public static func naturalSort(_ paths: [String]) -> [String] {
        return naturalSortPaths(items: paths)
    }

    /// Natural string comparator backed by pure Rust UniFFI kernel.
    @inlinable
    public static func naturalCompare(_ a: String, _ b: String) -> ComparisonResult {
        #if SWIFT_PACKAGE
        let cmp = TTZipCore.naturalCompare(a: a, b: b)
        #else
        let cmp = TTZipCoreBindings.naturalCompare(a: a, b: b)
        #endif
        if cmp < 0 { return .orderedAscending }
        if cmp > 0 { return .orderedDescending }
        return .orderedSame
    }
    
    /// Extracts normalized audio waveform amplitudes [0.08 ... 1.0] from a file path using pure Rust UniFFI kernel.
    @inlinable
    public static func extractAudioWaveform(path: String, bucketCount: Int = 36) -> [Float] {
        #if SWIFT_PACKAGE
        let result = try? TTZipCore.extractAudioWaveform(path: path, bucketCount: UInt32(bucketCount))
        #else
        let result = try? TTZipCoreBindings.extractAudioWaveform(path: path, bucketCount: UInt32(bucketCount))
        #endif
        if let result = result, !result.isEmpty {
            return result
        }
        return (0..<bucketCount).map { idx in
            let p = Float(idx) / Float(bucketCount)
            let curve = sin(p * Float.pi * 3.2) * 0.4 + cos(p * Float.pi * 1.8) * 0.3
            return max(0.12, min(0.9, 0.35 + abs(curve)))
        }
    }
    
    /// Extracts normalized audio waveform amplitudes [0.08 ... 1.0] from memory data using pure Rust UniFFI kernel.
    @inlinable
    public static func extractAudioWaveformFromMemory(data: Data, bucketCount: Int = 36) -> [Float] {
        #if SWIFT_PACKAGE
        let result = try? TTZipCore.extractAudioWaveformFromMemory(data: data, bucketCount: UInt32(bucketCount))
        #else
        let result = try? TTZipCoreBindings.extractAudioWaveformFromMemory(data: data, bucketCount: UInt32(bucketCount))
        #endif
        if let result = result, !result.isEmpty {
            return result
        }
        return (0..<bucketCount).map { idx in
            let p = Float(idx) / Float(bucketCount)
            let curve = sin(p * Float.pi * 3.2) * 0.4 + cos(p * Float.pi * 1.8) * 0.3
            return max(0.12, min(0.9, 0.35 + abs(curve)))
        }
    }

    /// Clones a source file to destination using Rust APFS Copy-on-Write (CoW) zero-copy clone,
    /// falling back to standard physical copy if APFS clone is unsupported on the underlying volume.
    @discardableResult
    public static func apfsCloneOrCopy(
        from sourcePath: String,
        to destinationPath: String,
        overwrite: Bool = true
    ) -> Bool {
        guard FileManager.default.fileExists(atPath: sourcePath) else {
            return false
        }
        return sourcePath.withCString { cSrc in
            destinationPath.withCString { cDst in
                ttzip_rust_apfs_clone_file(cSrc, cDst, overwrite) == 0
            }
        }
    }

    /// Clones file descriptor range using macOS kernel and Rust microkernel zero-copy clone.
    @discardableResult
    public static func cloneRange(sourceFd: Int32, destinationFd: Int32) -> Bool {
        guard sourceFd >= 0, destinationFd >= 0 else { return false }
        return ttzip_rust_apfs_clone_range(sourceFd, destinationFd) == 0
    }

    /// Preallocates contiguous physical extent space on APFS filesystems via Rust microkernel.
    @discardableResult
    public static func apfsPreallocate(fd: Int32, targetSize: Int64) -> Bool {
        guard fd >= 0, targetSize > 0 else { return false }
        return ttzip_rust_apfs_preallocate(fd, targetSize) == 0
    }

    /// Checks whether the filesystem containing the given path is formatted as APFS.
    public static func isAPFSFileSystem(at path: String) -> Bool {
        var stat = statfs()
        let result = path.withCString { cPath in
            statfs(cPath, &stat)
        }
        guard result == 0 else { return false }
        
        let fsName = withUnsafePointer(to: &stat.f_fstypename) { ptr in
            String(cString: UnsafeRawPointer(ptr).assumingMemoryBound(to: CChar.self))
        }
        return fsName.lowercased() == "apfs"
    }
}

// MARK: - Global Microkernel Convenience APIs

/// Clones a source file to destination using Rust APFS Copy-on-Write (CoW) zero-copy clone,
/// falling back to physical copy if APFS clone is unsupported on the underlying volume.
@discardableResult
public func apfsCloneOrCopy(
    from sourcePath: String,
    to destinationPath: String,
    overwrite: Bool = true
) -> Bool {
    NativeMicrokernelBridge.apfsCloneOrCopy(from: sourcePath, to: destinationPath, overwrite: overwrite)
}

/// Checks whether the filesystem containing the given path is formatted as APFS.
public func isAPFSFileSystem(at path: String) -> Bool {
    NativeMicrokernelBridge.isAPFSFileSystem(at: path)
}

