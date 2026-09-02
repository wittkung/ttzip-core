// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation
import os
#if canImport(AppKit)
import AppKit
#endif

// MARK: - Strongly-Typed Video Formats & Codecs

/// Supported video container and format classifications.
public enum TTZipVideoFormat: String, Sendable, Equatable, Hashable, CaseIterable {
    case mp4
    case m4v
    case mov
    case mkv
    case webm
    case avi
    case wmv
    case flv
    case ts
    case ogv
    case unknown

    /// Human-readable display label for the container format.
    public var displayName: String {
        switch self {
        case .mp4: return "MPEG-4 Part 14 Video (MP4)"
        case .m4v: return "Apple MPEG-4 Video (M4V)"
        case .mov: return "Apple QuickTime Movie (MOV)"
        case .mkv: return "Matroska Video (MKV)"
        case .webm: return "WebM Video (WebM)"
        case .avi: return "Audio Video Interleave (AVI)"
        case .wmv: return "Windows Media Video (WMV)"
        case .flv: return "Flash Video (FLV)"
        case .ts: return "MPEG Transport Stream (TS)"
        case .ogv: return "Ogg Theora Video (OGV)"
        case .unknown: return "Unknown Video Format"
        }
    }

    /// Primary MIME type string associated with this video format.
    public var mimeType: String {
        switch self {
        case .mp4: return "video/mp4"
        case .m4v: return "video/x-m4v"
        case .mov: return "video/quicktime"
        case .mkv: return "video/x-matroska"
        case .webm: return "video/webm"
        case .avi: return "video/x-msvideo"
        case .wmv: return "video/x-ms-wmv"
        case .flv: return "video/x-flv"
        case .ts: return "video/mp2t"
        case .ogv: return "video/ogg"
        case .unknown: return "application/octet-stream"
        }
    }

    /// Uniform Type Identifier (UTI) string for macOS system integration.
    public var uniformTypeIdentifier: String {
        switch self {
        case .mp4: return "public.mpeg-4"
        case .m4v: return "com.apple.m4v-video"
        case .mov: return "com.apple.quicktime-movie"
        case .mkv: return "org.matroska.mkv"
        case .webm: return "org.webmproject.webm"
        case .avi: return "public.avi"
        case .wmv: return "com.microsoft.windows-media-wmv"
        case .flv: return "com.adobe.flash.video"
        case .ts: return "public.mpeg-2-transport-stream"
        case .ogv: return "org.xiph.ogv"
        case .unknown: return "public.movie"
        }
    }

    /// Infers video format from a filename, file path, or extension string.
    public static func from(pathOrExtension: String) -> TTZipVideoFormat {
        let ext = (pathOrExtension as NSString).pathExtension.lowercased()
        let cleanExt = ext.isEmpty ? pathOrExtension.lowercased() : ext
        switch cleanExt {
        case "mp4": return .mp4
        case "m4v": return .m4v
        case "mov", "qt": return .mov
        case "mkv": return .mkv
        case "webm": return .webm
        case "avi": return .avi
        case "wmv", "asf": return .wmv
        case "flv": return .flv
        case "ts", "m2ts", "mts": return .ts
        case "ogv": return .ogv
        default: return .unknown
        }
    }

    /// Converts from UniFFI format enum.
    internal static func fromUniFFI(_ f: UniFfiVideoFormat) -> TTZipVideoFormat {
        switch f {
        case .mp4: return .mp4
        case .m4v: return .m4v
        case .mov: return .mov
        case .mkv: return .mkv
        case .webm: return .webm
        case .avi: return .avi
        case .wmv: return .wmv
        case .flv: return .flv
        case .ts: return .ts
        case .ogv: return .ogv
        case .unknown: return .unknown
        }
    }
}

/// Video track codec classifications.
public enum TTZipVideoCodec: String, Sendable, Equatable, Hashable, CaseIterable {
    case h264
    case hevc
    case av1
    case vp9
    case vp8
    case proRes
    case theora
    case mpeg4
    case mpeg2
    case unknown

    /// Human-readable display label for the video codec.
    public var displayName: String {
        switch self {
        case .h264: return "H.264 / AVC"
        case .hevc: return "H.265 / HEVC"
        case .av1: return "AV1"
        case .vp9: return "VP9"
        case .vp8: return "VP8"
        case .proRes: return "Apple ProRes"
        case .theora: return "Theora"
        case .mpeg4: return "MPEG-4 Part 2"
        case .mpeg2: return "MPEG-2"
        case .unknown: return "Unknown Codec"
        }
    }

