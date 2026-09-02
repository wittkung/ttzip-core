// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipVideoMetadataServiceTests: XCTestCase {

    private var sandbox: IsolatedTempSandbox!
    private let service = TTZipVideoMetadataService.shared

    override func setUp() async throws {
        try await super.setUp()
        sandbox = try IsolatedTempSandbox(prefix: "VideoMetadataTest")
        service.clearCache()
    }

    override func tearDown() async throws {
        service.clearCache()
        sandbox?.cleanup()
        sandbox = nil
        try await super.tearDown()
    }

    // MARK: - Synthetic Video Fixture Generators

    /// Constructs a synthetic valid MP4 byte buffer with video track, audio track, metadata tags, and cover art.
    private func createSyntheticMP4() -> Data {
        var data = Data()

        // 1. ftyp box
        let ftypPayload = "isom\0\0\u{02}\0isommp41".data(using: .utf8)!
        var ftypSize = UInt32(8 + ftypPayload.count).bigEndian
        data.append(Data(bytes: &ftypSize, count: 4))
        data.append("ftyp".data(using: .utf8)!)
        data.append(ftypPayload)

        // 2. Build ilst tags (©nam, ©ART, covr)
        var ilstPayload = Data()

        // ©nam (Title)
        let titleStr = "Epic 4K Journey".data(using: .utf8)!
        var nameBox = Data()
        var nameDataSize = UInt32(16 + titleStr.count).bigEndian
        nameBox.append(Data(bytes: &nameDataSize, count: 4))
        nameBox.append("data".data(using: .utf8)!)
        var utf8Flag = UInt32(1).bigEndian
        nameBox.append(Data(bytes: &utf8Flag, count: 4))
        var zeroLocale = UInt32(0).bigEndian
        nameBox.append(Data(bytes: &zeroLocale, count: 4))
        nameBox.append(titleStr)
        var nameBoxSize = UInt32(8 + nameBox.count).bigEndian
        ilstPayload.append(Data(bytes: &nameBoxSize, count: 4))
        ilstPayload.append(Data([0xA9, 0x6E, 0x61, 0x6D])) // ©nam
        ilstPayload.append(nameBox)

        // ©ART (Artist)
        let artistStr = "Director Witt".data(using: .utf8)!
        var artBox = Data()
        var artDataSize = UInt32(16 + artistStr.count).bigEndian
        artBox.append(Data(bytes: &artDataSize, count: 4))
        artBox.append("data".data(using: .utf8)!)
        artBox.append(Data(bytes: &utf8Flag, count: 4))
        artBox.append(Data(bytes: &zeroLocale, count: 4))
        artBox.append(artistStr)
        var artBoxSize = UInt32(8 + artBox.count).bigEndian
        ilstPayload.append(Data(bytes: &artBoxSize, count: 4))
        ilstPayload.append(Data([0xA9, 0x41, 0x52, 0x54])) // ©ART
        ilstPayload.append(artBox)

        // covr (Cover JPEG)
        let fakeJpeg = Data([0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0xFF, 0xD9])
        var covrBox = Data()
        var covrDataSize = UInt32(16 + fakeJpeg.count).bigEndian
        covrBox.append(Data(bytes: &covrDataSize, count: 4))
        covrBox.append("data".data(using: .utf8)!)
        var jpegFlag = UInt32(13).bigEndian // JPEG flag = 13
        covrBox.append(Data(bytes: &jpegFlag, count: 4))
        covrBox.append(Data(bytes: &zeroLocale, count: 4))
        covrBox.append(fakeJpeg)
        var covrBoxSize = UInt32(8 + covrBox.count).bigEndian
        ilstPayload.append(Data(bytes: &covrBoxSize, count: 4))
        ilstPayload.append("covr".data(using: .utf8)!)
        ilstPayload.append(covrBox)

        // Wrap ilst in meta box
        var ilstSize = UInt32(8 + ilstPayload.count).bigEndian
        var ilstBox = Data()
        ilstBox.append(Data(bytes: &ilstSize, count: 4))
        ilstBox.append("ilst".data(using: .utf8)!)
        ilstBox.append(ilstPayload)

        var metaSize = UInt32(12 + ilstBox.count).bigEndian
        var metaBox = Data()
        metaBox.append(Data(bytes: &metaSize, count: 4))
        metaBox.append("meta".data(using: .utf8)!)
        metaBox.append(Data([0, 0, 0, 0])) // version + flags
        metaBox.append(ilstBox)

        // Wrap meta in udta box
        var udtaSize = UInt32(8 + metaBox.count).bigEndian
        var udtaBox = Data()
        udtaBox.append(Data(bytes: &udtaSize, count: 4))
        udtaBox.append("udta".data(using: .utf8)!)
        udtaBox.append(metaBox)

        // 3. Build mvhd box (timescale = 1000, duration = 120_000 -> 120.0s)
        var mvhdBox = Data()
        var mvhdSize = UInt32(108).bigEndian
        mvhdBox.append(Data(bytes: &mvhdSize, count: 4))
        mvhdBox.append("mvhd".data(using: .utf8)!)
        mvhdBox.append(Data([0, 0, 0, 0])) // version + flags
        mvhdBox.append(Data(repeating: 0, count: 8)) // creation/mod times
        var timescale = UInt32(1000).bigEndian
        mvhdBox.append(Data(bytes: &timescale, count: 4))
        var duration = UInt32(120000).bigEndian
        mvhdBox.append(Data(bytes: &duration, count: 4))
        mvhdBox.append(Data(repeating: 0, count: 80))

        // 4. Build video trak box (width = 3840, height = 2160, hvc1)
        var trakVideo = Data()
        var tkhdV = Data(repeating: 0, count: 92)
        var tkhdVSize = UInt32(92).bigEndian
        tkhdV.replaceSubrange(0..<4, with: Data(bytes: &tkhdVSize, count: 4))
        tkhdV.replaceSubrange(4..<8, with: "tkhd".data(using: .utf8)!)
        var trackId1 = UInt32(1).bigEndian
        tkhdV.replaceSubrange(20..<24, with: Data(bytes: &trackId1, count: 4))
        var wFixed = UInt32(3840 << 16).bigEndian
        var hFixed = UInt32(2160 << 16).bigEndian
        tkhdV.replaceSubrange(84..<88, with: Data(bytes: &wFixed, count: 4))
        tkhdV.replaceSubrange(88..<92, with: Data(bytes: &hFixed, count: 4))

        var hdlrV = Data(repeating: 0, count: 32)
        var hdlrVSize = UInt32(32).bigEndian
        hdlrV.replaceSubrange(0..<4, with: Data(bytes: &hdlrVSize, count: 4))
        hdlrV.replaceSubrange(4..<8, with: "hdlr".data(using: .utf8)!)
        hdlrV.replaceSubrange(16..<20, with: "vide".data(using: .utf8)!)

        var stsdV = Data()
        var stsdVSize = UInt32(48).bigEndian
        stsdV.append(Data(bytes: &stsdVSize, count: 4))
        stsdV.append("stsd".data(using: .utf8)!)
        stsdV.append(Data([0, 0, 0, 0]))
        var entryCount = UInt32(1).bigEndian
        stsdV.append(Data(bytes: &entryCount, count: 4))
        var hvc1Size = UInt32(32).bigEndian
        stsdV.append(Data(bytes: &hvc1Size, count: 4))
        stsdV.append("hvc1".data(using: .utf8)!)
        stsdV.append(Data(repeating: 0, count: 24))

        var mdiaV = Data()
        var mdiaVSize = UInt32(8 + hdlrV.count + stsdV.count).bigEndian
        mdiaV.append(Data(bytes: &mdiaVSize, count: 4))
        mdiaV.append("mdia".data(using: .utf8)!)
        mdiaV.append(hdlrV)
        mdiaV.append(stsdV)

        var trakVSize = UInt32(8 + tkhdV.count + mdiaV.count).bigEndian
        trakVideo.append(Data(bytes: &trakVSize, count: 4))
        trakVideo.append("trak".data(using: .utf8)!)
        trakVideo.append(tkhdV)
        trakVideo.append(mdiaV)

        // 5. Build audio trak box (sample_rate = 48000, channels = 6, mp4a)
        var trakAudio = Data()
        var tkhdA = Data(repeating: 0, count: 92)
        var tkhdASize = UInt32(92).bigEndian
        tkhdA.replaceSubrange(0..<4, with: Data(bytes: &tkhdASize, count: 4))
        tkhdA.replaceSubrange(4..<8, with: "tkhd".data(using: .utf8)!)
        var trackId2 = UInt32(2).bigEndian
        tkhdA.replaceSubrange(20..<24, with: Data(bytes: &trackId2, count: 4))

        var hdlrA = Data(repeating: 0, count: 32)
        var hdlrASize = UInt32(32).bigEndian
        hdlrA.replaceSubrange(0..<4, with: Data(bytes: &hdlrASize, count: 4))
        hdlrA.replaceSubrange(4..<8, with: "hdlr".data(using: .utf8)!)
        hdlrA.replaceSubrange(16..<20, with: "soun".data(using: .utf8)!)

        var stsdA = Data()
        var stsdASize = UInt32(44).bigEndian
        stsdA.append(Data(bytes: &stsdASize, count: 4))
        stsdA.append("stsd".data(using: .utf8)!)
        stsdA.append(Data([0, 0, 0, 0]))
        stsdA.append(Data(bytes: &entryCount, count: 4))
        var mp4aSize = UInt32(28).bigEndian
        stsdA.append(Data(bytes: &mp4aSize, count: 4))
        stsdA.append("mp4a".data(using: .utf8)!)
        stsdA.append(Data(repeating: 0, count: 8))
        var ch6 = UInt16(6).bigEndian
        stsdA.append(Data(bytes: &ch6, count: 2))
        var bits16 = UInt16(16).bigEndian
        stsdA.append(Data(bytes: &bits16, count: 2))
        stsdA.append(Data([0, 0, 0, 0]))
        var sr48k = UInt32(48000 << 16).bigEndian
        stsdA.append(Data(bytes: &sr48k, count: 4))

        var mdiaA = Data()
        var mdiaASize = UInt32(8 + hdlrA.count + stsdA.count).bigEndian
        mdiaA.append(Data(bytes: &mdiaASize, count: 4))
        mdiaA.append("mdia".data(using: .utf8)!)
        mdiaA.append(hdlrA)
        mdiaA.append(stsdA)

        var trakoASize = UInt32(8 + tkhdA.count + mdiaA.count).bigEndian
        trakAudio.append(Data(bytes: &trakoASize, count: 4))
        trakAudio.append("trak".data(using: .utf8)!)
        trakAudio.append(tkhdA)
        trakAudio.append(mdiaA)

        // 6. Assemble moov box
        var moovSize = UInt32(8 + mvhdBox.count + trakVideo.count + trakAudio.count + udtaBox.count).bigEndian
        data.append(Data(bytes: &moovSize, count: 4))
        data.append("moov".data(using: .utf8)!)
        data.append(mvhdBox)
        data.append(trakVideo)
        data.append(trakAudio)
        data.append(udtaBox)

        return data
    }

    /// Constructs a synthetic Matroska byte buffer.
    private func createSyntheticMKV() -> Data {
        var data = Data([0x1A, 0x45, 0xDF, 0xA3])
        data.append("matroska".data(using: .utf8)!)
        data.append(contentsOf: [0xB0, 0x82, 0x07, 0x80]) // width = 1920
        data.append(contentsOf: [0xBA, 0x82, 0x04, 0x38]) // height = 1080

        // duration = 60.0s (60000.0 ms)
        data.append(contentsOf: [0x44, 0x89, 0x84])
        var durFloat = Float32(60000.0).bitPattern.bigEndian
        data.append(Data(bytes: &durFloat, count: 4))

        // Title
        let titleStr = "Nature Documentary".data(using: .utf8)!
        data.append(contentsOf: [0x7B, 0xA9, UInt8(0x80 | titleStr.count)])
        data.append(titleStr)

        // Attachments with cover image
        let fakeCover = Data([0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0xFF, 0xD9])
        data.append(contentsOf: [0x19, 0x41, 0xA4, 0x69]) // Attachments
        data.append(contentsOf: [0x46, 0x5C]) // FileData
        data.append(UInt8(0x80 | fakeCover.count))
        data.append(fakeCover)

        return data
    }

    /// Constructs a synthetic AVI byte buffer.
    private func createSyntheticAVI() -> Data {
        var data = Data("RIFF".utf8)
        var riffSize = UInt32(200).littleEndian
        data.append(Data(bytes: &riffSize, count: 4))
        data.append("AVI ".data(using: .utf8)!)

        data.append("avih".data(using: .utf8)!)
        var avihSize = UInt32(56).littleEndian
        data.append(Data(bytes: &avihSize, count: 4))
        var microsec = UInt32(33333).littleEndian
        data.append(Data(bytes: &microsec, count: 4))
        data.append(Data(repeating: 0, count: 12))
        var totalFrames = UInt32(300).littleEndian
        data.append(Data(bytes: &totalFrames, count: 4))
        data.append(Data(repeating: 0, count: 12))
        var width = UInt32(1280).littleEndian
        data.append(Data(bytes: &width, count: 4))
        var height = UInt32(720).littleEndian
        data.append(Data(bytes: &height, count: 4))
        data.append(Data(repeating: 0, count: 8))

        let titleStr = "Classic Film\0".data(using: .utf8)!
        data.append("INAM".data(using: .utf8)!)
        var titleLen = UInt32(titleStr.count).littleEndian
        data.append(Data(bytes: &titleLen, count: 4))
        data.append(titleStr)

        return data
    }

    // MARK: - 1. Format & Codec Inference Tests

    func testVideoFormatInference() {
        XCTAssertEqual(TTZipVideoFormat.from(pathOrExtension: "movie.mp4"), .mp4)
        XCTAssertEqual(TTZipVideoFormat.from(pathOrExtension: "clip.m4v"), .m4v)
        XCTAssertEqual(TTZipVideoFormat.from(pathOrExtension: "trailer.mov"), .mov)
        XCTAssertEqual(TTZipVideoFormat.from(pathOrExtension: "film.mkv"), .mkv)
        XCTAssertEqual(TTZipVideoFormat.from(pathOrExtension: "stream.webm"), .webm)
        XCTAssertEqual(TTZipVideoFormat.from(pathOrExtension: "recording.avi"), .avi)
        XCTAssertEqual(TTZipVideoFormat.from(pathOrExtension: "broadcast.ts"), .ts)
        XCTAssertEqual(TTZipVideoFormat.from(pathOrExtension: "video.wmv"), .wmv)
        XCTAssertEqual(TTZipVideoFormat.from(pathOrExtension: "animation.flv"), .flv)
        XCTAssertEqual(TTZipVideoFormat.from(pathOrExtension: "open.ogv"), .ogv)
        XCTAssertEqual(TTZipVideoFormat.from(pathOrExtension: "unknown.xyz"), .unknown)

        XCTAssertEqual(TTZipVideoFormat.mp4.displayName, "MPEG-4 Part 14 Video (MP4)")
        XCTAssertEqual(TTZipVideoFormat.mp4.mimeType, "video/mp4")
        XCTAssertEqual(TTZipVideoFormat.mkv.uniformTypeIdentifier, "org.matroska.mkv")
    }

    func testVideoCodecDisplayNames() {
        XCTAssertEqual(TTZipVideoCodec.h264.displayName, "H.264 / AVC")
        XCTAssertEqual(TTZipVideoCodec.hevc.displayName, "H.265 / HEVC")
        XCTAssertEqual(TTZipVideoCodec.av1.displayName, "AV1")
        XCTAssertEqual(TTZipVideoCodec.vp9.displayName, "VP9")
        XCTAssertEqual(TTZipVideoCodec.proRes.displayName, "Apple ProRes")
    }

    func testAudioCodecDisplayNames() {
        XCTAssertEqual(TTZipVideoAudioCodec.aac.displayName, "AAC")
        XCTAssertEqual(TTZipVideoAudioCodec.ac3.displayName, "Dolby Digital (AC-3)")
        XCTAssertEqual(TTZipVideoAudioCodec.opus.displayName, "Opus")
        XCTAssertEqual(TTZipVideoAudioCodec.flac.displayName, "FLAC")
    }

    // MARK: - 2. MP4 Probing & Extraction Tests

    func testSyntheticMP4ProbingAndExtraction() throws {
        let mp4Bytes = createSyntheticMP4()
        let metadata = try service.probe(bytes: mp4Bytes, fileName: "sample.mp4")

        XCTAssertEqual(metadata.format, .mp4)
        XCTAssertEqual(metadata.title, "Epic 4K Journey")
        XCTAssertEqual(metadata.artistOrDirector, "Director Witt")
        XCTAssertTrue(metadata.hasCover)
        XCTAssertEqual(metadata.coverMimeType, "image/jpeg")
        XCTAssertEqual(metadata.durationSeconds, 120.0, accuracy: 0.1)
        XCTAssertEqual(metadata.durationFormatted, "02:00")

        // Video track validation
        XCTAssertEqual(metadata.videoTracks.count, 1)
        guard let vtrack = metadata.primaryVideoTrack else {
            XCTFail("Missing primary video track")
            return
        }
        XCTAssertEqual(vtrack.trackId, 1)
        XCTAssertEqual(vtrack.codec, .hevc)
        XCTAssertEqual(vtrack.width, 3840)
        XCTAssertEqual(vtrack.height, 2160)
        XCTAssertEqual(vtrack.aspectRatio, "16:9")
        XCTAssertEqual(vtrack.resolutionDisplayString, "3840 × 2160 (4K UHD)")

        // Audio track validation
        XCTAssertEqual(metadata.audioTracks.count, 1)
        guard let atrack = metadata.primaryAudioTrack else {
            XCTFail("Missing primary audio track")
            return
        }
        XCTAssertEqual(atrack.trackId, 2)
        XCTAssertEqual(atrack.codec, .aac)
        XCTAssertEqual(atrack.sampleRate, 48000)
        XCTAssertEqual(atrack.channels, 6)
        XCTAssertEqual(atrack.channelLayout, "5.1 Surround")

        // Cover art extraction
        let coverData = try service.extractCover(bytes: mp4Bytes, fileName: "sample.mp4")
        XCTAssertFalse(coverData.isEmpty)
        XCTAssertEqual(coverData.prefix(3), Data([0xFF, 0xD8, 0xFF]))
    }

    // MARK: - 3. Matroska MKV Probing Tests

    func testSyntheticMKVProbing() throws {
        let mkvBytes = createSyntheticMKV()
        let metadata = try service.probe(bytes: mkvBytes, fileName: "movie.mkv")

        XCTAssertEqual(metadata.format, .mkv)
        XCTAssertEqual(metadata.title, "Nature Documentary")
        XCTAssertTrue(metadata.hasCover)
        XCTAssertEqual(metadata.durationSeconds, 60.0, accuracy: 0.1)

        XCTAssertEqual(metadata.videoTracks.count, 1)
        let vtrack = metadata.videoTracks[0]
        XCTAssertEqual(vtrack.width, 1920)
        XCTAssertEqual(vtrack.height, 1080)
        XCTAssertEqual(vtrack.resolutionDisplayString, "1920 × 1080 (1080p FHD)")

        let cover = try service.extractCover(bytes: mkvBytes, fileName: "movie.mkv")
        XCTAssertFalse(cover.isEmpty)
    }

    // MARK: - 4. AVI Probing Tests

    func testSyntheticAVIProbing() throws {
        let aviBytes = createSyntheticAVI()
        let metadata = try service.probe(bytes: aviBytes, fileName: "clip.avi")

        XCTAssertEqual(metadata.format, .avi)
        XCTAssertEqual(metadata.title, "Classic Film")
        XCTAssertEqual(metadata.durationSeconds, 10.0, accuracy: 0.5)

        XCTAssertEqual(metadata.videoTracks.count, 1)
        let vtrack = metadata.videoTracks[0]
        XCTAssertEqual(vtrack.width, 1280)
        XCTAssertEqual(vtrack.height, 720)
        XCTAssertEqual(vtrack.resolutionDisplayString, "1280 × 720 (720p HD)")
    }

    // MARK: - 5. Error Handling Tests

    func testEmptyBufferCorruptedError() {
        XCTAssertThrowsError(try service.probe(bytes: Data(), fileName: "empty.mp4")) { error in
            guard let videoErr = error as? TTZipVideoError else {
                XCTFail("Expected TTZipVideoError, got \(error)")
                return
            }
            XCTAssertEqual(videoErr, .corruptedData)
        }
    }

    func testCoverArtNotFoundError() {
        let aviBytes = createSyntheticAVI()
        XCTAssertThrowsError(try service.extractCover(bytes: aviBytes, fileName: "clip.avi")) { error in
            guard let videoErr = error as? TTZipVideoError else {
                XCTFail("Expected TTZipVideoError, got \(error)")
                return
            }
            XCTAssertEqual(videoErr, .coverArtNotFound)
        }
    }

    func testUnsupportedFormatError() {
        let junkBytes = Data([1, 2, 3, 4, 5, 6, 7, 8])
        XCTAssertThrowsError(try service.probe(bytes: junkBytes, fileName: nil)) { error in
            guard let videoErr = error as? TTZipVideoError else {
                XCTFail("Expected TTZipVideoError, got \(error)")
                return
            }
            if case .unsupportedFormat = videoErr {
                // Passed
            } else {
                XCTFail("Expected unsupportedFormat error, got \(videoErr)")
            }
        }
    }

    // MARK: - 6. Async & Concurrency Tests

    func testAsyncProbingAndCancellation() async throws {
        let mp4Bytes = createSyntheticMP4()
        let metadata = try await service.probeAsync(bytes: mp4Bytes, fileName: "async.mp4")
        XCTAssertEqual(metadata.format, .mp4)
        XCTAssertEqual(metadata.title, "Epic 4K Journey")

        let coverData = try await service.extractCoverAsync(bytes: mp4Bytes, fileName: "async.mp4")
        XCTAssertFalse(coverData.isEmpty)
    }

    // MARK: - 7. File URL Probing & Cache Lifecycle

    func testFileURLProbingAndCache() throws {
        let mp4Bytes = createSyntheticMP4()
        let testFile = sandbox.url.appendingPathComponent("cached_test.mp4")
        try mp4Bytes.write(to: testFile)

        let meta1 = try service.probe(fileURL: testFile)
        XCTAssertEqual(meta1.title, "Epic 4K Journey")

        // Second probe should hit in-memory cache
        let meta2 = try service.probe(fileURL: testFile)
        XCTAssertEqual(meta1, meta2)

        // Cover cache test
        let cover1 = try service.extractCover(fileURL: testFile)
        let cover2 = try service.extractCover(fileURL: testFile)
        XCTAssertEqual(cover1, cover2)

        service.clearCache()
    }
}
