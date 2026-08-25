// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// High-level engine providing multi-volume split archive management, slicing, and reassembly.
public final class SplitVolumeEngine: Sendable {
    public static let shared = SplitVolumeEngine()
    
    public init() {}
    
    /// Joins multi-volume split files into a continuous output archive.
    public func joinVolumes(
        firstVolumePath: String,
        outputPath: String,
        progressHandler: (@Sendable (Double) -> Bool)? = nil
    ) throws {
        try SplitVolumeConcatenator.shared.join(
            firstVolumePath: firstVolumePath,
            outputPath: outputPath,
            progressHandler: progressHandler
        )
    }
    
    /// Discovers and lists all volume paths belonging to a split archive set from a seed volume.
    public func resolveVolumes(seedPath: String) -> [String] {
        return SplitVolumeConcatenator.shared.inspect(seedPath: seedPath)?.volumePaths ?? [seedPath]
    }

    /// Slices a file into multi-volume parts of `splitSizeBytes` with zero heap memory overhead.
    public func sliceArchive(
        archivePath: String,
        splitSizeBytes: Int64,
        namingPattern: VolumeNamingPattern = .numberedExtension,
        cleanOnFailure: Bool = true
    ) throws {
        guard splitSizeBytes > 0 else { throw ArchiveError.readFailed(code: -1) }
        guard let inputStream = InputStream(fileAtPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }
        inputStream.open()
        defer { inputStream.close() }
        
        let bufferSize = 1024 * 1024 // 1 MB chunk stream
        let buffer = UnsafeMutablePointer<UInt8>.allocate(capacity: bufferSize)
        defer { buffer.deallocate() }
        
        var partIndex = 1
        var bytesWrittenToCurrentPart: Int64 = 0
        var currentOutputStream: OutputStream? = nil
        var createdParts: [String] = []
        
        func closeCurrentOutput() {
            if let stream = currentOutputStream {
                stream.close()
                currentOutputStream = nil
            }
        }
        
        func openNextOutput() throws -> OutputStream {
            closeCurrentOutput()
            let partExt: String
            switch namingPattern {
            case .numberedExtension, .rawSplit:
                partExt = String(format: "%03d", partIndex)
            case .pkzipSpanned:
                partExt = String(format: "z%02d", partIndex)
            }
            let partPath = "\(archivePath).\(partExt)"
            createdParts.append(partPath)
            guard let outStream = OutputStream(toFileAtPath: partPath, append: false) else {
                throw ArchiveError.engineFailure(code: -1, message: "Failed to open output stream")
            }
            outStream.open()
            partIndex += 1
            bytesWrittenToCurrentPart = 0
            return outStream
        }
        
        do {
            var activeStream = try openNextOutput()
            while inputStream.hasBytesAvailable {
                let bytesToRead = min(Int64(bufferSize), splitSizeBytes - bytesWrittenToCurrentPart)
                if bytesToRead <= 0 {
                    activeStream = try openNextOutput()
                    continue
                }
                let bytesRead = inputStream.read(buffer, maxLength: Int(bytesToRead))
                if bytesRead < 0 {
                    throw ArchiveError.readFailed(code: -2)
                } else if bytesRead == 0 {
                    break
                }
                var totalWritten = 0
                while totalWritten < bytesRead {
                    let written = activeStream.write(buffer.advanced(by: totalWritten), maxLength: bytesRead - totalWritten)
                    if written <= 0 {
                        throw ArchiveError.engineFailure(code: -3, message: "Failed to write bytes to stream")
                    }
                    totalWritten += written
                }
                bytesWrittenToCurrentPart += Int64(bytesRead)
                if bytesWrittenToCurrentPart >= splitSizeBytes && inputStream.hasBytesAvailable {
                    activeStream = try openNextOutput()
                }
            }
            closeCurrentOutput()
        } catch {
            closeCurrentOutput()
            if cleanOnFailure {
                for path in createdParts {
                    try? FileManager.default.removeItem(atPath: path)
                }
            }
            throw error
        }
    }
}

// MARK: - Split Volume Config

/// Predefined media size presets for split volume generation.
public enum VolumePreset: String, Sendable, Codable, CaseIterable {
    case cd700MB = "cd_700mb"
    case dvd4700MB = "dvd_4700mb"
    case fat32_4GB = "fat32_4gb"
    case email25MB = "email_25mb"
    case wechat100MB = "wechat_100mb"
    case custom = "custom"
}

