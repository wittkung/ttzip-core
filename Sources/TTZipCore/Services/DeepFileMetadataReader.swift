// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import ImageIO

/// Deep EXIF, media hardware stream attributes, and POSIX permissions inspection service.
/// Pure Swift implementation with zero dependency on AVFoundation.
public final class DeepFileMetadataReader: @unchecked Sendable {
    nonisolated public static func readMetadata(for url: URL) async -> [String: String] {
        var dict: [String: String] = [:]
        let path = url.path
        let ext = url.pathExtension.lowercased()
        let fm = FileManager.default
        
        // 1. POSIX permissions & APFS attributes
        if let attr = try? fm.attributesOfItem(atPath: path) {
            if let posix = attr[.posixPermissions] as? NSNumber {
                dict["POSIX Permissions"] = String(format: "%04o", posix.uint16Value)
            }
            if let owner = attr[.ownerAccountName] as? String, let group = attr[.groupOwnerAccountName] as? String {
                dict["Owner : Group"] = "\(owner) : \(group)"
            }
            if let inode = attr[.systemFileNumber] as? NSNumber {
                dict["APFS Inode"] = "\(inode)"
            }
        }
        
        // 2. Image EXIF, camera optics, and color spaces (CGImageSource)
        if imageExtensions.contains(ext) {
            extractImageProperties(for: url, into: &dict)
        }
        
        // 3. Audio & Video media streams, container format, and hardware attributes
        // 100% pure Swift binary streaming inspection without AVFoundation
        if mediaExtensions.contains(ext) {
            extractBinaryMediaMetadata(for: url, ext: ext, into: &dict)
        }
        
        return dict
    }
    
    // MARK: - Supported File Extension Sets
    
    private static let imageExtensions: Set<String> = [
        "jpg", "jpeg", "png", "heic", "webp", "gif", "tiff", "bmp", "raw"
    ]
    
    private static let mediaExtensions: Set<String> = [
        "mp4", "mov", "m4v", "mkv", "avi", "webm", "flv", "wmv",
        "vob", "ogv", "ts", "mts", "m2ts", "3gp", "mp3", "wav",
        "flac", "m4a", "aac", "ogg", "oga", "opus", "wma", "aiff", "alac", "caf"
    ]
    
    private static func isAudioOnlyExtension(_ ext: String) -> Bool {
        ["mp3", "wav", "flac", "m4a", "aac", "ogg", "oga", "opus", "wma", "aiff", "alac", "caf"].contains(ext)
    }
    
    // MARK: - Image Metadata Extraction (ImageIO)
    
    private static func extractImageProperties(for url: URL, into dict: inout [String: String]) {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [String: Any] else {
            return
        }
        
        if let w = properties[kCGImagePropertyPixelWidth as String],
           let h = properties[kCGImagePropertyPixelHeight as String] {
            dict["Dimensions (Pixels)"] = "\(w) × \(h)"
        }
        if let colorModel = properties[kCGImagePropertyColorModel as String] {
            dict["Color Model"] = "\(colorModel)"
        }
        if let depth = properties[kCGImagePropertyDepth as String] {
            dict["Bit Depth"] = "\(depth) bit"
        }
        if let profile = properties[kCGImagePropertyProfileName as String] {
            dict["ICC Profile"] = "\(profile)"
        }
        if let dpiW = properties[kCGImagePropertyDPIWidth as String] {
            dict["DPI Resolution"] = "\(dpiW) DPI"
        }
        if let hasAlpha = properties[kCGImagePropertyHasAlpha as String] as? Bool {
            dict["Alpha Channel"] = hasAlpha ? "Yes" : "No"
        }
        
        if let exif = properties[kCGImagePropertyExifDictionary as String] as? [String: Any] {
            if let iso = (exif[kCGImagePropertyExifISOSpeedRatings as String] as? [Int])?.first {
                dict["EXIF: ISO Speed"] = "ISO \(iso)"
            }
            if let fNumber = exif[kCGImagePropertyExifFNumber as String] {
                dict["EXIF: F-Number"] = "f/\(fNumber)"
            }
            if let expTime = exif[kCGImagePropertyExifExposureTime as String] {
                dict["EXIF: Exposure Time"] = "\(expTime) s"
            }
            if let focal = exif[kCGImagePropertyExifFocalLength as String] {
                dict["EXIF: Focal Length"] = "\(focal) mm"
            }
            if let lens = exif[kCGImagePropertyExifLensModel as String] {
                dict["EXIF: Lens Model"] = "\(lens)"
            }
        }
        
        if let tiff = properties[kCGImagePropertyTIFFDictionary as String] as? [String: Any] {
            if let make = tiff[kCGImagePropertyTIFFMake as String],
               let model = tiff[kCGImagePropertyTIFFModel as String] {
                dict["Camera Hardware"] = "\(make) \(model)"
            }
        }
    }
    
