// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import XCTest
@testable import TTZipCore

final class DeepFileMetadataReaderTests: XCTestCase {
    
    private var tempDir: URL!
    
    override func setUpWithError() throws {
        try super.setUpWithError()
        tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }
    
    override func tearDownWithError() throws {
        if let tempDir = tempDir {
            try? FileManager.default.removeItem(at: tempDir)
        }
        try super.tearDownWithError()
    }
    
    func testMatroskaAndWebMFallbackInspection() throws {
        // MKV (EBML Header 0x1A 0x45 0xDF 0xA3)
        let mkvURL = tempDir.appendingPathComponent("test_movie.mkv")
        var mkvData = Data([0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81, 0x01, 0x42, 0xF7, 0x81, 0x01])
        mkvData.append(Data("matroska".utf8))
        try mkvData.write(to: mkvURL)
        
        let mkvMeta = try TTZipCore.probeFileMetadata(path: mkvURL.path)
        XCTAssertEqual(mkvMeta.mediaType, .video)
        XCTAssertEqual(mkvMeta.formatName, "Matroska Video (MKV)")
        XCTAssertEqual(mkvMeta.mimeType, "video/x-matroska")
        XCTAssertTrue(mkvMeta.isContainer)
        
        // WebM
        let webmURL = tempDir.appendingPathComponent("test_clip.webm")
        var webmData = Data([0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81, 0x01])
        webmData.append(Data("webm".utf8))
        try webmData.write(to: webmURL)
        
        let webmMeta = try TTZipCore.probeFileMetadata(path: webmURL.path)
        XCTAssertEqual(webmMeta.mediaType, .video)
        XCTAssertEqual(webmMeta.formatName, "WebM Media")
        XCTAssertEqual(webmMeta.mimeType, "video/webm")
        XCTAssertTrue(webmMeta.isContainer)
    }
    
    func testWAVAudioInspection() throws {
        let wavURL = tempDir.appendingPathComponent("sound.wav")
        let wavBytes: [UInt8] = [
            0x52, 0x49, 0x46, 0x46, // "RIFF"
            0x2C, 0x00, 0x00, 0x00, // file length - 8
            0x57, 0x41, 0x56, 0x45, // "WAVE"
            0x66, 0x6D, 0x74, 0x20, // "fmt "
            0x10, 0x00, 0x00, 0x00, // chunk size 16
            0x01, 0x00,             // PCM format = 1
            0x02, 0x00,             // 2 channels
            0x44, 0xAC, 0x00, 0x00, // 44100 Hz
            0x10, 0xB1, 0x02, 0x00, // ByteRate (176400 B/s)
            0x04, 0x00,             // BlockAlign = 4
            0x10, 0x00,             // 16 bits per sample
            0x64, 0x61, 0x74, 0x61, // "data"
            0x00, 0xA6, 0x0E, 0x00  // 960000 bytes
        ]
        try Data(wavBytes).write(to: wavURL)
        
        let wavMeta = try TTZipCore.probeFileMetadata(path: wavURL.path)
        XCTAssertEqual(wavMeta.mediaType, .audio)
        XCTAssertEqual(wavMeta.formatName, "WAV Audio")
        XCTAssertEqual(wavMeta.audio?.channels, 2)
        XCTAssertEqual(wavMeta.audio?.sampleRate, 44100)
        XCTAssertEqual(wavMeta.audio?.bitDepth, 16)
    }
    
    func testFLACAudioInspection() throws {
        let flacURL = tempDir.appendingPathComponent("track.flac")
        var flacData = Data([0x66, 0x4C, 0x61, 0x43, 0x80, 0x00, 0x00, 0x22]) // fLaC header + STREAMINFO
        var sinfo = [UInt8](repeating: 0, count: 34)
        sinfo[10] = UInt8(44100 >> 12)
        sinfo[11] = UInt8((44100 >> 4) & 0xFF)
        sinfo[12] = UInt8(((44100 & 0x0F) << 4) | (1 << 1))
        sinfo[13] = 15 << 4
        sinfo[15] = UInt8((441000 >> 16) & 0xFF)
        sinfo[16] = UInt8((441000 >> 8) & 0xFF)
        sinfo[17] = UInt8(441000 & 0xFF)
        flacData.append(Data(sinfo))
        try flacData.write(to: flacURL)
        
        let flacMeta = try TTZipCore.probeFileMetadata(path: flacURL.path)
        XCTAssertEqual(flacMeta.mediaType, .audio)
        XCTAssertEqual(flacMeta.formatName, "FLAC Audio")
        XCTAssertEqual(flacMeta.audio?.channels, 2)
        XCTAssertEqual(flacMeta.audio?.sampleRate, 44100)
        XCTAssertEqual(flacMeta.audio?.bitDepth, 16)
    }
    
    func testOggAudioInspection() throws {
        let oggURL = tempDir.appendingPathComponent("stream.ogg")
        var oggData = Data("OggS".utf8)
        oggData.append(Data(repeating: 0, count: 24))
        oggData.append(Data("\u{01}vorbis".utf8))
        var vHeader = [UInt8](repeating: 0, count: 24)
        vHeader[11] = 2 // 2 channels
        vHeader[12] = 0x44; vHeader[13] = 0xAC; vHeader[14] = 0x00; vHeader[15] = 0x00 // 44100 Hz
        vHeader[20] = 0x00; vHeader[21] = 0xE1; vHeader[22] = 0x01; vHeader[23] = 0x00 // 123 kbps
        oggData.append(Data(vHeader))
        try oggData.write(to: oggURL)
        
        let oggMeta = try TTZipCore.probeFileMetadata(path: oggURL.path)
        XCTAssertEqual(oggMeta.mediaType, .audio)
        XCTAssertEqual(oggMeta.formatName, "Ogg Audio")
        XCTAssertEqual(oggMeta.mimeType, "audio/ogg")
    }
    