    /// Converts from UniFFI video codec enum.
    internal static func fromUniFFI(_ c: UniFfiVideoCodec) -> TTZipVideoCodec {
        switch c {
        case .h264: return .h264
        case .hevc: return .hevc
        case .av1: return .av1
        case .vp9: return .vp9
        case .vp8: return .vp8
        case .proRes: return .proRes
        case .theora: return .theora
        case .mpeg4: return .mpeg4
        case .mpeg2: return .mpeg2
        case .unknown: return .unknown
        }
    }
}

/// Audio track codec classifications within video containers.
public enum TTZipVideoAudioCodec: String, Sendable, Equatable, Hashable, CaseIterable {
    case aac
    case ac3
    case eac3
    case opus
    case flac
    case vorbis
    case mp3
    case alac
    case pcm
    case unknown

    /// Human-readable display label for the audio codec.
    public var displayName: String {
        switch self {
        case .aac: return "AAC"
        case .ac3: return "Dolby Digital (AC-3)"
        case .eac3: return "Dolby Digital Plus (E-AC-3)"
        case .opus: return "Opus"
        case .flac: return "FLAC"
        case .vorbis: return "Vorbis"
        case .mp3: return "MP3"
        case .alac: return "Apple Lossless (ALAC)"
        case .pcm: return "Linear PCM"
        case .unknown: return "Unknown Audio Codec"
        }
    }

    /// Converts from UniFFI audio codec enum.
    internal static func fromUniFFI(_ c: UniFfiAudioCodec) -> TTZipVideoAudioCodec {
        switch c {
        case .aac: return .aac
        case .ac3: return .ac3
        case .eac3: return .eac3
        case .opus: return .opus
        case .flac: return .flac
        case .vorbis: return .vorbis
        case .mp3: return .mp3
        case .alac: return .alac
        case .pcm: return .pcm
        case .unknown: return .unknown
        }
    }
}

// MARK: - Strongly-Typed Domain Models

/// Technical stream properties of an individual video track.
public struct TTZipVideoTrackInfo: Sendable, Equatable, Hashable, Identifiable {
    public var id: UInt32 { trackId }
    public var trackId: UInt32
    public var codec: TTZipVideoCodec
    public var codecName: String
    public var width: Int
    public var height: Int
    public var frameRate: Double
    public var bitrateKbps: Int
    public var durationSeconds: Double
    public var aspectRatio: String
    public var colorSpace: String?
    public var hdrFormat: String?
    public var rotationDegrees: Int

    /// Formatted resolution string (e.g. "3840 × 2160 (4K UHD)").
    public var resolutionDisplayString: String {
        if width >= 3840 && height >= 2160 {
            return "\(width) × \(height) (4K UHD)"
        } else if width >= 2560 && height >= 1440 {
            return "\(width) × \(height) (2K QHD)"
        } else if width >= 1920 && height >= 1080 {
            return "\(width) × \(height) (1080p FHD)"
        } else if width >= 1280 && height >= 720 {
            return "\(width) × \(height) (720p HD)"
        } else {
            return "\(width) × \(height)"
        }
    }

    /// Converts from UniFFI video track record.
    internal static func fromUniFFI(_ r: UniFfiVideoTrackInfo) -> TTZipVideoTrackInfo {
        TTZipVideoTrackInfo(
            trackId: r.trackId,
            codec: TTZipVideoCodec.fromUniFFI(r.codec),
            codecName: r.codecName,
            width: Int(r.width),
            height: Int(r.height),
            frameRate: r.frameRate,
            bitrateKbps: Int(r.bitrateKbps),
            durationSeconds: r.durationSeconds,
            aspectRatio: r.aspectRatio,
            colorSpace: r.colorSpace,
            hdrFormat: r.hdrFormat,
            rotationDegrees: Int(r.rotationDegrees)
        )
    }
}

/// Technical stream properties of an audio track embedded in a video container.
public struct TTZipVideoAudioTrackInfo: Sendable, Equatable, Hashable, Identifiable {
    public var id: UInt32 { trackId }
    public var trackId: UInt32
    public var codec: TTZipVideoAudioCodec
    public var codecName: String
    public var sampleRate: Int
    public var channels: Int
    public var channelLayout: String
    public var bitDepth: Int?
    public var bitrateKbps: Int
    public var language: String?
    public var title: String?
    public var isDefault: Bool