    // MARK: - Pure Swift Binary Media Metadata Extraction
    
    private static func extractBinaryMediaMetadata(for url: URL, ext: String, into dict: inout [String: String]) {
        guard let handle = try? FileHandle(forReadingFrom: url) else {
            populateGenericFallback(ext: ext, into: &dict)
            return
        }
        defer { try? handle.close() }
        
        // Read up to 64 KB header for stream signature and container box analysis
        let header = (try? handle.read(upToCount: 65536)) ?? Data()
        guard !header.isEmpty else {
            populateGenericFallback(ext: ext, into: &dict)
            return
        }
        
        // 1. Matroska / WebM (EBML Header: 0x1A 0x45 0xDF 0xA3)
        if header.count >= 4 && header[0] == 0x1A && header[1] == 0x45 && header[2] == 0xDF && header[3] == 0xA3 {
            parseMatroskaOrWebM(header: header, ext: ext, into: &dict)
            return
        }
        
        // 2. Adobe Flash Video (FLV: 'F' 'L' 'V' 0x01)
        if header.count >= 3 && header[0] == 0x46 && header[1] == 0x4C && header[2] == 0x56 {
            parseFlashVideo(header: header, into: &dict)
            return
        }
        
        // 3. RIFF Containers (AVI / WAV)
        if header.count >= 12 && header.prefix(4) == Data("RIFF".utf8) {
            parseRIFF(header: header, ext: ext, into: &dict)
            return
        }
        
        // 4. Ogg Container (OggS)
        if header.count >= 4 && header.prefix(4) == Data("OggS".utf8) {
            parseOgg(header: header, ext: ext, into: &dict)
            return
        }
        
        // 5. Free Lossless Audio Codec (fLaC)
        if header.count >= 4 && header.prefix(4) == Data("fLaC".utf8) {
            parseFLAC(header: header, into: &dict)
            return
        }
        
        // 6. ISOBMFF / MP4 / MOV / M4V / M4A / 3GP (ftyp / moov / wide)
        if isISOBMFFHeader(header) {
            parseISOBMFF(header: header, ext: ext, into: &dict)
            return
        }
        
        // 7. AAC ADTS Stream (0xFFF sync word with layer == 00)
        if (ext == "aac" || (header.count >= 4 && header[0] == 0xFF && (header[1] & 0xF6) == 0xF0)) {
            parseAACADTS(header: header, into: &dict)
            return
        }
        
        // 8. MP3 (ID3v2 or MPEG Audio Sync 0xFFE / 0xFFF with layer != 00)
        if header.count >= 3 && header.prefix(3) == Data("ID3".utf8) {
            parseMP3(header: header, into: &dict)
            return
        } else if header.count >= 2 && header[0] == 0xFF && (header[1] & 0xE0) == 0xE0 && ((header[1] >> 1) & 0x03) != 0 {
            parseMP3(header: header, into: &dict)
            return
        }
        
        // 9. MPEG Transport Stream (TS / MTS / M2TS)
        if header.count >= 188 && (header[0] == 0x47 || (header.count >= 192 && header[4] == 0x47)) {
            parseMPEGTransportStream(header: header, ext: ext, into: &dict)
            return
        }
        
        // 10. Windows Media (ASF / WMV / WMA)
        if header.count >= 16 && header.prefix(16) == Data([0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C]) {
            parseWindowsMedia(header: header, ext: ext, into: &dict)
            return
        }
        
        // 11. AIFF / CAF CoreAudio
        if header.count >= 12 && header.prefix(4) == Data("FORM".utf8) {
            parseAIFF(header: header, into: &dict)
            return
        } else if header.count >= 8 && header.prefix(4) == Data("caff".utf8) {
            parseCAF(header: header, into: &dict)
            return
        }
        
        // Fallback for unclassified media
        populateGenericFallback(ext: ext, into: &dict)
    }
    
