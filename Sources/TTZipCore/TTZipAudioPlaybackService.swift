// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation
import os

// MARK: - Strongly-Typed Audio Formats & Constants

/// Supported audio container and codec classifications.
public enum TTZipAudioFormat: String, Sendable, Equatable, Hashable, CaseIterable {
    case mp3
    case aac
    case m4a
    case flac
    case wav
    case aiff
    case ogg
    case alac
    case caf
    case wma
    case opus
    case unknown

    /// Human-readable display label for the format.
    public var displayName: String {
        switch self {
        case .mp3: return "MPEG-3 Audio (MP3)"
        case .aac: return "Advanced Audio Coding (AAC)"
        case .m4a: return "Apple MPEG-4 Audio (M4A)"
        case .flac: return "Free Lossless Audio Codec (FLAC)"
        case .wav: return "Waveform Audio File Format (WAV)"
        case .aiff: return "Audio Interchange File Format (AIFF)"
        case .ogg: return "Ogg Vorbis Audio (OGG)"
        case .alac: return "Apple Lossless Audio (ALAC)"
        case .caf: return "Core Audio Format (CAF)"
        case .wma: return "Windows Media Audio (WMA)"
        case .opus: return "Opus Interactive Audio Codec"
        case .unknown: return "Unknown Audio Format"
        }
    }

    /// Primary MIME type string associated with this audio format.
    public var mimeType: String {
        switch self {
        case .mp3: return "audio/mpeg"
        case .aac: return "audio/aac"
        case .m4a, .alac: return "audio/mp4"
        case .flac: return "audio/flac"
        case .wav: return "audio/wav"
        case .aiff: return "audio/aiff"
        case .ogg: return "audio/ogg"
        case .caf: return "audio/x-caf"
        case .wma: return "audio/x-ms-wma"
        case .opus: return "audio/opus"
        case .unknown: return "application/octet-stream"
        }
    }

    /// Uniform Type Identifier (UTI) string for macOS system integration.
    public var uniformTypeIdentifier: String {
        switch self {
        case .mp3: return "public.mp3"
        case .aac: return "public.aac-audio"
        case .m4a: return "com.apple.m4a-audio"
        case .flac: return "org.xiph.flac"
        case .wav: return "com.microsoft.waveform-audio"
        case .aiff: return "public.aiff-audio"
        case .ogg: return "org.xiph.ogg-audio"
        case .alac: return "com.apple.apple-lossless-audio"
        case .caf: return "com.apple.coreaudio-format"
        case .wma: return "com.microsoft.windows-media-wma"
        case .opus: return "org.xiph.opus"
        case .unknown: return "public.audio"
        }
    }

    /// Whether this audio format is natively lossless uncompressed or compressed without loss.
    public var isLossless: Bool {
        switch self {
        case .flac, .wav, .aiff, .alac:
            return true
        default:
            return false
        }
    }

    /// Infers audio format from a filename, file path, or extension string.
    public static func from(pathOrExtension: String) -> TTZipAudioFormat {
        let ext = (pathOrExtension as NSString).pathExtension.lowercased()
        let cleanExt = ext.isEmpty ? pathOrExtension.lowercased() : ext
        switch cleanExt {
        case "mp3": return .mp3
        case "aac": return .aac
        case "m4a": return .m4a
        case "flac": return .flac
        case "wav", "wave": return .wav
        case "aiff", "aif", "aifc": return .aiff
        case "ogg", "oga": return .ogg
        case "alac": return .alac
        case "caf": return .caf
        case "wma": return .wma
        case "opus": return .opus
        default: return .unknown
        }
    }
}

// MARK: - Strongly-Typed Domain Models

/// Embedded picture / album artwork extracted from audio tags.
public struct TTZipAudioCoverArt: Sendable, Equatable, Hashable, Identifiable {
    public var id: String
    public var mimeType: String
    public var width: Int?
    public var height: Int?
    public var data: Data
    public var descriptionText: String?