    /// Converts from UniFFI audio track record.
    internal static func fromUniFFI(_ r: UniFfiAudioTrackInfo) -> TTZipVideoAudioTrackInfo {
        TTZipVideoAudioTrackInfo(
            trackId: r.trackId,
            codec: TTZipVideoAudioCodec.fromUniFFI(r.codec),
            codecName: r.codecName,
            sampleRate: Int(r.sampleRate),
            channels: Int(r.channels),
            channelLayout: r.channelLayout,
            bitDepth: r.bitDepth.map { Int($0) },
            bitrateKbps: Int(r.bitrateKbps),
            language: r.language,
            title: r.title,
            isDefault: r.isDefault
        )
    }
}

/// Subtitle or timed text track information.
public struct TTZipSubtitleTrackInfo: Sendable, Equatable, Hashable, Identifiable {
    public var id: UInt32 { trackId }
    public var trackId: UInt32
    public var format: String
    public var language: String?
    public var title: String?
    public var isForced: Bool
    public var isDefault: Bool
    public var isSdh: Bool

    /// Converts from UniFFI subtitle track record.
    internal static func fromUniFFI(_ r: UniFfiSubtitleTrackInfo) -> TTZipSubtitleTrackInfo {
        TTZipSubtitleTrackInfo(
            trackId: r.trackId,
            format: r.format,
            language: r.language,
            title: r.title,
            isForced: r.isForced,
            isDefault: r.isDefault,
            isSdh: r.isSdh
        )
    }
}

/// Chapter navigation marker in the video timeline.
public struct TTZipChapterInfo: Sendable, Equatable, Hashable, Identifiable {
    public var id: UInt32 { chapterId }
    public var chapterId: UInt32
    public var title: String
    public var startTimeSeconds: Double
    public var endTimeSeconds: Double

    /// Formatted timestamp range string (e.g. "01:23 - 04:56").
    public var timestampDisplayString: String {
        let startMin = Int(startTimeSeconds) / 60
        let startSec = Int(startTimeSeconds) % 60
        let endMin = Int(endTimeSeconds) / 60
        let endSec = Int(endTimeSeconds) % 60
        return String(format: "%02d:%02d - %02d:%02d", startMin, startSec, endMin, endSec)
    }

    /// Converts from UniFFI chapter record.
    internal static func fromUniFFI(_ r: UniFfiChapterInfo) -> TTZipChapterInfo {
        TTZipChapterInfo(
            chapterId: r.chapterId,
            title: r.title,
            startTimeSeconds: r.startTimeSeconds,
            endTimeSeconds: r.endTimeSeconds
        )
    }
}

/// Comprehensive high-level video container and media stream metadata record.
public struct TTZipVideoMetadata: Sendable, Equatable, Hashable, Identifiable {
    public var id: String {
        "\(format.rawValue)_\(fileSizeBytes)_\(durationSeconds)"
    }
    public var format: TTZipVideoFormat
    public var containerName: String
    public var durationSeconds: Double
    public var fileSizeBytes: UInt64
    public var bitrateKbps: Int
    public var videoTracks: [TTZipVideoTrackInfo]
    public var audioTracks: [TTZipVideoAudioTrackInfo]
    public var subtitleTracks: [TTZipSubtitleTrackInfo]
    public var chapters: [TTZipChapterInfo]
    public var title: String?
    public var artistOrDirector: String?
    public var creationDate: String?
    public var encoder: String?
    public var hasCover: Bool
    public var coverMimeType: String?
    public var extraTags: [String: String]

    /// Primary video track if present.
    public var primaryVideoTrack: TTZipVideoTrackInfo? {
        videoTracks.first
    }

    /// Primary audio track if present.
    public var primaryAudioTrack: TTZipVideoAudioTrackInfo? {
        audioTracks.first(where: { $0.isDefault }) ?? audioTracks.first
    }

    /// Formatted duration string (e.g. "01:23:45" or "04:32").
    public var durationFormatted: String {
        let totalSecs = Int(durationSeconds.rounded())
        let hours = totalSecs / 3600
        let minutes = (totalSecs % 3600) / 60
        let seconds = totalSecs % 60
        if hours > 0 {
            return String(format: "%02d:%02d:%02d", hours, minutes, seconds)
        } else {
            return String(format: "%02d:%02d", minutes, seconds)
        }
    }