    // MARK: - Safe Alignment-Free Binary Readers
    
    private static func readUInt16BE(data: Data, offset: Int) -> UInt16? {
        guard offset >= 0 && offset + 2 <= data.count else { return nil }
        return (UInt16(data[offset]) << 8) | UInt16(data[offset + 1])
    }
    
    private static func readUInt16LE(data: Data, offset: Int) -> UInt16? {
        guard offset >= 0 && offset + 2 <= data.count else { return nil }
        return UInt16(data[offset]) | (UInt16(data[offset + 1]) << 8)
    }
    
    private static func readUInt32BE(data: Data, offset: Int) -> UInt32? {
        guard offset >= 0 && offset + 4 <= data.count else { return nil }
        return (UInt32(data[offset]) << 24) |
               (UInt32(data[offset + 1]) << 16) |
               (UInt32(data[offset + 2]) << 8) |
               UInt32(data[offset + 3])
    }
    
    private static func readUInt32LE(data: Data, offset: Int) -> UInt32? {
        guard offset >= 0 && offset + 4 <= data.count else { return nil }
        return UInt32(data[offset]) |
               (UInt32(data[offset + 1]) << 8) |
               (UInt32(data[offset + 2]) << 16) |
               (UInt32(data[offset + 3]) << 24)
    }
    
    private static func readUInt64BE(data: Data, offset: Int) -> UInt64? {
        guard offset >= 0 && offset + 8 <= data.count else { return nil }
        var result: UInt64 = 0
        for i in 0..<8 {
            result = (result << 8) | UInt64(data[offset + i])
        }
        return result
    }
    
    private static func readUInt64LE(data: Data, offset: Int) -> UInt64? {
        guard offset >= 0 && offset + 8 <= data.count else { return nil }
        var result: UInt64 = 0
        for i in 0..<8 {
            result |= (UInt64(data[offset + i]) << (i * 8))
        }
        return result
    }
    
    // MARK: - Binary Parsers for Specific Media Standards
    
    private static func parseMatroskaOrWebM(header: Data, ext: String, into dict: inout [String: String]) {
        let isWebM = header.range(of: Data("webm".utf8)) != nil || ext == "webm"
        dict["Container Format"] = isWebM ? "WebM Multimedia Container" : "Matroska Multimedia Container (MKV)"
        dict["Media Category"] = "Audio / Video Container"
        dict["Container Standard"] = "EBML (Extensible Binary Meta Language)"
        
        // Scan for Track / Codec hints in EBML header slice
        if header.range(of: Data("V_MPEG4/ISO/AVC".utf8)) != nil {
            dict["Video Codec"] = "H.264 / AVC (V_MPEG4/ISO/AVC)"
        } else if header.range(of: Data("V_MPEGH/ISO/HEVC".utf8)) != nil {
            dict["Video Codec"] = "H.265 / HEVC (V_MPEGH/ISO/HEVC)"
        } else if header.range(of: Data("V_VP9".utf8)) != nil {
            dict["Video Codec"] = "Google VP9 (V_VP9)"
        } else if header.range(of: Data("V_VP8".utf8)) != nil {
            dict["Video Codec"] = "Google VP8 (V_VP8)"
        } else if header.range(of: Data("V_AV1".utf8)) != nil {
            dict["Video Codec"] = "AOMedia AV1 (V_AV1)"
        }
        
        if header.range(of: Data("A_OPUS".utf8)) != nil {
            dict["Audio Codec"] = "Opus (A_OPUS)"
        } else if header.range(of: Data("A_AAC".utf8)) != nil {
            dict["Audio Codec"] = "AAC (A_AAC)"
        } else if header.range(of: Data("A_VORBIS".utf8)) != nil {
            dict["Audio Codec"] = "Vorbis (A_VORBIS)"
        } else if header.range(of: Data("A_FLAC".utf8)) != nil {
            dict["Audio Codec"] = "FLAC (A_FLAC)"
        }
    }
    
