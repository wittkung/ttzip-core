// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipAudioPlaybackServiceTests: XCTestCase {

    private var sandbox: IsolatedTempSandbox!
    private let service = TTZipAudioPlaybackService.shared

    override func setUp() async throws {
        try await super.setUp()
        sandbox = try IsolatedTempSandbox(prefix: "AudioPlaybackTest")
        service.clearCache()
    }

    override func tearDown() async throws {
        service.clearCache()
        sandbox?.cleanup()
        sandbox = nil
        try await super.tearDown()
    }

    // MARK: - Helper Methods

    /// Creates a valid synthetic 16-bit uncompressed PCM WAV byte buffer.
    private func createTestWav(
        sampleRate: Int = 44100,
        channels: Int = 2,
        numSamplesPerChan: Int = 4410
    ) -> Data {
        let bitsPerSample = 16
        let blockAlign = channels * (bitsPerSample / 8)
        let byteRate = sampleRate * blockAlign
        let dataLen = numSamplesPerChan * blockAlign
        let riffLen = 36 + dataLen

        var data = Data()
        // RIFF header
        data.append(contentsOf: "RIFF".utf8)
        var riffLenLe = UInt32(riffLen).littleEndian
        data.append(Data(bytes: &riffLenLe, count: 4))
        data.append(contentsOf: "WAVE".utf8)

        // fmt chunk
        data.append(contentsOf: "fmt ".utf8)
        var subchunk1Size = UInt32(16).littleEndian
        data.append(Data(bytes: &subchunk1Size, count: 4))
        var audioFormat = UInt16(1).littleEndian // PCM = 1
        data.append(Data(bytes: &audioFormat, count: 2))
        var numChannelsLe = UInt16(channels).littleEndian
        data.append(Data(bytes: &numChannelsLe, count: 2))
        var sampleRateLe = UInt32(sampleRate).littleEndian
        data.append(Data(bytes: &sampleRateLe, count: 4))
        var byteRateLe = UInt32(byteRate).littleEndian
        data.append(Data(bytes: &byteRateLe, count: 4))
        var blockAlignLe = UInt16(blockAlign).littleEndian
        data.append(Data(bytes: &blockAlignLe, count: 2))
        var bitsPerSampleLe = UInt16(bitsPerSample).littleEndian
        data.append(Data(bytes: &bitsPerSampleLe, count: 2))

        // data chunk
        data.append(contentsOf: "data".utf8)
        var dataLenLe = UInt32(dataLen).littleEndian
        data.append(Data(bytes: &dataLenLe, count: 4))

        // Synthetic 440Hz sine wave
        for i in 0..<numSamplesPerChan {
            let t = Double(i) / Double(sampleRate)
            let val = sin(t * 440.0 * 2.0 * .pi)
            var sample = Int16(val * 24000.0).littleEndian
            for _ in 0..<channels {
                data.append(Data(bytes: &sample, count: 2))
            }
        }

        return data
    }

    // MARK: - 1. Format Kind & Extension Inference Tests

    func testAudioFormatInference() {
        XCTAssertEqual(TTZipAudioFormat.from(pathOrExtension: "song.mp3"), .mp3)
        XCTAssertEqual(TTZipAudioFormat.from(pathOrExtension: "track.aac"), .aac)
        XCTAssertEqual(TTZipAudioFormat.from(pathOrExtension: "voice.m4a"), .m4a)
        XCTAssertEqual(TTZipAudioFormat.from(pathOrExtension: "music.flac"), .flac)
        XCTAssertEqual(TTZipAudioFormat.from(pathOrExtension: "sample.wav"), .wav)
        XCTAssertEqual(TTZipAudioFormat.from(pathOrExtension: "audio.aiff"), .aiff)
        XCTAssertEqual(TTZipAudioFormat.from(pathOrExtension: "stream.ogg"), .ogg)
        XCTAssertEqual(TTZipAudioFormat.from(pathOrExtension: "file.alac"), .alac)
        XCTAssertEqual(TTZipAudioFormat.from(pathOrExtension: "recording.caf"), .caf)
        XCTAssertEqual(TTZipAudioFormat.from(pathOrExtension: "clip.opus"), .opus)
        XCTAssertEqual(TTZipAudioFormat.from(pathOrExtension: "unknown.xyz"), .unknown)

        XCTAssertEqual(TTZipAudioFormat.mp3.displayName, "MPEG-3 Audio (MP3)")
        XCTAssertEqual(TTZipAudioFormat.flac.displayName, "Free Lossless Audio Codec (FLAC)")
        XCTAssertEqual(TTZipAudioFormat.wav.displayName, "Waveform Audio File Format (WAV)")

        XCTAssertEqual(TTZipAudioFormat.mp3.mimeType, "audio/mpeg")
        XCTAssertEqual(TTZipAudioFormat.wav.mimeType, "audio/wav")
        XCTAssertEqual(TTZipAudioFormat.flac.mimeType, "audio/flac")

        XCTAssertTrue(TTZipAudioFormat.flac.isLossless)
        XCTAssertTrue(TTZipAudioFormat.wav.isLossless)
        XCTAssertFalse(TTZipAudioFormat.mp3.isLossless)
    }

    // MARK: - 2. Domain Models Formatting & Utilities Tests

    func testDomainModelsFormatting() {
        let streamInfo = TTZipAudioStreamInfo(
            codecName: "pcm_s16le",
            codecLongName: "PCM 16-bit little-endian",
            sampleRate: 48000,
            channels: 2,
            channelLayout: "stereo",
            bitsPerSample: 16,
            bitRate: 1536000,
            durationSeconds: 125.5
        )

        XCTAssertEqual(streamInfo.formattedDuration, "2:05")
        XCTAssertEqual(streamInfo.formattedSampleRate, "48.0 kHz")
        XCTAssertEqual(streamInfo.formattedBitRate, "1536 kbps")

        let cover = TTZipAudioCoverArt(
            mimeType: "image/jpeg",
            width: 600,
            height: 600,
            data: Data(repeating: 0xFF, count: 2048),
            descriptionText: "Front Cover"
        )
        XCTAssertEqual(cover.dimensionsString, "600 × 600 px")
        XCTAssertFalse(cover.formattedDataSize.isEmpty)

        let meta = TTZipAudioMetadata(
            id: "/path/to/song.wav",
            title: "Summer Vibes",
            artist: "Awesome Band",
            album: "Best Hits",
            trackNumber: 3,
            trackTotal: 12,
            discNumber: 1,
            discTotal: 1,
            streamInfo: streamInfo,
            fileSizeBytes: 24000000,
            containerFormat: "wav"
        )

        XCTAssertEqual(meta.displayTitle, "Summer Vibes")
        XCTAssertEqual(meta.displayArtist, "Awesome Band")
        XCTAssertEqual(meta.formattedTrackNumber, "3/12")
        XCTAssertFalse(meta.formattedFileSize.isEmpty)

        let waveform = TTZipAudioWaveform(
            amplitudes: [0.1, 0.5, 0.9, 0.3],
            bucketCount: 4,
            durationSeconds: 10.0,
            sampleRate: 44100,
            channels: 2
        )
        XCTAssertEqual(waveform.amplitude(at: 0.0), 0.1, accuracy: 0.01)
        XCTAssertEqual(waveform.amplitude(at: 0.5), 0.9, accuracy: 0.01)
        XCTAssertEqual(waveform.amplitude(at: 1.0), 0.3, accuracy: 0.01)
    }

    // MARK: - 3. Audio Metadata Probing Tests

    func testAudioMetadataProbing() async throws {
        let wavData = createTestWav(sampleRate: 44100, channels: 2, numSamplesPerChan: 4410)
        let wavURL = sandbox.fileURL(named: "test_probe.wav")
        try wavData.write(to: wavURL)

        // 1. In-memory probing
        let memMeta = try await service.probeMetadata(data: wavData, fileName: "test_probe.wav")
        XCTAssertEqual(memMeta.containerFormat, "wav")
        XCTAssertEqual(memMeta.streamInfo.sampleRate, 44100)
        XCTAssertEqual(memMeta.streamInfo.channels, 2)
        XCTAssertEqual(memMeta.streamInfo.channelLayout, "stereo")
        XCTAssertEqual(memMeta.displayTitle, "test_probe.wav")

        // 2. File URL probing with caching
        let fileMeta = try await service.probeMetadata(url: wavURL)
        XCTAssertEqual(fileMeta.containerFormat, "wav")
        XCTAssertEqual(fileMeta.streamInfo.sampleRate, 44100)
        XCTAssertEqual(fileMeta.streamInfo.channels, 2)

        // Verify cached retrieval
        let cachedMeta = try await service.probeMetadata(url: wavURL)
        XCTAssertEqual(cachedMeta.id, fileMeta.id)
    }

    // MARK: - 4. Waveform Generation Tests

    func testAudioWaveformGeneration() async throws {
        let wavData = createTestWav(sampleRate: 48000, channels: 2, numSamplesPerChan: 9600)
        let wavURL = sandbox.fileURL(named: "waveform_test.wav")
        try wavData.write(to: wavURL)

        // 1. In-memory waveform generation
        let memWaveform = try await service.generateWaveform(data: wavData, bucketCount: 64, fileName: "waveform_test.wav")
        XCTAssertEqual(memWaveform.bucketCount, 64)
        XCTAssertEqual(memWaveform.amplitudes.count, 64)
        XCTAssertEqual(memWaveform.sampleRate, 48000)
        XCTAssertEqual(memWaveform.channels, 2)

        // 2. File URL waveform generation
        let fileWaveform = try await service.generateWaveform(url: wavURL, bucketCount: 32)
        XCTAssertEqual(fileWaveform.bucketCount, 32)
        XCTAssertEqual(fileWaveform.amplitudes.count, 32)
    }

    // MARK: - 5. PCM Packet Decoding Tests

    func testAudioPacketDecoding() async throws {
        let wavData = createTestWav(sampleRate: 44100, channels: 2, numSamplesPerChan: 4410)
        let wavURL = sandbox.fileURL(named: "decode_test.wav")
        try wavData.write(to: wavURL)

        // 1. In-memory decoding
        let memPackets = try await service.decodePackets(data: wavData, maxPackets: 5, fileName: "decode_test.wav")
        XCTAssertFalse(memPackets.isEmpty)
        let firstMem = memPackets[0]
        XCTAssertEqual(firstMem.channels, 2)
        XCTAssertEqual(firstMem.sampleRate, 44100)
        XCTAssertFalse(firstMem.pcmF32Samples.isEmpty)

        // 2. File URL decoding
        let filePackets = try await service.decodePackets(url: wavURL, maxPackets: 10)
        XCTAssertFalse(filePackets.isEmpty)
    }

    // MARK: - 6. Cache Management & Error Handling Tests

    func testCacheManagementAndErrors() async {
        service.clearCache()
        XCTAssertNil(service.lastInspectedMetadata)
        XCTAssertNil(service.lastGeneratedWaveform)

        // Expect error on empty buffer
        do {
            _ = try await service.probeMetadata(data: Data(), fileName: "empty.wav")
            XCTFail("Should have thrown error on empty audio buffer")
        } catch {
            XCTAssertNotNil(service.latestError)
        }
    }
}
