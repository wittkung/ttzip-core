// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
import CTTZipBridge

/// High-performance APFS Copy-on-Write (CoW) zero-copy clone and allocation service.
///
/// Enables instantaneous, zero-disk-IO file duplication and uncompressed archive staging
/// on Apple File System (APFS) volumes without consuming physical SSD write endurance.
///
/// > **CRITICAL ARCHITECTURAL INVARIANT**:
/// > APFS zero-copy cloning is strictly prohibited inside `ttzip-bench`, micro-benchmarks,
/// > and compression telemetry suites to ensure benchmark metrics evaluate pure algorithmic
/// > compute throughput and SIMD vector performance.
public enum APFSCloneService: Sendable {

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

    /// Clones a source file to the destination path using APFS Copy-on-Write zero-copy metadata clone.
    ///
    /// - Parameters:
    ///   - sourcePath: Absolute path to the source file.
    ///   - destinationPath: Absolute path to the target destination file.
    ///   - overwrite: If true, removes existing destination file before cloning.
    /// - Returns: True if APFS clone succeeded, false if unsupported or failed.
    @discardableResult
    public static func cloneFile(
        from sourcePath: String,
        to destinationPath: String,
        overwrite: Bool = true
    ) -> Bool {
        let fileManager = FileManager.default
        guard fileManager.fileExists(atPath: sourcePath) else {
            return false
        }

        if overwrite && fileManager.fileExists(atPath: destinationPath) {
            try? fileManager.removeItem(atPath: destinationPath)
        }

        return sourcePath.withCString { cSrc in
            destinationPath.withCString { cDst in
                let status = ttzip_rust_apfs_clone_file(cSrc, cDst, overwrite)
                return status == 0
            }
        }
    }

    /// Clones file descriptor range using macOS kernel `fcopyfile` zero-copy clone.
    ///
    /// - Parameters:
    ///   - sourceFd: Open file descriptor for the source file.
    ///   - destinationFd: Open file descriptor for the destination file.
    /// - Returns: True if kernel zero-copy clone succeeded.
    @discardableResult
    public static func cloneRange(sourceFd: Int32, destinationFd: Int32) -> Bool {
        guard sourceFd >= 0, destinationFd >= 0 else { return false }
        let status = ttzip_rust_apfs_clone_range(sourceFd, destinationFd)
        return status == 0
    }
}