    /// Converts from UniFFI video metadata record.
    internal static func fromUniFFI(_ r: UniFfiVideoMetadata) -> TTZipVideoMetadata {
        TTZipVideoMetadata(
            format: TTZipVideoFormat.fromUniFFI(r.format),
            containerName: r.containerName,
            durationSeconds: r.durationSeconds,
            fileSizeBytes: r.fileSizeBytes,
            bitrateKbps: Int(r.bitrateKbps),
            videoTracks: r.videoTracks.map { TTZipVideoTrackInfo.fromUniFFI($0) },
            audioTracks: r.audioTracks.map { TTZipVideoAudioTrackInfo.fromUniFFI($0) },
            subtitleTracks: r.subtitleTracks.map { TTZipSubtitleTrackInfo.fromUniFFI($0) },
            chapters: r.chapters.map { TTZipChapterInfo.fromUniFFI($0) },
            title: r.title,
            artistOrDirector: r.artistOrDirector,
            creationDate: r.creationDate,
            encoder: r.encoder,
            hasCover: r.hasCover,
            coverMimeType: r.coverMimeType,
            extraTags: r.extraTags
        )
    }
}

/// Strongly-typed domain errors for video media operations.
public enum TTZipVideoError: LocalizedError, Sendable, Equatable {
    case unsupportedFormat(format: String)
    case parseError(message: String)
    case ioError(message: String)
    case corruptedData
    case trackNotFound(trackId: UInt32)
    case coverArtNotFound
    case invalidParameter(parameter: String)
    case cancelled

    public var errorDescription: String? {
        switch self {
        case .unsupportedFormat(let format):
            return "Unsupported video format: \(format)"
        case .parseError(let msg):
            return "Video parse error: \(msg)"
        case .ioError(let msg):
            return "I/O error during video operation: \(msg)"
        case .corruptedData:
            return "Video stream corrupted or truncated"
        case .trackNotFound(let trackId):
            return "Track not found: \(trackId)"
        case .coverArtNotFound:
            return "Cover art not found in video container"
        case .invalidParameter(let param):
            return "Invalid video parameter: \(param)"
        case .cancelled:
            return "Video operation was cancelled"
        }
    }

    /// Maps UniFfiVideoError to domain TTZipVideoError.
    internal static func fromUniFFI(_ err: UniFfiVideoError) -> TTZipVideoError {
        switch err {
        case .UnsupportedFormat(let format):
            return .unsupportedFormat(format: format)
        case .ParseError(let message):
            return .parseError(message: message)
        case .IoError(let message):
            return .ioError(message: message)
        case .CorruptedData:
            return .corruptedData
        case .TrackNotFound(let trackId):
            return .trackNotFound(trackId: trackId)
        case .CoverArtNotFound:
            return .coverArtNotFound
        case .InvalidParameter(let parameter):
            return .invalidParameter(parameter: parameter)
        case .Cancelled:
            return .cancelled
        }
    }
}

// MARK: - In-Memory Cache Wrapper

private final class CachedMetadataBox: @unchecked Sendable {
    let metadata: TTZipVideoMetadata
    init(metadata: TTZipVideoMetadata) {
        self.metadata = metadata
    }
}

// MARK: - Swift 6 Video Metadata Facade Service

/// High-performance Swift 6 facade service for zero-allocation video probing, track metadata, and cover art extraction.
@Observable
public final class TTZipVideoMetadataService: @unchecked Sendable {

    /// Shared singleton instance.
    public static let shared = TTZipVideoMetadataService()

    private let logger = Logger(subsystem: "com.ttzip.core", category: "VideoMetadataService")
    private let uniffiService: UniFfiVideoService
    private let metadataCache = NSCache<NSString, CachedMetadataBox>()
    private let coverCache = NSCache<NSString, NSData>()

    /// Public initializer.
    public init() {
        self.uniffiService = UniFfiVideoService()
        self.metadataCache.countLimit = 128
        self.coverCache.countLimit = 64
    }

    // MARK: - Synchronous Probing & Extraction

    /// Probes and extracts comprehensive video metadata from an in-memory byte buffer.
    public func probe(bytes: Data, fileName: String? = nil) throws -> TTZipVideoMetadata {
        do {
            let record = try uniffiService.probeBytes(data: bytes, fileName: fileName)
            let result = TTZipVideoMetadata.fromUniFFI(record)
            return result
        } catch let uniffiErr as UniFfiVideoError {
            throw TTZipVideoError.fromUniFFI(uniffiErr)
        } catch {
            throw TTZipVideoError.parseError(message: error.localizedDescription)
        }
    }