    public init(
        id: String = UUID().uuidString,
        mimeType: String = "image/jpeg",
        width: Int? = nil,
        height: Int? = nil,
        data: Data = Data(),
        descriptionText: String? = nil
    ) {
        self.id = id
        self.mimeType = mimeType
        self.width = width
        self.height = height
        self.data = data
        self.descriptionText = descriptionText
    }

    internal init(from uniffi: UniFfiAudioCoverArt, id: String = UUID().uuidString) {
        self.id = id
        self.mimeType = uniffi.mimeType
        self.width = uniffi.width.map { Int($0) }
        self.height = uniffi.height.map { Int($0) }
        self.data = uniffi.data
        self.descriptionText = uniffi.description
    }

    /// Formatted dimensions string (e.g. "500 × 500 px") if dimensions are known.
    public var dimensionsString: String? {
        guard let w = width, let h = height else { return nil }
        return "\(w) × \(h) px"
    }

    /// Formatted data byte size string (e.g. "145.2 KB").
    public var formattedDataSize: String {
        ByteCountFormatter.string(fromByteCount: Int64(data.count), countStyle: .file)
    }
}

/// Technical stream properties of the primary audio track.
public struct TTZipAudioStreamInfo: Sendable, Equatable, Hashable {
    public var codecName: String
    public var codecLongName: String
    public var sampleRate: Int
    public var channels: Int
    public var channelLayout: String
    public var bitsPerSample: Int?
    public var bitRate: Int64?
    public var durationSeconds: Double
    public var totalFrames: UInt64?

    public init(
        codecName: String = "unknown",
        codecLongName: String = "Unknown Codec",
        sampleRate: Int = 44100,
        channels: Int = 2,
        channelLayout: String = "stereo",
        bitsPerSample: Int? = 16,
        bitRate: Int64? = nil,
        durationSeconds: Double = 0.0,
        totalFrames: UInt64? = nil
    ) {
        self.codecName = codecName
        self.codecLongName = codecLongName
        self.sampleRate = sampleRate
        self.channels = channels
        self.channelLayout = channelLayout
        self.bitsPerSample = bitsPerSample
        self.bitRate = bitRate
        self.durationSeconds = durationSeconds
        self.totalFrames = totalFrames
    }

    internal init(from uniffi: UniFfiAudioStreamInfo) {
        self.codecName = uniffi.codecName
        self.codecLongName = uniffi.codecLongName
        self.sampleRate = Int(uniffi.sampleRate)
        self.channels = Int(uniffi.channels)
        self.channelLayout = uniffi.channelLayout
        self.bitsPerSample = uniffi.bitsPerSample.map { Int($0) }
        self.bitRate = uniffi.bitRate.map { Int64($0) }
        self.durationSeconds = uniffi.durationSeconds
        self.totalFrames = uniffi.totalFrames
    }

    /// Formatted duration string (e.g. "3:45" or "1:02:15").
    public var formattedDuration: String {
        let totalSec = max(0, Int(durationSeconds))
        let hours = totalSec / 3600
        let minutes = (totalSec % 3600) / 60
        let seconds = totalSec % 60
        if hours > 0 {
            return String(format: "%d:%02d:%02d", hours, minutes, seconds)
        } else {
            return String(format: "%d:%02d", minutes, seconds)
        }
    }

    /// Formatted sample rate string (e.g. "44.1 kHz" or "48.0 kHz").
    public var formattedSampleRate: String {
        String(format: "%.1f kHz", Double(sampleRate) / 1000.0)
    }

    /// Formatted bitrate string (e.g. "320 kbps").
    public var formattedBitRate: String {
        guard let br = bitRate, br > 0 else { return "Variable" }
        return "\(br / 1000) kbps"
    }
}