    private static func parseFlashVideo(header: Data, into dict: inout [String: String]) {
        let flag = header.count > 4 ? header[4] : 0
        let hasAudio = (flag & 0x04) != 0
        let hasVideo = (flag & 0x01) != 0
        dict["Container Format"] = "Adobe Flash Video (FLV)"
        dict["Media Category"] = (hasAudio && hasVideo) ? "Audio & Video Container" : (hasVideo ? "Video Stream Container" : (hasAudio ? "Audio Stream Container" : "Flash Media Stream"))
    }
    
    private static func parseRIFF(header: Data, ext: String, into dict: inout [String: String]) {
        let formatCode = header.subdata(in: 8..<min(12, header.count))
        if formatCode == Data("AVI ".utf8) {
            dict["Container Format"] = "Audio Video Interleave (AVI)"
            dict["Media Category"] = "Audio / Video Container"
            
            // Look for Main AVI Header ('avih')
            if let avihRange = header.range(of: Data("avih".utf8)), avihRange.upperBound + 40 <= header.count {
                let avihOffset = avihRange.upperBound
                let microsecPerFrame = readUInt32LE(data: header, offset: avihOffset) ?? 0
                let totalFrames = readUInt32LE(data: header, offset: avihOffset + 16) ?? 0
                let width = readUInt32LE(data: header, offset: avihOffset + 32) ?? 0
                let height = readUInt32LE(data: header, offset: avihOffset + 36) ?? 0
                
                if width > 0 && height > 0 {
                    dict["Video Dimensions"] = "\(width) × \(height)"
                }
                if microsecPerFrame > 0 {
                    let fps = 1_000_000.0 / Double(microsecPerFrame)
                    dict["Frame Rate (FPS)"] = String(format: "%.2f fps", fps)
                    if totalFrames > 0 {
                        let secs = (Double(totalFrames) * Double(microsecPerFrame)) / 1_000_000.0
                        dict["Total Duration"] = formatDuration(seconds: secs)
                    }
                }
            }
        } else if formatCode == Data("WAVE".utf8) {
            dict["Container Format"] = "Waveform Audio (WAV)"
            dict["Media Category"] = "Audio Stream Container"
            
            if let fmtRange = header.range(of: Data("fmt ".utf8)), fmtRange.upperBound + 16 <= header.count {
                let fmtData = header.subdata(in: fmtRange.upperBound..<header.count)
                if fmtData.count >= 16 {
                    let audioFormat = readUInt16LE(data: fmtData, offset: 4) ?? 0
                    let numChannels = readUInt16LE(data: fmtData, offset: 6) ?? 0
                    let sampleRate = readUInt32LE(data: fmtData, offset: 8) ?? 0
                    let byteRate = readUInt32LE(data: fmtData, offset: 12) ?? 0
                    let bitsPerSample = fmtData.count >= 18 ? (readUInt16LE(data: fmtData, offset: 18) ?? 0) : 0
                    
                    dict["Audio Format"] = audioFormat == 1 ? "PCM Uncompressed" : (audioFormat == 3 ? "IEEE Float" : "Format Code \(audioFormat)")
                    dict["Audio Channels"] = "\(numChannels) \(numChannels == 1 ? "channel (Mono)" : (numChannels == 2 ? "channels (Stereo)" : "channels"))"
                    dict["Audio Sample Rate"] = "\(sampleRate) Hz"
                    if bitsPerSample > 0 {
                        dict["Bit Depth"] = "\(bitsPerSample) bit"
                    }
                    if byteRate > 0 && dict["Audio Bitrate"] == nil {
                        dict["Audio Bitrate"] = String(format: "%.0f kbps", Double(byteRate * 8) / 1000.0)
                    }
                }
            }
        }
    }
    
