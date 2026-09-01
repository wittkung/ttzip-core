// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
#if canImport(CoreGraphics)
import CoreGraphics
#endif
#if canImport(AppKit)
import AppKit
#endif

// MARK: - Enums & Strongly-Typed Domain Models

/// Image format categorization and signature identification.
public enum TTZipImageFormat: String, Sendable, CaseIterable, Identifiable {
    case png = "PNG"
    case jpeg = "JPEG"
    case webp = "WebP"
    case gif = "GIF"
    case bmp = "BMP"
    case tiff = "TIFF"
    case ico = "ICO"
    case psd = "PSD"
    case qoi = "QOI"
    case hdr = "HDR"
    case unknown = "Unknown"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .png:
            return "Portable Network Graphics (PNG)"
        case .jpeg:
            return "JPEG Image"
        case .webp:
            return "WebP Image"
        case .gif:
            return "Graphics Interchange Format (GIF)"
        case .bmp:
            return "Windows Bitmap (BMP)"
        case .tiff:
            return "Tagged Image File Format (TIFF)"
        case .ico:
            return "Windows Icon (ICO)"
        case .psd:
            return "Adobe Photoshop Document (PSD)"
        case .qoi:
            return "Quite OK Image (QOI)"
        case .hdr:
            return "Radiance High Dynamic Range (HDR)"
        case .unknown:
            return "Generic Image"
        }
    }

    /// Primary uniform type identifier (UTI) string for macOS preview.
    public var uniformTypeIdentifier: String {
        switch self {
        case .png:
            return "public.png"
        case .jpeg:
            return "public.jpeg"
        case .webp:
            return "org.webmproject.webp"
        case .gif:
            return "com.compuserve.gif"
        case .bmp:
            return "com.microsoft.bmp"
        case .tiff:
            return "public.tiff"
        case .ico:
            return "com.microsoft.ico"
        case .psd:
            return "com.adobe.photoshop-image"
        case .qoi:
            return "public.image"
        case .hdr:
            return "public.radiance"
        case .unknown:
            return "public.image"
        }
    }

    /// Infers the image format kind from a file extension or format name string.
    public static func from(pathOrExtension: String) -> TTZipImageFormat {
        let ext = (pathOrExtension as NSString).pathExtension.lowercased()
        let clean = ext.isEmpty ? pathOrExtension.lowercased() : ext
        switch clean {
        case "png":
            return .png
        case "jpg", "jpeg", "jpe", "jfif":
            return .jpeg
        case "webp":
            return .webp
        case "gif":
            return .gif
        case "bmp", "dib":
            return .bmp
        case "tif", "tiff":
            return .tiff
        case "ico", "cur":
            return .ico
        case "psd", "psb":
            return .psd
        case "qoi":
            return .qoi
        case "hdr", "pic":
            return .hdr
        default:
            let upper = clean.uppercased()
            for format in TTZipImageFormat.allCases where format.rawValue == upper {
                return format
            }
            return .unknown
        }
    }
}

/// Universal image metadata and EXIF introspection descriptor.
public struct TTZipImageMetadata: Sendable, Equatable, Hashable, Identifiable {
    public var id: String
    public var width: Int
    public var height: Int
    public var format: TTZipImageFormat
    public var formatName: String
    public var colorSpace: String
    public var hasAlpha: Bool
    public var bitDepth: Int
    public var orientation: Int
    public var frameCount: Int
    public var cameraMake: String?
    public var cameraModel: String?
    public var lensModel: String?
    public var isoSpeed: Int?
    public var fNumber: Double?
    public var exposureTimeSecs: Double?
    public var focalLengthMm: Double?
    public var dateTimeOriginal: String?
    public var iccProfileName: String?
    public var byteSize: Int64

    public init(
        id: String = UUID().uuidString,
        width: Int = 0,
        height: Int = 0,
        format: TTZipImageFormat = .unknown,
        formatName: String = "Unknown",
        colorSpace: String = "sRGB",
        hasAlpha: Bool = false,
        bitDepth: Int = 8,
        orientation: Int = 1,
        frameCount: Int = 1,
        cameraMake: String? = nil,
        cameraModel: String? = nil,
        lensModel: String? = nil,
        isoSpeed: Int? = nil,
        fNumber: Double? = nil,
        exposureTimeSecs: Double? = nil,
        focalLengthMm: Double? = nil,
        dateTimeOriginal: String? = nil,
        iccProfileName: String? = nil,
        byteSize: Int64 = 0
    ) {
        self.id = id
        self.width = width
        self.height = height
        self.format = format
        self.formatName = formatName
        self.colorSpace = colorSpace
        self.hasAlpha = hasAlpha
        self.bitDepth = bitDepth
        self.orientation = orientation
        self.frameCount = frameCount
        self.cameraMake = cameraMake
        self.cameraModel = cameraModel
        self.lensModel = lensModel
        self.isoSpeed = isoSpeed
        self.fNumber = fNumber
        self.exposureTimeSecs = exposureTimeSecs
        self.focalLengthMm = focalLengthMm
        self.dateTimeOriginal = dateTimeOriginal
        self.iccProfileName = iccProfileName
        self.byteSize = byteSize
    }

