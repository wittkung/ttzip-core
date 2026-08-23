// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation

/// Centralized lifecycle manager for temporary extraction directories, drag-and-drop promises,
/// and Quick Look preview caches.
///
/// Features:
/// 1. Automatic orphaned directory sweep on application launch.
/// 2. 2GB LRU soft capacity quota enforcement.
/// 3. Scoped ephemeral resource tokens with automatic or explicit delayed cleanup.
public final actor EphemeralResourceManager {
    public static let shared = EphemeralResourceManager()

    private let maxCacheSizeBytes: Int64 = 2 * 1024 * 1024 * 1024 // 2 GB soft quota
    private var trackedDirectories: [URL: Date] = [:]
    private let prefix = "ttzip_ephemeral_"

    private init() {
        sweepOrphanedDirectories()
    }

    /// Allocates an ephemeral working directory tracked by the manager.
    public func createEphemeralDirectory() throws -> URL {
        let tempBase = FileManager.default.temporaryDirectory
        let uniqueName = "\(prefix)\(UUID().uuidString)"
        let dirURL = tempBase.appendingPathComponent(uniqueName, isDirectory: true)

        try FileManager.default.createDirectory(at: dirURL, withIntermediateDirectories: true)
        trackedDirectories[dirURL] = Date()

        enforceCapacityQuota()
        return dirURL
    }

    /// Releases and removes a tracked ephemeral directory immediately or after a delay.
    public func releaseDirectory(_ url: URL, delaySeconds: TimeInterval = 0) {
        trackedDirectories.removeValue(forKey: url)

        if delaySeconds <= 0 {
            try? FileManager.default.removeItem(at: url)
        } else {
            Task.detached(priority: .background) {
                try? await Task.sleep(nanoseconds: UInt64(delaySeconds * 1_000_000_000))
                try? FileManager.default.removeItem(at: url)
            }
        }
    }

    /// Sweeps any orphaned `ttzip_` temporary directories left behind by prior crashes or abnormal terminations.
    public nonisolated func sweepOrphanedDirectories() {
        Task.detached(priority: .background) {
            let fm = FileManager.default
            let tempBase = fm.temporaryDirectory

            guard let contents = try? fm.contentsOfDirectory(at: tempBase, includingPropertiesForKeys: [.contentModificationDateKey]) else {
                return
            }

            for item in contents {
                let name = item.lastPathComponent
                if name.hasPrefix("ttzip_") || name.hasPrefix("TTZip_") {
                    try? fm.removeItem(at: item)
                }
            }
        }
    }

    private func enforceCapacityQuota() {
        let fm = FileManager.default
        let snapshot = self.trackedDirectories

        var totalSize: Int64 = 0
        var dirSizes: [(URL, Int64, Date)] = []

        for (url, date) in snapshot {
            if let files = try? fm.contentsOfDirectory(at: url, includingPropertiesForKeys: [.fileSizeKey]) {
                var size: Int64 = 0
                for fileURL in files {
                    if let s = try? fileURL.resourceValues(forKeys: [.fileSizeKey]).fileSize {
                        size += Int64(s)
                    }
                }
                totalSize += size
                dirSizes.append((url, size, date))
            }
        }

        if totalSize > self.maxCacheSizeBytes {
            let sorted = dirSizes.sorted { $0.2 < $1.2 }
            var current = totalSize
            for (url, sz, _) in sorted {
                if current <= self.maxCacheSizeBytes / 2 { break }
                try? fm.removeItem(at: url)
                self.trackedDirectories.removeValue(forKey: url)
                current -= sz
            }
        }
    }
}
