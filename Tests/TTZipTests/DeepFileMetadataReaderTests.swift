// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

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
    
    func testMatroskaAndWebMFallbackInspection() async throws {
        // MKV (EBML Header 0x1A 0x45 0xDF 0xA3)
        let mkvURL = tempDir.appendingPathComponent("test_movie.mkv")
        var mkvData = Data([0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81, 0x01, 0x42, 0xF7, 0x81, 0x01])
        mkvData.append(Data("matroska".utf8))
        try mkvData.write(to: mkvURL)
        
        let mkvMeta = await DeepFileMetadataReader.readMetadata(for: mkvURL)
        XCTAssertEqual(mkvMeta["Container Format"], "Matroska Multimedia Container (MKV)")
        XCTAssertEqual(mkvMeta["Media Category"], "Audio / Video Container")
        XCTAssertEqual(mkvMeta["Container Standard"], "EBML (Extensible Binary Meta Language)")
        
        // WebM
        let webmURL = tempDir.appendingPathComponent("test_clip.webm")
        var webmData = Data([0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81, 0x01])
        webmData.append(Data("webm".utf8))
        try webmData.write(to: webmURL)
        
        let webmMeta = await DeepFileMetadataReader.readMetadata(for: webmURL)
        XCTAssertEqual(webmMeta["Container Format"], "WebM Multimedia Container")
        XCTAssertEqual(webmMeta["Media Category"], "Audio / Video Container")
    }
    
    func testFlashVideoFallbackInspection() async throws {
        let flvURL = tempDir.appendingPathComponent("sample.flv")
        // FLV header: 'F' 'L' 'V', version 0x01, flags 0x05 (Audio + Video), header length 0x00 0x00 0x00 0x09
        let flvData = Data([0x46, 0x4C, 0x56, 0x01, 0x05, 0x00, 0x00, 0x00, 0x09])
        try flvData.write(to: flvURL)
        
        let flvMeta = await DeepFileMetadataReader.readMetadata(for: flvURL)
        XCTAssertEqual(flvMeta["Container Format"], "Adobe Flash Video (FLV)")
        XCTAssertEqual(flvMeta["Media Category"], "Audio & Video Container")
    }
    
    func testRIFFAudioAndVideoInspection() async throws {
        // AVI
        let aviURL = tempDir.appendingPathComponent("clip.avi")
        let aviData = Data("RIFF\0\0\0\0AVI LIST".utf8)
        try aviData.write(to: aviURL)
        
        let aviMeta = await DeepFileMetadataReader.readMetadata(for: aviURL)
        XCTAssertEqual(aviMeta["Container Format"], "Audio Video Interleave (AVI)")
        XCTAssertEqual(aviMeta["Media Category"], "Audio / Video Container")
        
        // WAV with fmt chunk: 44.1kHz, 16-bit stereo PCM
        let wavURL = tempDir.appendingPathComponent("sound.wav")
        var wavData = Data("RIFF\0\0\0\0WAVEfmt ".utf8)
        let fmtChunk = Data([
            0x10, 0x00, 0x00, 0x00, // chunk size 16
            0x01, 0x00,             // PCM format = 1
            0x02, 0x00,             // 2 channels
            0x44, 0xAC, 0x00, 0x00, // 44100 Hz
            0x10, 0xB1, 0x02, 0x00, // ByteRate (176400 B/s = 1411 kbps)
            0x04, 0x00,             // BlockAlign
            0x10, 0x00              // 16 bits per sample
        ])
        wavData.append(fmtChunk)
        try wavData.write(to: wavURL)
        
        let wavMeta = await DeepFileMetadataReader.readMetadata(for: wavURL)
        XCTAssertEqual(wavMeta["Container Format"], "Waveform Audio (WAV)")
        XCTAssertEqual(wavMeta["Audio Format"], "PCM Uncompressed")
        XCTAssertEqual(wavMeta["Audio Channels"], "2 channels (Stereo)")
        XCTAssertEqual(wavMeta["Audio Sample Rate"], "44100 Hz")
        XCTAssertEqual(wavMeta["Bit Depth"], "16 bit")
        XCTAssertEqual(wavMeta["Audio Bitrate"], "1411 kbps")
    }
    
    func testOggAndFLACInspection() async throws {
        // OGG
        let oggURL = tempDir.appendingPathComponent("stream.ogg")
        let oggData = Data("OggS\0\0\0\0\0\0\0\0".utf8)
        try oggData.write(to: oggURL)
        
        let oggMeta = await DeepFileMetadataReader.readMetadata(for: oggURL)
        XCTAssertEqual(oggMeta["Container Format"], "Ogg Multimedia Container (OGG)")
        XCTAssertEqual(oggMeta["Media Category"], "Audio / Bitstream Container")
        
        // FLAC (44.1kHz, 16-bit stereo)
        let flacURL = tempDir.appendingPathComponent("track.flac")
        var flacData = Data("fLaC\0\0\0\0\0\0\0\0\0\0\0\0\0\0".utf8)
        let b18: UInt8 = 0x0A
        let b19: UInt8 = 0xC4
        let b20: UInt8 = 0x42 // sampleRate low nibble 4, channels 1 (2 ch), bit depth high bit 0
        let b21: UInt8 = 0xF0 // bit depth low 4 bits (15 -> 16 bit), total samples top nibble 0
        let b22: UInt8 = 0x00
        let b23: UInt8 = 0x00
        let b24: UInt8 = 0xAC
        let b25: UInt8 = 0x44 // 44100 total samples -> 1 second
        
        let streamInfo = Data([b18, b19, b20, b21, b22, b23, b24, b25])
        flacData.append(streamInfo)
        try flacData.write(to: flacURL)
        
        let flacMeta = await DeepFileMetadataReader.readMetadata(for: flacURL)
        XCTAssertEqual(flacMeta["Container Format"], "Free Lossless Audio Codec (FLAC)")
        XCTAssertEqual(flacMeta["Audio Channels"], "2 channels (Stereo)")
        XCTAssertEqual(flacMeta["Bit Depth"], "16 bit")
        XCTAssertEqual(flacMeta["Audio Sample Rate"], "44100 Hz")
    }
    
    func testMP3Inspection() async throws {
        let mp3URL = tempDir.appendingPathComponent("song.mp3")
        let mp3Data = Data([0x49, 0x44, 0x33, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20]) // ID3v2.4
        try mp3Data.write(to: mp3URL)
        
        let mp3Meta = await DeepFileMetadataReader.readMetadata(for: mp3URL)
        XCTAssertEqual(mp3Meta["Container Format"], "MPEG Audio Layer III (MP3)")
        XCTAssertEqual(mp3Meta["Metadata Tag"], "ID3v2.4.0")
        XCTAssertEqual(mp3Meta["Media Category"], "Audio Stream")
    }
    
    func testPosixPermissionsAndEmptyFileResilience() async throws {
        let emptyURL = tempDir.appendingPathComponent("empty.mkv")
        try Data().write(to: emptyURL)
        
        let meta = await DeepFileMetadataReader.readMetadata(for: emptyURL)
        XCTAssertNotNil(meta["POSIX Permissions"])
        XCTAssertEqual(meta["Container Format"], "MKV Media File")
        XCTAssertEqual(meta["Media Category"], "Audio / Video Container")
    }
    
    func testNonAppleContainersBypassAVFoundation() async throws {
        let extensions = ["mkv", "webm", "avi", "flv", "wmv", "vob", "ogv", "ts"]
        for ext in extensions {
            let fileURL = tempDir.appendingPathComponent("sample.\(ext)")
            try Data([0x00, 0x00, 0x00, 0x00]).write(to: fileURL)
            
            let meta = await DeepFileMetadataReader.readMetadata(for: fileURL)
            XCTAssertNotNil(meta["Container Format"], "Failed to extract fallback container format for .\(ext)")
            XCTAssertNotNil(meta["Media Category"], "Failed to extract fallback media category for .\(ext)")
        }
    }
    
    func testMP4AndMOVBinaryInspection() async throws {
        // MP4 with ftyp (isom) + mvhd (timescale 1000, duration 5000 -> 5.0s) + tkhd (1920x1080) + avc1
        let mp4URL = tempDir.appendingPathComponent("video.mp4")
        var mp4Data = Data([
            0x00, 0x00, 0x00, 0x18, // ftyp box size 24
            0x66, 0x74, 0x79, 0x70, // 'ftyp'
            0x69, 0x73, 0x6F, 0x6D, // 'isom'
            0x00, 0x00, 0x02, 0x00, // minor version
            0x69, 0x73, 0x6F, 0x6D, // compatible brand isom
            0x6D, 0x70, 0x34, 0x32  // compatible brand mp42
        ])
        
        // mvhd box (version 0, timescale = 1000, duration = 5000)
        var mvhdData = Data([
            0x00, 0x00, 0x00, 0x6C, // size 108
            0x6D, 0x76, 0x68, 0x64, // 'mvhd'
            0x00, 0x00, 0x00, 0x00, // version 0, flags 0
            0x00, 0x00, 0x00, 0x00, // creation time
            0x00, 0x00, 0x00, 0x00, // mod time
            0x00, 0x00, 0x03, 0xE8, // timescale = 1000 (0x03E8)
            0x00, 0x00, 0x13, 0x88  // duration = 5000 (0x1388) -> 5.0 s
        ])
        mvhdData.append(Data(repeating: 0, count: 80))
        mp4Data.append(mvhdData)
        
        // tkhd box (version 0, width = 1920 << 16, height = 1080 << 16)
        var tkhdData = Data([
            0x00, 0x00, 0x00, 0x5C, // size 92
            0x74, 0x6B, 0x68, 0x64, // 'tkhd'
            0x00, 0x00, 0x00, 0x01  // version 0, flags 1
        ])
        tkhdData.append(Data(repeating: 0, count: 72)) // padding up to width offset (76 bytes from header start)
        tkhdData.append(Data([0x07, 0x80, 0x00, 0x00])) // 1920 << 16
        tkhdData.append(Data([0x04, 0x38, 0x00, 0x00])) // 1080 << 16
        mp4Data.append(tkhdData)
        
        // Codec marker avc1
        mp4Data.append(Data("avc1".utf8))
        
        try mp4Data.write(to: mp4URL)
        
        let mp4Meta = await DeepFileMetadataReader.readMetadata(for: mp4URL)
        XCTAssertEqual(mp4Meta["Container Format"], "MPEG-4 Container (MP4)")
        XCTAssertEqual(mp4Meta["Media Category"], "Audio & Video Container")
        XCTAssertEqual(mp4Meta["Total Duration"], "00:05 (5.0 s)")
        XCTAssertEqual(mp4Meta["Video Dimensions"], "1920 × 1080")
        XCTAssertEqual(mp4Meta["Video Codec"], "H.264 / AVC")
    }
    
    func testAACAndAIFFBinaryInspection() async throws {
        // AAC ADTS: 0xFFF sync, MPEG-4, 44100 Hz, 2 ch
        let aacURL = tempDir.appendingPathComponent("stream.aac")
        // ADTS header: 0xFF, 0xF1 (MPEG-4, layer 00, no CRC), 0x50 (Profile 01 = LC, SR 0100 = 44100, private 0, ch MSB 0), 0x80 (ch LSB 2 = stereo)
        let aacData = Data([0xFF, 0xF1, 0x50, 0x80, 0x00, 0x00, 0x00])
        try aacData.write(to: aacURL)
        
        let aacMeta = await DeepFileMetadataReader.readMetadata(for: aacURL)
        XCTAssertEqual(aacMeta["Container Format"], "Advanced Audio Coding (AAC Stream)")
        XCTAssertEqual(aacMeta["Audio Sample Rate"], "44100 Hz")
        XCTAssertEqual(aacMeta["Audio Channels"], "2 channels (Stereo)")
        
        // AIFF: 'FORM' ... 'AIFF' + 'COMM' chunk
        let aiffURL = tempDir.appendingPathComponent("sound.aiff")
        var aiffData = Data("FORM\0\0\0\0AIFFCOMM\0\0\0\0".utf8)
        let commPayload = Data([
            0x00, 0x02,             // 2 channels
            0x00, 0x00, 0xAC, 0x44, // 44100 sample frames
            0x00, 0x10,             // 16 bits per sample
            0x40, 0x0E, 0xAC, 0x44, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 // 80-bit float 44100
        ])
        aiffData.append(commPayload)
        try aiffData.write(to: aiffURL)
        
        let aiffMeta = await DeepFileMetadataReader.readMetadata(for: aiffURL)
        XCTAssertEqual(aiffMeta["Container Format"], "Audio Interchange File Format (AIFF)")
        XCTAssertEqual(aiffMeta["Audio Channels"], "2 channels (Stereo)")
        XCTAssertEqual(aiffMeta["Bit Depth"], "16 bit")
        XCTAssertEqual(aiffMeta["Total Duration"], "00:01 (1.0 s)")
    }
}