    internal init(from uniffi: UniFfiImageInfo, sourceId: String = UUID().uuidString) {
        self.id = sourceId
        self.width = Int(uniffi.width)
        self.height = Int(uniffi.height)
        self.format = TTZipImageFormat.from(pathOrExtension: uniffi.formatName)
        self.formatName = uniffi.formatName
        self.colorSpace = uniffi.colorSpace
        self.hasAlpha = uniffi.hasAlpha
        self.bitDepth = Int(uniffi.bitDepth)
        self.orientation = Int(uniffi.orientation)
        self.frameCount = Int(uniffi.frameCount)
        self.cameraMake = uniffi.cameraMake
        self.cameraModel = uniffi.cameraModel
        self.lensModel = uniffi.lensModel
        self.isoSpeed = uniffi.isoSpeed.map(Int.init)
        self.fNumber = uniffi.fNumber
        self.exposureTimeSecs = uniffi.exposureTimeSecs
        self.focalLengthMm = uniffi.focalLengthMm
        self.dateTimeOriginal = uniffi.dateTimeOriginal
        self.iccProfileName = uniffi.iccProfileName
        self.byteSize = Int64(uniffi.byteSize)
    }

    /// Calculated total megapixels of the image (e.g. 24.1 MP).
    public var megapixels: Double {
        Double(width * height) / 1_000_000.0
    }

    /// Calculated aspect ratio (`width / height`).
    public var aspectRatio: Double {
        guard height > 0 else { return 1.0 }
        return Double(width) / Double(height)
    }

    /// Formatted dimensions string (e.g. "3840 × 2160").
    public var dimensionsString: String {
        "\(width) × \(height)"
    }

    /// Formatted byte size string (e.g. "4.2 MB").
    public var formattedByteSize: String {
        ByteCountFormatter.string(fromByteCount: byteSize, countStyle: .file)
    }

    /// Formatted shutter speed exposure fraction string (e.g. "1/250s").
    public var formattedExposure: String? {
        guard let exp = exposureTimeSecs, exp > 0 else { return nil }
        if exp >= 1.0 {
            return String(format: "%.1fs", exp)
        }
        let denom = Int((1.0 / exp).rounded())
        return "1/\(denom)s"
    }
}

/// Decoded full-frame RGBA8 pixel buffer with dimensions and row stride.
public struct TTZipRenderedFrame: Sendable, Equatable {
    public var width: Int
    public var height: Int
    public var stride: Int
    public var rgbaBytes: Data
    public var colorSpace: String
    public var durationMs: Int?

    public init(
        width: Int,
        height: Int,
        stride: Int,
        rgbaBytes: Data,
        colorSpace: String = "sRGB",
        durationMs: Int? = nil
    ) {
        self.width = width
        self.height = height
        self.stride = stride
        self.rgbaBytes = rgbaBytes
        self.colorSpace = colorSpace
        self.durationMs = durationMs
    }

    internal init(from uniffi: UniFfiImageFrame) {
        self.width = Int(uniffi.width)
        self.height = Int(uniffi.height)
        self.stride = Int(uniffi.stride)
        self.rgbaBytes = uniffi.rgbaBytes
        self.colorSpace = uniffi.colorSpace
        self.durationMs = uniffi.durationMs.map(Int.init)
    }

    #if canImport(CoreGraphics)
    /// Converts decoded RGBA8 bytes to a hardware-accelerated CoreGraphics image.
    public var cgImage: CGImage? {
        createCGImage(rgbaBytes: rgbaBytes, width: width, height: height, stride: stride)
    }
    #endif

    #if canImport(AppKit)
    /// Converts decoded RGBA8 bytes to an AppKit `NSImage` instance.
    public var nsImage: NSImage? {
        guard let cg = cgImage else { return nil }
        return NSImage(cgImage: cg, size: NSSize(width: width, height: height))
    }
    #endif
}

/// High-performance downsampled thumbnail generation result with execution metrics.
public struct TTZipThumbnail: Sendable, Equatable {
    public var width: Int
    public var height: Int
    public var stride: Int
    public var rgbaBytes: Data
    public var scaleFactor: Double
    public var durationMs: Double