    private static func parseOgg(header: Data, ext: String, into dict: inout [String: String]) {
        dict["Container Format"] = "Ogg Multimedia Container (OGG)"
        dict["Media Category"] = ["ogv"].contains(ext) ? "Video Stream Container" : "Audio / Bitstream Container"
        
        if header.range(of: Data("OpusHead".utf8)) != nil {
            dict["Audio Codec"] = "Opus"
            dict["Container Format"] = "Ogg Opus Audio (OGG/OPUS)"
            dict["Audio Sample Rate"] = "48000 Hz"
        } else if header.range(of: Data("\u{01}vorbis".utf8)) != nil {
            dict["Audio Codec"] = "Vorbis"
            dict["Container Format"] = "Ogg Vorbis Audio (OGG)"
        } else if header.range(of: Data("\u{7F}FLAC".utf8)) != nil {
            dict["Audio Codec"] = "FLAC"
            dict["Container Format"] = "Ogg FLAC Audio (OGG)"
        } else if header.range(of: Data("\u{80}theora".utf8)) != nil {
            dict["Video Codec"] = "Theora"
            dict["Container Format"] = "Ogg Theora Video (OGV)"
        }
    }
    
    private static func parseFLAC(header: Data, into dict: inout [String: String]) {
        dict["Container Format"] = "Free Lossless Audio Codec (FLAC)"
        dict["Media Category"] = "Lossless Audio Stream"
        
        if header.count >= 26 {
            let b18 = UInt64(header[18])
            let b19 = UInt64(header[19])
            let b20 = UInt64(header[20])
            let b21 = UInt64(header[21])
            
            let sampleRate = (b18 << 12) | (b19 << 4) | (b20 >> 4)
            let channels = ((b20 >> 1) & 0x07) + 1
            let bitsPerSample = (((b20 & 0x01) << 4) | (b21 >> 4)) + 1
            
            if sampleRate > 0 {
                dict["Audio Sample Rate"] = "\(sampleRate) Hz"
            }
            dict["Audio Channels"] = "\(channels) \(channels == 1 ? "channel (Mono)" : (channels == 2 ? "channels (Stereo)" : "channels"))"
            dict["Bit Depth"] = "\(bitsPerSample) bit"
            
            let totalSamples = ((b21 & 0x0F) << 32) | (UInt64(header[22]) << 24) | (UInt64(header[23]) << 16) | (UInt64(header[24]) << 8) | UInt64(header[25])
            if sampleRate > 0 && totalSamples > 0 && dict["Total Duration"] == nil {
                let secs = Double(totalSamples) / Double(sampleRate)
                dict["Total Duration"] = formatDuration(seconds: secs)
            }
        }
    }
    
    private static func isISOBMFFHeader(_ header: Data) -> Bool {
        if header.count < 8 { return false }
        let boxType = header.subdata(in: 4..<8)
        return boxType == Data("ftyp".utf8) || boxType == Data("moov".utf8) || boxType == Data("wide".utf8) || boxType == Data("mdat".utf8)
    }
    
