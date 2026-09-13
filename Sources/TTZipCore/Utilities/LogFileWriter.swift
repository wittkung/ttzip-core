// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Industrial asynchronous, non-blocking log file writer with rolling backups (`TTLogFileWriter`).
public final class TTLogFileWriter: @unchecked Sendable {
    public static let shared = TTLogFileWriter()

    private static let queueSpecificKey = DispatchSpecificKey<Void>()
    private let queue: DispatchQueue

    private let maxFileSizeBytes: UInt64 = 10 * 1024 * 1024 // 10 MB
    private let maxBackupCount: Int = 5

    private let logDirectory: URL
    private let primaryLogFileURL: URL
    private var fileHandle: FileHandle?
    private var currentFileSize: UInt64 = 0

    public var logFilePath: String {
        return primaryLogFileURL.path
    }

    public var logDirectoryPath: String {
        return logDirectory.path
    }

    public init(customLogDirectory: URL? = nil) {
        let queue = DispatchQueue(label: "com.metastudyline.ttzip.logwriter", qos: .utility)
        queue.setSpecific(key: Self.queueSpecificKey, value: ())
        self.queue = queue

        if let customDir = customLogDirectory {
            self.logDirectory = customDir
        } else {
            let baseDir: URL
            if let libraryDir = FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask).first {
                baseDir = libraryDir
            } else {
                baseDir = URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Library", isDirectory: true)
            }
            self.logDirectory = baseDir.appendingPathComponent("Logs/TTZip", isDirectory: true)
        }

        self.primaryLogFileURL = self.logDirectory.appendingPathComponent("ttzip.log")

        self.queue.sync {
            self.prepareDirectoryAndOpenFile()
        }
    }

    deinit {
        flushSync()
        try? fileHandle?.close()
    }

    // MARK: - Public API

    /// Appends formatted string asynchronously onto dedicated serial queue.
    public func write(_ string: String) {
        queue.async { [self] in
            self.appendInternal(string)
        }
    }

    /// Synchronously flushes log file buffers to disk for emergency/crash handlers.
    public func flushSync() {
        if DispatchQueue.getSpecific(key: Self.queueSpecificKey) != nil {
            self.flushInternal()
        } else {
            queue.sync {
                self.flushInternal()
            }
        }
    }

    // MARK: - Internal Engine

    private func prepareDirectoryAndOpenFile() {
        let fm = FileManager.default
        if !fm.fileExists(atPath: logDirectory.path) {
            try? fm.createDirectory(at: logDirectory, withIntermediateDirectories: true, attributes: nil)
        }

        if !fm.fileExists(atPath: primaryLogFileURL.path) {
            fm.createFile(atPath: primaryLogFileURL.path, contents: nil, attributes: nil)
            currentFileSize = 0
        } else {
            if let attrs = try? fm.attributesOfItem(atPath: primaryLogFileURL.path),
               let size = attrs[.size] as? UInt64 {
                currentFileSize = size
            } else {
                currentFileSize = 0
            }
        }

        fileHandle = try? FileHandle(forWritingTo: primaryLogFileURL)
        if let handle = fileHandle {
            _ = try? handle.seekToEnd()
        }
    }

    private func appendInternal(_ string: String) {
        guard let data = string.data(using: .utf8) else { return }
        let writeLength = UInt64(data.count)

        if currentFileSize + writeLength > maxFileSizeBytes {
            rotateFilesInternal()
        }

        if fileHandle == nil {
            prepareDirectoryAndOpenFile()
        }

        guard let handle = fileHandle else { return }
        do {
            try handle.write(contentsOf: data)
            currentFileSize += writeLength
        } catch {
            // Fallback: try reopening once on failure
            try? handle.close()
            fileHandle = nil
            prepareDirectoryAndOpenFile()
            try? fileHandle?.write(contentsOf: data)
            currentFileSize += writeLength
        }
    }

    private func rotateFilesInternal() {
        try? fileHandle?.synchronize()
        try? fileHandle?.close()
        fileHandle = nil

        let fm = FileManager.default

        // 1. Remove the oldest backup if it exists (e.g. ttzip.5.log)
        let oldestBackup = logDirectory.appendingPathComponent("ttzip.\(maxBackupCount).log")
        if fm.fileExists(atPath: oldestBackup.path) {
            try? fm.removeItem(at: oldestBackup)
        }

        // 2. Shift older backups down: ttzip.4.log -> ttzip.5.log, ..., ttzip.1.log -> ttzip.2.log
        if maxBackupCount > 1 {
            for index in stride(from: maxBackupCount - 1, through: 1, by: -1) {
                let src = logDirectory.appendingPathComponent("ttzip.\(index).log")
                let dst = logDirectory.appendingPathComponent("ttzip.\(index + 1).log")
                if fm.fileExists(atPath: src.path) {
                    try? fm.moveItem(at: src, to: dst)
                }
            }
        }

        // 3. Move active log to ttzip.1.log
        let firstBackup = logDirectory.appendingPathComponent("ttzip.1.log")
        if fm.fileExists(atPath: primaryLogFileURL.path) {
            try? fm.moveItem(at: primaryLogFileURL, to: firstBackup)
        }

        // 4. Create fresh active file
        fm.createFile(atPath: primaryLogFileURL.path, contents: nil, attributes: nil)
        currentFileSize = 0
        fileHandle = try? FileHandle(forWritingTo: primaryLogFileURL)
    }

    private func flushInternal() {
        try? fileHandle?.synchronize()
    }
}