    func testMP3AudioInspection() throws {
        let mp3URL = tempDir.appendingPathComponent("song.mp3")
        let mp3Data = Data([0x49, 0x44, 0x33, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20] + [UInt8](repeating: 0, count: 32))
        try mp3Data.write(to: mp3URL)
        
        let mp3Meta = try TTZipCore.probeFileMetadata(path: mp3URL.path)
        XCTAssertEqual(mp3Meta.mediaType, .audio)
        XCTAssertEqual(mp3Meta.mimeType, "audio/mpeg")
    }
    
    func testAIFFAudioInspection() throws {
        let aiffURL = tempDir.appendingPathComponent("sound.aiff")
        let aiffData = Data("FORM\0\0\0,AIFFCOMM\0\0\0\u{12}\0\u{02}\0\u{06}\u{BA}\u{A8}\0\u{10}\u{40}\u{0E}\u{AC}\u{44}\0\0\0\0\0\0".utf8)
        try aiffData.write(to: aiffURL)
        
        let aiffMeta = try TTZipCore.probeFileMetadata(path: aiffURL.path)
        XCTAssertEqual(aiffMeta.mediaType, .audio)
        XCTAssertEqual(aiffMeta.formatName, "AIFF Audio")
        XCTAssertEqual(aiffMeta.audio?.channels, 2)
        XCTAssertEqual(aiffMeta.audio?.sampleRate, 44100)
    }
    
    func testMP4VideoInspection() throws {
        let mp4URL = tempDir.appendingPathComponent("video.mp4")
        var mp4Data = Data([
            0x00, 0x00, 0x00, 0x18, // ftyp box size 24
            0x66, 0x74, 0x79, 0x70, // 'ftyp'
            0x6D, 0x70, 0x34, 0x32, // 'mp42'
            0x00, 0x00, 0x00, 0x00,
            0x69, 0x73, 0x6F, 0x6D,
            0x6D, 0x70, 0x34, 0x32
        ])
        
        // mvhd box
        var mvhdPayload = [UInt8](repeating: 0, count: 100)
        mvhdPayload[12] = 0x00; mvhdPayload[13] = 0x00; mvhdPayload[14] = 0x03; mvhdPayload[15] = 0xE8 // timescale 1000
        mvhdPayload[16] = 0x00; mvhdPayload[17] = 0x00; mvhdPayload[18] = 0x30; mvhdPayload[19] = 0xD4 // duration 12500
        var mvhdData = Data([0x00, 0x00, 0x00, 0x6C, 0x6D, 0x76, 0x68, 0x64])
        mvhdData.append(Data(mvhdPayload))
        
        // tkhd box
        var tkhdPayload = [UInt8](repeating: 0, count: 84)
        tkhdPayload[76] = 0x07; tkhdPayload[77] = 0x80; tkhdPayload[78] = 0x00; tkhdPayload[79] = 0x00 // 1920
        tkhdPayload[80] = 0x04; tkhdPayload[81] = 0x38; tkhdPayload[82] = 0x00; tkhdPayload[83] = 0x00 // 1080
        var tkhdData = Data([0x00, 0x00, 0x00, 0x5C, 0x74, 0x6B, 0x68, 0x64])
        tkhdData.append(Data(tkhdPayload))
        
        // trak box
        var trakData = Data([0x00, 0x00, 0x00, 0x64, 0x74, 0x72, 0x61, 0x6B])
        trakData.append(tkhdData)
        
        // moov box
        var moovData = Data([0x00, 0x00, 0x00, 0xD8, 0x6D, 0x6F, 0x6F, 0x76])
        moovData.append(mvhdData)
        moovData.append(trakData)
        
        mp4Data.append(moovData)
        try mp4Data.write(to: mp4URL)
        
        let mp4Meta = try TTZipCore.probeFileMetadata(path: mp4URL.path)
        XCTAssertEqual(mp4Meta.mediaType, .video)
        XCTAssertEqual(mp4Meta.mimeType, "video/mp4")
        XCTAssertTrue(mp4Meta.isContainer)
    }
    
    func testEmptyFileResilience() throws {
        let emptyURL = tempDir.appendingPathComponent("empty.mkv")
        try Data().write(to: emptyURL)
        
        let meta = try TTZipCore.probeFileMetadata(path: emptyURL.path)
        XCTAssertEqual(meta.fileSize, 0)
        XCTAssertEqual(meta.mediaType, .unknown)
    }
    
    func testBufferMetadataProbe() throws {
        let pngHeader = Data([
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x08, 0x06, 0x00, 0x00, 0x00
        ])
        let pngMeta = try TTZipCore.probeBufferMetadata(data: pngHeader, filenameHint: "test.png")
        XCTAssertEqual(pngMeta.mediaType, .image)
        XCTAssertEqual(pngMeta.formatName, "PNG Image")
        XCTAssertEqual(pngMeta.mimeType, "image/png")
    }
}