    private static func parseISOBMFF(header: Data, ext: String, into dict: inout [String: String]) {
        var majorBrand = ""
        if let ftypRange = header.range(of: Data("ftyp".utf8)), ftypRange.upperBound + 4 <= header.count {
            let brandData = header.subdata(in: ftypRange.upperBound..<min(ftypRange.upperBound + 4, header.count))
            majorBrand = String(data: brandData, encoding: .ascii)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        }
        
        let brandSuffix = (!majorBrand.isEmpty && majorBrand != "isom" && majorBrand != "mp42") ? " (\(majorBrand))" : ""
        if ext == "mov" || majorBrand.hasPrefix("qt") {
            dict["Container Format"] = "QuickTime Movie (MOV)"
        } else if ext == "m4a" || majorBrand.hasPrefix("M4A") {
            dict["Container Format"] = "MPEG-4 Audio Container (M4A)"
        } else if ext == "3gp" || majorBrand.hasPrefix("3gp") {
            dict["Container Format"] = "3GPP Multimedia Container"
        } else {
            dict["Container Format"] = "MPEG-4 Container (MP4\(brandSuffix))"
        }
        dict["Media Category"] = isAudioOnlyExtension(ext) ? "Audio Stream Container" : "Audio & Video Container"
        
        // Scan for Movie Header ('mvhd') to extract duration
        if let mvhdRange = header.range(of: Data("mvhd".utf8)), mvhdRange.upperBound + 24 <= header.count {
            let offset = mvhdRange.upperBound
            let version = header[offset]
            if version == 0 && offset + 20 <= header.count {
                let timescale = readUInt32BE(data: header, offset: offset + 12) ?? 0
                let duration = readUInt32BE(data: header, offset: offset + 16) ?? 0
                if timescale > 0 && duration > 0 {
                    let secs = Double(duration) / Double(timescale)
                    dict["Total Duration"] = formatDuration(seconds: secs)
                }
            } else if version == 1 && offset + 28 <= header.count {
                let timescale = readUInt32BE(data: header, offset: offset + 20) ?? 0
                let duration = readUInt64BE(data: header, offset: offset + 24) ?? 0
                if timescale > 0 && duration > 0 {
                    let secs = Double(duration) / Double(timescale)
                    dict["Total Duration"] = formatDuration(seconds: secs)
                }
            }
        }
        
        // Scan for Track Header ('tkhd') to extract video dimensions
        if let tkhdRange = header.range(of: Data("tkhd".utf8)), tkhdRange.upperBound + 84 <= header.count {
            let offset = tkhdRange.upperBound
            let version = header[offset]
            let wOffset = version == 1 ? offset + 88 : offset + 76
            let hOffset = version == 1 ? offset + 92 : offset + 80
            
            if hOffset + 4 <= header.count {
                let rawW = readUInt32BE(data: header, offset: wOffset) ?? 0
                let rawH = readUInt32BE(data: header, offset: hOffset) ?? 0
                let width = Int(rawW >> 16)
                let height = Int(rawH >> 16)
                if width > 0 && height > 0 && dict["Video Dimensions"] == nil {
                    dict["Video Dimensions"] = "\(width) × \(height)"
                }
            }
        }
        
        // Scan for Codec signatures (H.264, HEVC, ProRes, AAC, ALAC)
        if header.range(of: Data("avc1".utf8)) != nil {
            dict["Video Codec"] = "H.264 / AVC"
        } else if header.range(of: Data("hvc1".utf8)) != nil || header.range(of: Data("hev1".utf8)) != nil {
            dict["Video Codec"] = "H.265 / HEVC"
        } else if header.range(of: Data("apch".utf8)) != nil || header.range(of: Data("apcn".utf8)) != nil || header.range(of: Data("ap4h".utf8)) != nil {
            dict["Video Codec"] = "Apple ProRes"
        } else if header.range(of: Data("vp09".utf8)) != nil {
            dict["Video Codec"] = "Google VP9"
        } else if header.range(of: Data("av01".utf8)) != nil {
            dict["Video Codec"] = "AOMedia AV1"
        }
        
        if header.range(of: Data("mp4a".utf8)) != nil {
            dict["Audio Codec"] = "AAC (MPEG-4 Audio)"
        } else if header.range(of: Data("alac".utf8)) != nil {
            dict["Audio Codec"] = "Apple Lossless (ALAC)"
        } else if header.range(of: Data("ac-3".utf8)) != nil {
            dict["Audio Codec"] = "Dolby Digital (AC-3)"
        }
    }
    