    public init(
        width: Int,
        height: Int,
        stride: Int,
        rgbaBytes: Data,
        scaleFactor: Double = 1.0,
        durationMs: Double = 0.0
    ) {
        self.width = width
        self.height = height
        self.stride = stride
        self.rgbaBytes = rgbaBytes
        self.scaleFactor = scaleFactor
        self.durationMs = durationMs
    }

    internal init(from uniffi: UniFfiThumbnailResult) {
        self.width = Int(uniffi.width)
        self.height = Int(uniffi.height)
        self.stride = Int(uniffi.stride)
        self.rgbaBytes = uniffi.rgbaBytes
        self.scaleFactor = uniffi.scaleFactor
        self.durationMs = uniffi.durationMs
    }

    #if canImport(CoreGraphics)
    /// Converts thumbnail RGBA8 bytes to a CoreGraphics image.
    public var cgImage: CGImage? {
        createCGImage(rgbaBytes: rgbaBytes, width: width, height: height, stride: stride)
    }
    #endif

    #if canImport(AppKit)
    /// Converts thumbnail RGBA8 bytes to an AppKit `NSImage`.
    public var nsImage: NSImage? {
        guard let cg = cgImage else { return nil }
        return NSImage(cgImage: cg, size: NSSize(width: width, height: height))
    }
    #endif
}

/// Viewport tile sub-region crop and target resolution request.
public struct TTZipViewportRequest: Sendable, Equatable, Hashable {
    public var cropX: Int
    public var cropY: Int
    public var cropWidth: Int
    public var cropHeight: Int
    public var targetWidth: Int
    public var targetHeight: Int

    public init(
        cropX: Int,
        cropY: Int,
        cropWidth: Int,
        cropHeight: Int,
        targetWidth: Int = 0,
        targetHeight: Int = 0
    ) {
        self.cropX = cropX
        self.cropY = cropY
        self.cropWidth = cropWidth
        self.cropHeight = cropHeight
        self.targetWidth = targetWidth
        self.targetHeight = targetHeight
    }
}

/// Sampled sub-region tile for high-resolution deep zoom viewports.
public struct TTZipViewportTile: Sendable, Equatable {
    public var tileX: Int
    public var tileY: Int
    public var tileWidth: Int
    public var tileHeight: Int
    public var stride: Int
    public var rgbaBytes: Data
    public var lodLevel: Int

    public init(
        tileX: Int,
        tileY: Int,
        tileWidth: Int,
        tileHeight: Int,
        stride: Int,
        rgbaBytes: Data,
        lodLevel: Int = 0
    ) {
        self.tileX = tileX
        self.tileY = tileY
        self.tileWidth = tileWidth
        self.tileHeight = tileHeight
        self.stride = stride
        self.rgbaBytes = rgbaBytes
        self.lodLevel = lodLevel
    }

    internal init(from uniffi: UniFfiViewportTile) {
        self.tileX = Int(uniffi.tileX)
        self.tileY = Int(uniffi.tileY)
        self.tileWidth = Int(uniffi.tileWidth)
        self.tileHeight = Int(uniffi.tileHeight)
        self.stride = Int(uniffi.stride)
        self.rgbaBytes = uniffi.rgbaBytes
        self.lodLevel = Int(uniffi.lodLevel)
    }

    #if canImport(CoreGraphics)
    /// Converts viewport tile RGBA8 bytes to a CoreGraphics image.
    public var cgImage: CGImage? {
        createCGImage(rgbaBytes: rgbaBytes, width: tileWidth, height: tileHeight, stride: stride)
    }
    #endif

    #if canImport(AppKit)
    /// Converts viewport tile RGBA8 bytes to an AppKit `NSImage`.
    public var nsImage: NSImage? {
        guard let cg = cgImage else { return nil }
        return NSImage(cgImage: cg, size: NSSize(width: tileWidth, height: tileHeight))
    }
    #endif
}

// MARK: - CoreGraphics Image Construction Helper

#if canImport(CoreGraphics)
private func createCGImage(rgbaBytes: Data, width: Int, height: Int, stride: Int) -> CGImage? {
    guard width > 0, height > 0, !rgbaBytes.isEmpty else { return nil }
    guard let provider = CGDataProvider(data: rgbaBytes as CFData) else { return nil }
    guard let colorSpace = CGColorSpace(name: CGColorSpace.sRGB) else { return nil }
    let bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue)
    return CGImage(
        width: width,
        height: height,
        bitsPerComponent: 8,
        bitsPerPixel: 32,
        bytesPerRow: stride > 0 ? stride : width * 4,
        space: colorSpace,
        bitmapInfo: bitmapInfo,
        provider: provider,
        decode: nil,
        shouldInterpolate: true,
        intent: .defaultIntent
    )
}
#endif