/// Comprehensive high-level acoustic and tag metadata descriptor.
public struct TTZipAudioMetadata: Sendable, Equatable, Identifiable {
    public var id: String
    public var title: String?
    public var artist: String?
    public var album: String?
    public var albumArtist: String?
    public var trackNumber: Int?
    public var trackTotal: Int?
    public var discNumber: Int?
    public var discTotal: Int?
    public var year: String?
    public var genre: String?
    public var composer: String?
    public var lyrics: String?
    public var copyright: String?
    public var coverArt: TTZipAudioCoverArt?
    public var streamInfo: TTZipAudioStreamInfo
    public var fileSizeBytes: Int64
    public var containerFormat: String
    public var extraTags: [String: String]

    public init(
        id: String = UUID().uuidString,
        title: String? = nil,
        artist: String? = nil,
        album: String? = nil,
        albumArtist: String? = nil,
        trackNumber: Int? = nil,
        trackTotal: Int? = nil,
        discNumber: Int? = nil,
        discTotal: Int? = nil,
        year: String? = nil,
        genre: String? = nil,
        composer: String? = nil,
        lyrics: String? = nil,
        copyright: String? = nil,
        coverArt: TTZipAudioCoverArt? = nil,
        streamInfo: TTZipAudioStreamInfo = TTZipAudioStreamInfo(),
        fileSizeBytes: Int64 = 0,
        containerFormat: String = "wav",
        extraTags: [String: String] = [:]
    ) {
        self.id = id
        self.title = title
        self.artist = artist
        self.album = album
        self.albumArtist = albumArtist
        self.trackNumber = trackNumber
        self.trackTotal = trackTotal
        self.discNumber = discNumber
        self.discTotal = discTotal
        self.year = year
        self.genre = genre
        self.composer = composer
        self.lyrics = lyrics
        self.copyright = copyright
        self.coverArt = coverArt
        self.streamInfo = streamInfo
        self.fileSizeBytes = fileSizeBytes
        self.containerFormat = containerFormat
        self.extraTags = extraTags
    }

    internal init(from uniffi: UniFfiAudioMetadata, sourcePath: String) {
        self.id = sourcePath
        self.title = uniffi.title
        self.artist = uniffi.artist
        self.album = uniffi.album
        self.albumArtist = uniffi.albumArtist
        self.trackNumber = uniffi.trackNumber.map { Int($0) }
        self.trackTotal = uniffi.trackTotal.map { Int($0) }
        self.discNumber = uniffi.discNumber.map { Int($0) }
        self.discTotal = uniffi.discTotal.map { Int($0) }
        self.year = uniffi.year
        self.genre = uniffi.genre
        self.composer = uniffi.composer
        self.lyrics = uniffi.lyrics
        self.copyright = uniffi.copyright
        self.coverArt = uniffi.coverArt.map { TTZipAudioCoverArt(from: $0, id: "\(sourcePath)#cover") }
        self.streamInfo = TTZipAudioStreamInfo(from: uniffi.streamInfo)
        self.fileSizeBytes = Int64(uniffi.fileSizeBytes)
        self.containerFormat = uniffi.containerFormat
        self.extraTags = uniffi.extraTags
    }

    /// User-friendly display title falling back to filename.
    public var displayTitle: String {
        if let t = title, !t.isEmpty { return t }
        return (id as NSString).lastPathComponent
    }

    /// User-friendly display artist falling back to "Unknown Artist".
    public var displayArtist: String {
        if let a = artist, !a.isEmpty { return a }
        return "Unknown Artist"
    }

    /// Formatted track numbering string (e.g. "03" or "3/12").
    public var formattedTrackNumber: String? {
        guard let num = trackNumber else { return nil }
        if let total = trackTotal {
            return "\(num)/\(total)"
        }
        return String(format: "%02d", num)
    }

    /// Formatted file size string.
    public var formattedFileSize: String {
        ByteCountFormatter.string(fromByteCount: fileSizeBytes, countStyle: .file)
    }
}