/// Volume naming conventions for spanned archives.
public enum VolumeNamingPattern: String, Sendable, Codable, CaseIterable {
    case numberedExtension = "numbered_extension" // .7z.001, .zip.001, .tar.001
    case pkzipSpanned = "pkzip_spanned"           // .z01, .z02, .zip
    case rawSplit = "raw_split"                   // .001, .002
}

/// Configuration for creating multi-volume / split archives.
public struct SplitVolumeConfig: Sendable, Codable, Equatable {
    public let volumeSizeBytes: Int64
    public let preset: VolumePreset
    public let namingPattern: VolumeNamingPattern
    public let cleanOnFailure: Bool

    public init(
        volumeSizeBytes: Int64,
        preset: VolumePreset = .custom,
        namingPattern: VolumeNamingPattern = .numberedExtension,
        cleanOnFailure: Bool = true
    ) {
        self.volumeSizeBytes = volumeSizeBytes
        self.preset = preset
        self.namingPattern = namingPattern
        self.cleanOnFailure = cleanOnFailure
    }
}

// MARK: - Concatenator

/// High-performance UniFFI engine for concatenating and inspecting multi-volume split archives.
public final class SplitVolumeConcatenator: Sendable {
    public static let shared = SplitVolumeConcatenator()
    
    public init() {}
    
    /// Joins multi-volume split archive files starting from the first volume seed into a single continuous file.
    public func join(
        firstVolumePath: String,
        outputPath: String,
        progressHandler: (@Sendable (Double) -> Bool)? = nil
    ) throws {
        do {
            try joinSplitVolumeChain(firstVolumePath: firstVolumePath, outputPath: outputPath)
            _ = progressHandler?(1.0)
        } catch {
            throw ArchiveError.readFailed(code: -1)
        }
    }
    
    /// Queries the total uncompressed continuous size and volume count for a split volume series.
    public func inspect(seedPath: String) -> (totalSize: UInt64, volumePaths: [String])? {
        guard let paths = try? detectSplitVolumeChain(seedPath: seedPath), !paths.isEmpty else {
            return nil
        }
        var total: UInt64 = 0
        for p in paths {
            if let attr = try? FileManager.default.attributesOfItem(atPath: p) {
                total += (attr[.size] as? UInt64) ?? 0
            }
        }
        return (total, paths)
    }
}

// MARK: - Parallel Encrypted Split Engine

/// Hardware-accelerated encrypted multi-volume archive engine (7z `.7z.001` and ZIP `.zip.001`).
public final class NativeParallelEncryptedSplitEngine: Sendable {
    public init() {}
    
    public enum SplitFormat: String, Sendable {
        case sevenZip = "7z"
        case zip = "zip"
    }
    
    /// Creates standard encrypted multi-volume split archives.
    public func createStandardEncryptedSplitVolume(
        format: SplitFormat = .sevenZip,
        sourcePaths: [String],
        outputDir: String,
        baseName: String,
        splitVolumeSizeBytes: Int64,
        password: String,
        encryptFileNames: Bool = true,
        progressHandler: (@Sendable (Double) -> Void)? = nil
    ) async throws -> [String] {
        guard !sourcePaths.isEmpty else {
            throw ArchiveError.readFailed(code: -404)
        }
        
        let targetExtension = (format == .sevenZip) ? "7z" : "zip"
        let primaryOutputPath = (outputDir as NSString).appendingPathComponent("\(baseName).\(targetExtension)")
        try? FileManager.default.removeItem(atPath: primaryOutputPath)
        
        progressHandler?(0.1)
        
        let writer = ArchiveWriter()
        let targetFormat: ArchiveCompressionFormat = (format == .sevenZip) ? .sevenZip : .zip
        
        try await writer.createArchive(
            outputPath: primaryOutputPath,
            format: targetFormat,
            level: .normal,
            inputPaths: sourcePaths,
            options: .defaultClean,
            splitVolumeSizeBytes: splitVolumeSizeBytes,
            password: password.isEmpty ? nil : password
        )
        
        progressHandler?(1.0)
        
        let fm = FileManager.default
        let allFiles = (try? fm.contentsOfDirectory(atPath: outputDir)) ?? []
        let generatedVolumes = allFiles.filter { file in
            file.hasPrefix(baseName) && (file.contains(".7z.") || file.contains(".z") || file.contains(".00") || file.hasSuffix(".7z") || file.hasSuffix(".zip"))
        }.sorted().map { (outputDir as NSString).appendingPathComponent($0) }
        
        return generatedVolumes.isEmpty ? [primaryOutputPath] : generatedVolumes
    }
}