    /// Probes and extracts comprehensive video metadata from a local file URL on disk.
    public func probe(fileURL: URL) throws -> TTZipVideoMetadata {
        let cacheKey = fileURL.path as NSString
        if let cached = metadataCache.object(forKey: cacheKey) {
            return cached.metadata
        }

        do {
            let record = try uniffiService.probeFile(filePath: fileURL.path)
            let result = TTZipVideoMetadata.fromUniFFI(record)
            metadataCache.setObject(CachedMetadataBox(metadata: result), forKey: cacheKey)
            return result
        } catch let uniffiErr as UniFfiVideoError {
            throw TTZipVideoError.fromUniFFI(uniffiErr)
        } catch {
            throw TTZipVideoError.parseError(message: error.localizedDescription)
        }
    }

    /// Extracts comprehensive video metadata from in-memory video bytes.
    public func extractMetadata(bytes: Data, fileName: String? = nil) throws -> TTZipVideoMetadata {
        try probe(bytes: bytes, fileName: fileName)
    }

    /// Extracts comprehensive video metadata from a local file URL.
    public func extractMetadata(fileURL: URL) throws -> TTZipVideoMetadata {
        try probe(fileURL: fileURL)
    }

    /// Extracts raw embedded poster or cover art bytes from in-memory video bytes.
    public func extractCover(bytes: Data, fileName: String? = nil) throws -> Data {
        do {
            let rawBytes = try uniffiService.extractCover(data: bytes, fileName: fileName)
            return rawBytes
        } catch let uniffiErr as UniFfiVideoError {
            throw TTZipVideoError.fromUniFFI(uniffiErr)
        } catch {
            throw TTZipVideoError.parseError(message: error.localizedDescription)
        }
    }

    /// Extracts raw embedded poster or cover art bytes from a local file URL on disk.
    public func extractCover(fileURL: URL) throws -> Data {
        let cacheKey = (fileURL.path + "_cover") as NSString
        if let cached = coverCache.object(forKey: cacheKey) {
            return cached as Data
        }

        do {
            let rawBytes = try uniffiService.extractCoverFromFile(filePath: fileURL.path)
            coverCache.setObject(rawBytes as NSData, forKey: cacheKey)
            return rawBytes
        } catch let uniffiErr as UniFfiVideoError {
            throw TTZipVideoError.fromUniFFI(uniffiErr)
        } catch {
            throw TTZipVideoError.parseError(message: error.localizedDescription)
        }
    }

    #if canImport(AppKit)
    /// Extracts embedded cover art rendered as an `NSImage` if present.
    public func extractCoverImage(bytes: Data, fileName: String? = nil) throws -> NSImage? {
        let coverData = try extractCover(bytes: bytes, fileName: fileName)
        return NSImage(data: coverData)
    }

    /// Extracts embedded cover art rendered as an `NSImage` from a local file URL.
    public func extractCoverImage(fileURL: URL) throws -> NSImage? {
        let coverData = try extractCover(fileURL: fileURL)
        return NSImage(data: coverData)
    }
    #endif

    // MARK: - Asynchronous APIs with Task Cancellation

    /// Asynchronously probes video metadata from an in-memory byte buffer with cooperative cancellation.
    public func probeAsync(bytes: Data, fileName: String? = nil) async throws -> TTZipVideoMetadata {
        try Task.checkCancellation()
        return try await Task.detached(priority: .userInitiated) {
            try Task.checkCancellation()
            return try self.probe(bytes: bytes, fileName: fileName)
        }.value
    }

    /// Asynchronously probes video metadata from a local file URL with cooperative cancellation.
    public func probeAsync(fileURL: URL) async throws -> TTZipVideoMetadata {
        try Task.checkCancellation()
        return try await Task.detached(priority: .userInitiated) {
            try Task.checkCancellation()
            return try self.probe(fileURL: fileURL)
        }.value
    }

    /// Asynchronously extracts raw embedded cover art bytes from in-memory video bytes.
    public func extractCoverAsync(bytes: Data, fileName: String? = nil) async throws -> Data {
        try Task.checkCancellation()
        return try await Task.detached(priority: .userInitiated) {
            try Task.checkCancellation()
            return try self.extractCover(bytes: bytes, fileName: fileName)
        }.value
    }

    #if canImport(AppKit)
    /// Asynchronously extracts embedded cover image as `NSImage`.
    public func extractCoverImageAsync(bytes: Data, fileName: String? = nil) async throws -> NSImage? {
        let coverData = try await extractCoverAsync(bytes: bytes, fileName: fileName)
        return NSImage(data: coverData)
    }
    #endif

    // MARK: - Cache Management

    /// Clears all cached video metadata and cover art image memory.
    public func clearCache() {
        metadataCache.removeAllObjects()
        coverCache.removeAllObjects()
    }
}