/// Normalized acoustic peak and RMS waveform amplitude envelope.
public struct TTZipAudioWaveform: Sendable, Equatable {
    public var amplitudes: [Float]
    public var bucketCount: Int
    public var durationSeconds: Double
    public var sampleRate: Int
    public var channels: Int
    public var rmsAmplitudes: [Float]

    public init(
        amplitudes: [Float] = [],
        bucketCount: Int = 0,
        durationSeconds: Double = 0.0,
        sampleRate: Int = 44100,
        channels: Int = 2,
        rmsAmplitudes: [Float] = []
    ) {
        self.amplitudes = amplitudes
        self.bucketCount = bucketCount
        self.durationSeconds = durationSeconds
        self.sampleRate = sampleRate
        self.channels = channels
        self.rmsAmplitudes = rmsAmplitudes
    }

    internal init(from uniffi: UniFfiAudioWaveform) {
        self.amplitudes = uniffi.amplitudes
        self.bucketCount = Int(uniffi.bucketCount)
        self.durationSeconds = uniffi.durationSeconds
        self.sampleRate = Int(uniffi.sampleRate)
        self.channels = Int(uniffi.channels)
        self.rmsAmplitudes = uniffi.rmsAmplitudes
    }

    /// Samples the peak amplitude at a normalized progress position `[0.0, 1.0]`.
    public func amplitude(at progress: Double) -> Float {
        guard !amplitudes.isEmpty else { return 0.0 }
        let clamped = max(0.0, min(1.0, progress))
        let index = min(Int(clamped * Double(amplitudes.count)), amplitudes.count - 1)
        return amplitudes[index]
    }
}

/// Decoded chunk packet of floating-point PCM audio samples for streaming playback.
public struct TTZipAudioPacket: Sendable, Equatable, Identifiable {
    public var id: String
    public var ptsMs: UInt64
    public var durationMs: UInt64
    public var channels: Int
    public var sampleRate: Int
    public var pcmF32Samples: [Float]
    public var frameCount: Int
    public var isEof: Bool

    public init(
        id: String = UUID().uuidString,
        ptsMs: UInt64 = 0,
        durationMs: UInt64 = 0,
        channels: Int = 2,
        sampleRate: Int = 44100,
        pcmF32Samples: [Float] = [],
        frameCount: Int = 0,
        isEof: Bool = false
    ) {
        self.id = id
        self.ptsMs = ptsMs
        self.durationMs = durationMs
        self.channels = channels
        self.sampleRate = sampleRate
        self.pcmF32Samples = pcmF32Samples
        self.frameCount = frameCount
        self.isEof = isEof
    }

    internal init(from uniffi: UniFfiAudioPacket, index: Int = 0) {
        self.id = "packet_\(index)_\(uniffi.ptsMs)"
        self.ptsMs = uniffi.ptsMs
        self.durationMs = uniffi.durationMs
        self.channels = Int(uniffi.channels)
        self.sampleRate = Int(uniffi.sampleRate)
        self.pcmF32Samples = uniffi.pcmF32Samples
        self.frameCount = Int(uniffi.frameCount)
        self.isEof = uniffi.isEof
    }
}

// MARK: - Swift 6 Actor-Isolated Background Worker