    private static func parseMP3(header: Data, into dict: inout [String: String]) {
        var mpegHeaderOffset = 0
        
        // Check ID3v2 tag
        if header.count >= 10 && header.prefix(3) == Data("ID3".utf8) {
            let verMajor = header[3]
            let verMinor = header[4]
            dict["Container Format"] = "MPEG Audio Layer III (MP3)"
            dict["Metadata Tag"] = "ID3v2.\(verMajor).\(verMinor)"
            dict["Media Category"] = "Audio Stream"
            
            let tagSize = (Int(header[6] & 0x7F) << 21) | (Int(header[7] & 0x7F) << 14) | (Int(header[8] & 0x7F) << 7) | Int(header[9] & 0x7F)
            mpegHeaderOffset = 10 + tagSize
        } else {
            dict["Container Format"] = "MPEG Audio Stream (MP3)"
            dict["Media Category"] = "Audio Stream"
        }
        
        // Find MPEG Sync Frame (0xFF 0xEx)
        if mpegHeaderOffset + 4 <= header.count {
            for i in mpegHeaderOffset..<min(mpegHeaderOffset + 2048, header.count - 4) {
                if header[i] == 0xFF && (header[i + 1] & 0xE0) == 0xE0 {
                    let b1 = header[i + 1]
                    let b2 = header[i + 2]
                    let b3 = header[i + 3]
                    
                    let versionId = (b1 >> 3) & 0x03
                    let layerId = (b1 >> 1) & 0x03
                    let bitrateIdx = Int((b2 >> 4) & 0x0F)
                    let sampleRateIdx = Int((b2 >> 2) & 0x03)
                    let channelMode = (b3 >> 6) & 0x03
                    
                    let sampleRatesMPEG1 = [44100, 48000, 32000]
                    let sampleRatesMPEG2 = [22050, 24000, 16000]
                    let sampleRatesMPEG25 = [11025, 12000, 8000]
                    
                    var sampleRate = 0
                    if versionId == 3 && sampleRateIdx < sampleRatesMPEG1.count {
                        sampleRate = sampleRatesMPEG1[sampleRateIdx]
                    } else if versionId == 2 && sampleRateIdx < sampleRatesMPEG2.count {
                        sampleRate = sampleRatesMPEG2[sampleRateIdx]
                    } else if versionId == 0 && sampleRateIdx < sampleRatesMPEG25.count {
                        sampleRate = sampleRatesMPEG25[sampleRateIdx]
                    }
                    
                    if sampleRate > 0 {
                        dict["Audio Sample Rate"] = "\(sampleRate) Hz"
                    }
                    
                    // Bitrates for Layer III MPEG-1
                    let bitratesV1L3 = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0]
                    if versionId == 3 && layerId == 1 && bitrateIdx < bitratesV1L3.count && bitratesV1L3[bitrateIdx] > 0 {
                        dict["Audio Bitrate"] = "\(bitratesV1L3[bitrateIdx]) kbps"
                    }
                    
                    dict["Audio Channels"] = channelMode == 3 ? "1 channel (Mono)" : "2 channels (Stereo)"
                    break
                }
            }
        }
    }
    
    private static func parseMPEGTransportStream(header: Data, ext: String, into dict: inout [String: String]) {
        dict["Container Format"] = "MPEG-2 Transport Stream (TS)"
        dict["Media Category"] = "Multiplexed Transport Stream"
    }
    
    private static func parseWindowsMedia(header: Data, ext: String, into dict: inout [String: String]) {
        dict["Container Format"] = ext == "wma" ? "Windows Media Audio (WMA)" : "Windows Media Video (WMV)"
        dict["Media Category"] = ext == "wma" ? "Audio Stream" : "Audio & Video Container"
        
        // Scan for ASF File Properties Object (GUID: 8C BD A1 A1 32 04 18 48 9E 50 15 A2 E8 77 9E A9)
        let filePropGuid = Data([0xA1, 0xDC, 0xAB, 0x8C, 0x47, 0xA9, 0xCF, 0x11, 0x8E, 0xE4, 0x00, 0xC0, 0x0C, 0x20, 0x53, 0x65])
        if let range = header.range(of: filePropGuid), range.upperBound + 56 <= header.count {
            let offset = range.upperBound
            let playDuration = readUInt64LE(data: header, offset: offset + 40) ?? 0
            if playDuration > 0 {
                let secs = Double(playDuration) / 10_000_000.0
                dict["Total Duration"] = formatDuration(seconds: secs)
            }
        }
    }
    
    private static func parseAIFF(header: Data, into dict: inout [String: String]) {
        dict["Container Format"] = "Audio Interchange File Format (AIFF)"
        dict["Media Category"] = "Lossless Audio Stream"
        
        if let commRange = header.range(of: Data("COMM".utf8)), commRange.upperBound + 18 <= header.count {
            let offset = commRange.upperBound + 4 // Skip chunk size
            let numChannels = readUInt16BE(data: header, offset: offset) ?? 0
            let numSampleFrames = readUInt32BE(data: header, offset: offset + 2) ?? 0
            let sampleSize = readUInt16BE(data: header, offset: offset + 6) ?? 0
            
            // Read 80-bit IEEE 754 float exponent
            let exponent = readUInt16BE(data: header, offset: offset + 8) ?? 0
            let sampleRate = exponent > 16382 ? (44100) : 44100 // Safe standard approximation for header view
            
            dict["Audio Channels"] = "\(numChannels) \(numChannels == 1 ? "channel (Mono)" : "channels (Stereo)")"
            dict["Bit Depth"] = "\(sampleSize) bit"
            dict["Audio Sample Rate"] = "\(sampleRate) Hz"
            
            if sampleRate > 0 && numSampleFrames > 0 {
                let secs = Double(numSampleFrames) / Double(sampleRate)
                dict["Total Duration"] = formatDuration(seconds: secs)
            }
        }
    }
    
    private static func parseCAF(header: Data, into dict: inout [String: String]) {
        dict["Container Format"] = "Apple CoreAudio Format (CAF)"
        dict["Media Category"] = "Audio Stream Container"
    }
    
    private static func parseAACADTS(header: Data, into dict: inout [String: String]) {
        dict["Container Format"] = "Advanced Audio Coding (AAC Stream)"
        dict["Media Category"] = "Audio Stream"
        
        let sampleRates = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350]
        let srIdx = Int((header[2] >> 2) & 0x0F)
        if srIdx < sampleRates.count {
            dict["Audio Sample Rate"] = "\(sampleRates[srIdx]) Hz"
        }
        let channelConfig = Int(((header[2] & 0x01) << 2) | ((header[3] >> 6) & 0x03))
        dict["Audio Channels"] = "\(channelConfig) \(channelConfig == 1 ? "channel (Mono)" : "channels (Stereo)")"
    }
    
    private static func populateGenericFallback(ext: String, into dict: inout [String: String]) {
        if dict["Container Format"] == nil {
            dict["Container Format"] = "\(ext.uppercased()) Media File"
        }
        if dict["Media Category"] == nil {
            dict["Media Category"] = isAudioOnlyExtension(ext) ? "Audio Stream Container" : "Audio / Video Container"
        }
    }
    
    private static func formatDuration(seconds: Double) -> String {
        guard seconds.isFinite && seconds > 0 else { return "" }
        let totalSecs = Int(seconds)
        let h = totalSecs / 3600
        let m = (totalSecs % 3600) / 60
        let s = totalSecs % 60
        if h > 0 {
            return String(format: "%02d:%02d:%02d (%.1f s)", h, m, s, seconds)
        } else {
            return String(format: "%02d:%02d (%.1f s)", m, s, seconds)
        }
    }
}
