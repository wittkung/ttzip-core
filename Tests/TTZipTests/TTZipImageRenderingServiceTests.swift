// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class TTZipImageRenderingServiceTests: XCTestCase {

    private var sandbox: IsolatedTempSandbox!
    private let service = TTZipImageRenderingService.shared

    override func setUp() async throws {
        try await super.setUp()
        sandbox = try IsolatedTempSandbox(prefix: "ImageRenderTest")
        service.clearCache()
    }

    override func tearDown() async throws {
        service.clearCache()
        sandbox?.cleanup()
        sandbox = nil
        try await super.tearDown()
    }

    // MARK: - 1. Format Kind & Extension Inference Tests

    func testImageFormatInference() {
        XCTAssertEqual(TTZipImageFormat.from(pathOrExtension: "photo.png"), .png)
        XCTAssertEqual(TTZipImageFormat.from(pathOrExtension: "scenery.jpg"), .jpeg)
        XCTAssertEqual(TTZipImageFormat.from(pathOrExtension: "graphic.jpeg"), .jpeg)
        XCTAssertEqual(TTZipImageFormat.from(pathOrExtension: "asset.webp"), .webp)
        XCTAssertEqual(TTZipImageFormat.from(pathOrExtension: "animation.gif"), .gif)
        XCTAssertEqual(TTZipImageFormat.from(pathOrExtension: "icon.bmp"), .bmp)
        XCTAssertEqual(TTZipImageFormat.from(pathOrExtension: "scan.tiff"), .tiff)
        XCTAssertEqual(TTZipImageFormat.from(pathOrExtension: "app.ico"), .ico)
        XCTAssertEqual(TTZipImageFormat.from(pathOrExtension: "design.psd"), .psd)
        XCTAssertEqual(TTZipImageFormat.from(pathOrExtension: "texture.qoi"), .qoi)
        XCTAssertEqual(TTZipImageFormat.from(pathOrExtension: "sky.hdr"), .hdr)
        XCTAssertEqual(TTZipImageFormat.from(pathOrExtension: "unknown.xyz"), .unknown)

        XCTAssertEqual(TTZipImageFormat.png.displayName, "Portable Network Graphics (PNG)")
        XCTAssertEqual(TTZipImageFormat.jpeg.displayName, "JPEG Image")
        XCTAssertEqual(TTZipImageFormat.webp.displayName, "WebP Image")
        XCTAssertEqual(TTZipImageFormat.bmp.displayName, "Windows Bitmap (BMP)")

        XCTAssertEqual(TTZipImageFormat.png.uniformTypeIdentifier, "public.png")
        XCTAssertEqual(TTZipImageFormat.jpeg.uniformTypeIdentifier, "public.jpeg")
        XCTAssertEqual(TTZipImageFormat.webp.uniformTypeIdentifier, "org.webmproject.webp")
        XCTAssertEqual(TTZipImageFormat.bmp.uniformTypeIdentifier, "com.microsoft.bmp")
    }

    // MARK: - 2. Image Metadata Probing Tests

    func testBmpImageMetadataProbing() async throws {
        let bmpData = createTestBMP(width: 32, height: 16)
        let bmpURL = sandbox.fileURL(named: "test_image.bmp")
        try bmpData.write(to: bmpURL)

        // 1. In-memory data probe
        let memMeta = try await service.probeMetadata(data: bmpData, fileName: "test_image.bmp")
        XCTAssertEqual(memMeta.width, 32)
        XCTAssertEqual(memMeta.height, 16)
        XCTAssertEqual(memMeta.format, .bmp)
        XCTAssertEqual(memMeta.formatName, "BMP")
        XCTAssertEqual(memMeta.colorSpace, "sRGB")
        XCTAssertEqual(memMeta.orientation, 1)
        XCTAssertEqual(memMeta.frameCount, 1)
        XCTAssertEqual(memMeta.aspectRatio, 2.0, accuracy: 0.001)
        XCTAssertEqual(memMeta.dimensionsString, "32 × 16")
        XCTAssertGreaterThan(memMeta.byteSize, 0)
        XCTAssertFalse(memMeta.formattedByteSize.isEmpty)

        // 2. File URL probe with caching
        let fileMeta = try await service.probeMetadata(url: bmpURL)
        XCTAssertEqual(fileMeta.width, 32)
        XCTAssertEqual(fileMeta.height, 16)
        XCTAssertEqual(fileMeta.format, .bmp)

        // Verify cached retrieval
        let cachedMeta = try await service.probeMetadata(url: bmpURL)
        XCTAssertEqual(cachedMeta.width, 32)
        XCTAssertEqual(cachedMeta.height, 16)
    }

    // MARK: - 3. Full Frame Decoding Tests

    func testBmpImageDecodingAndRendering() async throws {
        let bmpData = createTestBMP(width: 16, height: 16)
        let bmpURL = sandbox.fileURL(named: "decode_test.bmp")
        try bmpData.write(to: bmpURL)

        // 1. In-memory decoding
        let memFrame = try await service.decodeImage(data: bmpData)
        XCTAssertEqual(memFrame.width, 16)
        XCTAssertEqual(memFrame.height, 16)
        XCTAssertEqual(memFrame.stride, 16 * 4)
        XCTAssertEqual(memFrame.rgbaBytes.count, 16 * 16 * 4)
        XCTAssertEqual(memFrame.colorSpace, "sRGB")

        #if canImport(CoreGraphics)
        let cgImage = memFrame.cgImage
        XCTAssertNotNil(cgImage)
        XCTAssertEqual(cgImage?.width, 16)
        XCTAssertEqual(cgImage?.height, 16)
        #endif

        #if canImport(AppKit)
        let nsImage = memFrame.nsImage
        XCTAssertNotNil(nsImage)
        XCTAssertEqual(nsImage?.size.width, 16)
        XCTAssertEqual(nsImage?.size.height, 16)
        #endif

        // 2. File URL decoding
        let fileFrame = try await service.decodeImage(url: bmpURL)
        XCTAssertEqual(fileFrame.width, 16)
        XCTAssertEqual(fileFrame.height, 16)
    }

    // MARK: - 4. Downsampled Decoding Tests

    func testBmpImageDownsampledDecoding() async throws {
        let bmpData = createTestBMP(width: 32, height: 32)
        let frame = try await service.decodeImage(data: bmpData, maxDimension: 8)
        XCTAssertEqual(frame.width, 8)
        XCTAssertEqual(frame.height, 8)
        XCTAssertEqual(frame.stride, 8 * 4)
        XCTAssertEqual(frame.rgbaBytes.count, 8 * 8 * 4)
    }

    // MARK: - 5. High-Performance Thumbnail Extraction Tests

    func testThumbnailExtraction() async throws {
        let bmpData = createTestBMP(width: 64, height: 32)
        let bmpURL = sandbox.fileURL(named: "thumb_test.bmp")
        try bmpData.write(to: bmpURL)

        // 1. In-memory thumbnail extraction
        let memThumb = try await service.extractThumbnail(data: bmpData, maxWidth: 16, maxHeight: 16)
        XCTAssertEqual(memThumb.width, 16)
        XCTAssertEqual(memThumb.height, 8) // Aspect ratio 2:1 preserved
        XCTAssertEqual(memThumb.stride, 16 * 4)
        XCTAssertEqual(memThumb.rgbaBytes.count, 16 * 8 * 4)
        XCTAssertEqual(memThumb.scaleFactor, 0.25, accuracy: 0.01)
        XCTAssertGreaterThanOrEqual(memThumb.durationMs, 0.0)

        #if canImport(CoreGraphics)
        XCTAssertNotNil(memThumb.cgImage)
        #endif
        #if canImport(AppKit)
        XCTAssertNotNil(memThumb.nsImage)
        #endif

        // 2. File URL thumbnail extraction with caching
        let fileThumb = try await service.extractThumbnail(url: bmpURL, maxWidth: 8, maxHeight: 8)
        XCTAssertEqual(fileThumb.width, 8)
        XCTAssertEqual(fileThumb.height, 4)

        let cachedThumb = try await service.extractThumbnail(url: bmpURL, maxWidth: 8, maxHeight: 8)
        XCTAssertEqual(cachedThumb.width, 8)
    }

    // MARK: - 6. Viewport Tile Sampling Tests

    func testViewportTileSampling() async throws {
        let bmpData = createTestBMP(width: 64, height: 64)
        let bmpURL = sandbox.fileURL(named: "viewport_test.bmp")
        try bmpData.write(to: bmpURL)

        // 1. 1:1 Sub-region crop
        let request1 = TTZipViewportRequest(
            cropX: 10,
            cropY: 10,
            cropWidth: 20,
            cropHeight: 20,
            targetWidth: 20,
            targetHeight: 20
        )
        let tile1 = try await service.sampleViewport(data: bmpData, request: request1)
        XCTAssertEqual(tile1.tileX, 10)
        XCTAssertEqual(tile1.tileY, 10)
        XCTAssertEqual(tile1.tileWidth, 20)
        XCTAssertEqual(tile1.tileHeight, 20)
        XCTAssertEqual(tile1.stride, 20 * 4)
        XCTAssertEqual(tile1.rgbaBytes.count, 20 * 20 * 4)
        XCTAssertEqual(tile1.lodLevel, 0)

        #if canImport(CoreGraphics)
        XCTAssertNotNil(tile1.cgImage)
        #endif

        // 2. Cropped and downsampled tile for deep zoom
        let request2 = TTZipViewportRequest(
            cropX: 0,
            cropY: 0,
            cropWidth: 32,
            cropHeight: 32,
            targetWidth: 8,
            targetHeight: 8
        )
        let tile2 = try await service.sampleViewport(url: bmpURL, request: request2)
        XCTAssertEqual(tile2.tileX, 0)
        XCTAssertEqual(tile2.tileY, 0)
        XCTAssertEqual(tile2.tileWidth, 8)
        XCTAssertEqual(tile2.tileHeight, 8)
        XCTAssertEqual(tile2.stride, 8 * 4)
        XCTAssertEqual(tile2.rgbaBytes.count, 8 * 8 * 4)
        XCTAssertGreaterThanOrEqual(tile2.lodLevel, 2)
    }

    // MARK: - 7. Concurrency & Parallel Throughput Tests

    func testConcurrentImageProcessing() async throws {
        let bmpData = createTestBMP(width: 32, height: 32)
        let localService = self.service

        try await withThrowingTaskGroup(of: Void.self) { group in
            for i in 0..<10 {
                group.addTask {
                    let frame = try await localService.decodeImage(data: bmpData, maxDimension: 16)
                    XCTAssertEqual(frame.width, 16)
                    XCTAssertEqual(frame.height, 16)

                    let thumb = try await localService.extractThumbnail(data: bmpData, maxWidth: 8, maxHeight: 8)
                    XCTAssertEqual(thumb.width, 8)
                    XCTAssertEqual(thumb.height, 8)

                    let req = TTZipViewportRequest(
                        cropX: i * 2,
                        cropY: i * 2,
                        cropWidth: 8,
                        cropHeight: 8,
                        targetWidth: 8,
                        targetHeight: 8
                    )
                    let tile = try await localService.sampleViewport(data: bmpData, request: req)
                    XCTAssertEqual(tile.tileWidth, 8)
                    XCTAssertEqual(tile.tileHeight, 8)
                }
            }
            try await group.waitForAll()
        }

        XCTAssertGreaterThan(service.totalImagesProcessed, 0)
        XCTAssertFalse(service.isProcessing)
        XCTAssertEqual(service.activeOperationsCount, 0)
    }

    // MARK: - 8. Error Handling & Edge Cases

    func testEmptyDataError() async {
        do {
            _ = try await service.decodeImage(data: Data())
            XCTFail("Expected empty data error")
        } catch {
            XCTAssertNotNil(service.latestError)
        }
    }

    func testNonExistentFileError() async {
        let nonExistentURL = sandbox.fileURL(named: "does_not_exist.png")
        do {
            _ = try await service.probeMetadata(url: nonExistentURL)
            XCTFail("Expected file not found error")
        } catch {
            XCTAssertNotNil(service.latestError)
        }
    }

    // MARK: - Private Synthetic BMP Generator Helper

    private func createTestBMP(width: Int, height: Int) -> Data {
        let rowBytes = (width * 3 + 3) & ~3
        let imgSize = rowBytes * height
        let fileSize = 54 + imgSize

        var data = Data(capacity: fileSize)
        data.append(contentsOf: [0x42, 0x4D]) // "BM"
        var fSize = UInt32(fileSize)
        data.append(Data(bytes: &fSize, count: 4))
        var reserved: UInt32 = 0
        data.append(Data(bytes: &reserved, count: 4))
        var offset: UInt32 = 54
        data.append(Data(bytes: &offset, count: 4))
        var headerSize: UInt32 = 40
        data.append(Data(bytes: &headerSize, count: 4))
        var w = Int32(width)
        data.append(Data(bytes: &w, count: 4))
        var h = Int32(height)
        data.append(Data(bytes: &h, count: 4))
        var planes: UInt16 = 1
        data.append(Data(bytes: &planes, count: 2))
        var bitCount: UInt16 = 24
        data.append(Data(bytes: &bitCount, count: 2))
        var compression: UInt32 = 0
        data.append(Data(bytes: &compression, count: 4))
        var rawSize = UInt32(imgSize)
        data.append(Data(bytes: &rawSize, count: 4))
        var xPpm: UInt32 = 2835
        data.append(Data(bytes: &xPpm, count: 4))
        var yPpm: UInt32 = 2835
        data.append(Data(bytes: &yPpm, count: 4))
        var clrUsed: UInt32 = 0
        data.append(Data(bytes: &clrUsed, count: 4))
        var clrImportant: UInt32 = 0
        data.append(Data(bytes: &clrImportant, count: 4))

        let padding = rowBytes - (width * 3)
        for y in 0..<height {
            for x in 0..<width {
                let r = UInt8((x * 255) / max(width - 1, 1))
                let g = UInt8((y * 255) / max(height - 1, 1))
                let b: UInt8 = 128
                data.append(contentsOf: [b, g, r])
            }
            for _ in 0..<padding {
                data.append(0)
            }
        }
        return data
    }
}