/// Actor-isolated background worker executing UniFFI C-ABI audio pipelines.
public actor TTZipAudioWorker {
    private let nativeService: UniFfiAudioService

    public init() {
        self.nativeService = UniFfiAudioService()
    }

    /// Probes technical stream parameters from a local file on disk.
    public func probeStreamInfo(at path: String) throws -> TTZipAudioStreamInfo {
        let uniffi = try nativeService.probeFile(filePath: path)
        return TTZipAudioStreamInfo(from: uniffi)
    }

    /// Probes technical stream parameters from in-memory audio bytes.
    public func probeStreamInfo(from data: Data, fileName: String? = nil) throws -> TTZipAudioStreamInfo {
        let uniffi = try nativeService.probeBytes(data: data, fileName: fileName)
        return TTZipAudioStreamInfo(from: uniffi)
    }

    /// Extracts comprehensive metadata tags and embedded cover art from a local file.
    public func extractMetadata(at path: String) throws -> TTZipAudioMetadata {
        let uniffi = try nativeService.extractMetadataFromFile(filePath: path)
        return TTZipAudioMetadata(from: uniffi, sourcePath: path)
    }

    /// Extracts comprehensive metadata tags and embedded cover art from in-memory bytes.
    public func extractMetadata(from data: Data, fileName: String? = nil) throws -> TTZipAudioMetadata {
        let uniffi = try nativeService.extractMetadata(data: data, fileName: fileName)
        return TTZipAudioMetadata(from: uniffi, sourcePath: fileName ?? "memory://audio")
    }

    /// Generates normalized waveform envelope amplitudes from a local file.
    public func generateWaveform(at path: String, bucketCount: Int) throws -> TTZipAudioWaveform {
        let uniffi = try nativeService.generateWaveformFromFile(filePath: path, bucketCount: UInt32(bucketCount))
        return TTZipAudioWaveform(from: uniffi)
    }

    /// Generates normalized waveform envelope amplitudes from in-memory bytes.
    public func generateWaveform(from data: Data, bucketCount: Int, fileName: String? = nil) throws -> TTZipAudioWaveform {
        let uniffi = try nativeService.generateWaveform(data: data, bucketCount: UInt32(bucketCount), fileName: fileName)
        return TTZipAudioWaveform(from: uniffi)
    }

    /// Decodes chunked PCM sample packets from a local file on disk.
    public func decodePackets(at path: String, maxPackets: Int? = nil) throws -> [TTZipAudioPacket] {
        let limit = maxPackets.map { UInt32($0) }
        let uniffiList = try nativeService.decodePacketsFromFile(filePath: path, maxPackets: limit)
        return uniffiList.enumerated().map { TTZipAudioPacket(from: $1, index: $0) }
    }

    /// Decodes chunked PCM sample packets from in-memory audio bytes.
    public func decodePackets(from data: Data, maxPackets: Int? = nil, fileName: String? = nil) throws -> [TTZipAudioPacket] {
        let limit = maxPackets.map { UInt32($0) }
        let uniffiList = try nativeService.decodePackets(data: data, maxPackets: limit, fileName: fileName)
        return uniffiList.enumerated().map { TTZipAudioPacket(from: $1, index: $0) }
    }
}

// MARK: - Swift 6 Observable Facade Service

/// Swift 6 `@Observable` and `Sendable` audio metadata inspection, waveform generation, and playback streaming service.
///
/// Provides zero-extraction streaming inspection of audio documents for UI inspector panels,
/// QuickLook previews, waveform visualization, and PCM sample decoding without landing temporary files to disk.
@Observable
public final class TTZipAudioPlaybackService: @unchecked Sendable {

    // MARK: - Shared Singleton

    public static let shared = TTZipAudioPlaybackService()

    // MARK: - Published Observable Metrics

    /// Indicates whether one or more audio inspection or decoding tasks are actively running.
    public private(set) var isProcessing: Bool = false

    /// Number of concurrent audio operations currently in flight.
    public private(set) var activeOperationsCount: Int = 0

    /// Cumulative total count of audio files processed across the lifetime of this service.
    public private(set) var totalAudiosProcessed: Int = 0

    /// Most recently inspected audio metadata record.
    public private(set) var lastInspectedMetadata: TTZipAudioMetadata? = nil

    /// Most recently generated waveform envelope.
    public private(set) var lastGeneratedWaveform: TTZipAudioWaveform? = nil

    /// Most recent localized error encountered during audio processing.
    public private(set) var latestError: String? = nil

    // MARK: - Internal Storage & Actor Worker

    private let worker = TTZipAudioWorker()

    private struct CacheState {
        var metadataCache: [String: TTZipAudioMetadata] = [:]
        var waveformCache: [String: TTZipAudioWaveform] = [:]
        var streamInfoCache: [String: TTZipAudioStreamInfo] = [:]
    }

    private let lock = OSAllocatedUnfairLock(initialState: CacheState())

    // MARK: - Initialization

    public init() {}

    // MARK: - High-Level Probing & Metadata APIs

    /// Inspects audio metadata and embedded artwork from a local file URL with caching.
    public func probeMetadata(url: URL) async throws -> TTZipAudioMetadata {
        let path = url.path
        if let cached = lock.withLock({ $0.metadataCache[path] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let meta = try await worker.extractMetadata(at: path)
            lock.withLock { $0.metadataCache[path] = meta }
            self.lastInspectedMetadata = meta
            self.totalAudiosProcessed += 1
            return meta
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Inspects audio metadata and embedded artwork directly from in-memory bytes.
    public func probeMetadata(data: Data, fileName: String? = nil) async throws -> TTZipAudioMetadata {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let meta = try await worker.extractMetadata(from: data, fileName: fileName)
            self.lastInspectedMetadata = meta
            self.totalAudiosProcessed += 1
            return meta
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts embedded album cover art image from a local file URL if present.
    public func extractCoverArt(url: URL) async throws -> TTZipAudioCoverArt? {
        let meta = try await probeMetadata(url: url)
        return meta.coverArt
    }

    /// Extracts embedded album cover art image from in-memory audio bytes if present.
    public func extractCoverArt(data: Data, fileName: String? = nil) async throws -> TTZipAudioCoverArt? {
        let meta = try await probeMetadata(data: data, fileName: fileName)
        return meta.coverArt
    }

    // MARK: - Waveform Generation APIs

    /// Generates normalized waveform envelope amplitudes from a local file URL with caching.
    public func generateWaveform(url: URL, bucketCount: Int = 128) async throws -> TTZipAudioWaveform {
        let cacheKey = "\(url.path)@buckets=\(bucketCount)"
        if let cached = lock.withLock({ $0.waveformCache[cacheKey] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let waveform = try await worker.generateWaveform(at: url.path, bucketCount: bucketCount)
            lock.withLock { $0.waveformCache[cacheKey] = waveform }
            self.lastGeneratedWaveform = waveform
            return waveform
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Generates normalized waveform envelope amplitudes from in-memory audio bytes.
    public func generateWaveform(data: Data, bucketCount: Int = 128, fileName: String? = nil) async throws -> TTZipAudioWaveform {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let waveform = try await worker.generateWaveform(from: data, bucketCount: bucketCount, fileName: fileName)
            self.lastGeneratedWaveform = waveform
            return waveform
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    // MARK: - Streaming PCM Sample Decoding APIs

    /// Decodes chunked float PCM samples from a local audio file URL.
    public func decodePackets(url: URL, maxPackets: Int? = nil) async throws -> [TTZipAudioPacket] {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.decodePackets(at: url.path, maxPackets: maxPackets)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Decodes chunked float PCM samples directly from in-memory audio bytes.
    public func decodePackets(data: Data, maxPackets: Int? = nil, fileName: String? = nil) async throws -> [TTZipAudioPacket] {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            return try await worker.decodePackets(from: data, maxPackets: maxPackets, fileName: fileName)
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    // MARK: - Cache Invalidation

    /// Clears all in-memory metadata, waveform, and stream info caches.
    public func clearCache() {
        lock.withLock {
            $0.metadataCache.removeAll()
            $0.waveformCache.removeAll()
            $0.streamInfoCache.removeAll()
        }
        self.lastInspectedMetadata = nil
        self.lastGeneratedWaveform = nil
        self.latestError = nil
    }

    // MARK: - Private Metrics Updaters

    private func updateOperationCount(delta: Int) {
        lock.withLock { _ in
            let newCount = self.activeOperationsCount + delta
            self.activeOperationsCount = max(0, newCount)
            self.isProcessing = self.activeOperationsCount > 0
        }
    }
}
