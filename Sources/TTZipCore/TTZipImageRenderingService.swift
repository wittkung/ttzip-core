// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation
import os

// MARK: - Swift 6 Actor-Isolated Background Worker

/// Actor-isolated background worker executing UniFFI C-ABI image processing pipelines.
public actor TTZipImageWorker {
    private let nativeService: UniFfiImageService

    public init() {
        self.nativeService = UniFfiImageService()
    }

    /// Probes image format and metadata from a filesystem path.
    public func probeMetadata(at path: String) throws -> TTZipImageMetadata {
        let uniffi = try nativeService.probeInfoFromFile(filePath: path)
        return TTZipImageMetadata(from: uniffi, sourceId: path)
    }

    /// Probes image format and metadata from in-memory bytes.
    public func probeMetadata(from data: Data, fileName: String? = nil) throws -> TTZipImageMetadata {
        let uniffi = try nativeService.probeInfo(data: data, fileName: fileName)
        return TTZipImageMetadata(from: uniffi, sourceId: fileName ?? "memory://buffer")
    }

    /// Decodes an image from disk into full-frame RGBA8 pixels.
    public func decodeImage(at path: String, maxDimension: Int? = nil) throws -> TTZipRenderedFrame {
        let maxDim = maxDimension.map { UInt32($0) }
        let uniffi = try nativeService.decodeImageFromFile(filePath: path, maxDimension: maxDim)
        return TTZipRenderedFrame(from: uniffi)
    }

    /// Decodes an image from in-memory bytes into RGBA8 pixels.
    public func decodeImage(from data: Data, maxDimension: Int? = nil) throws -> TTZipRenderedFrame {
        let maxDim = maxDimension.map { UInt32($0) }
        let uniffi = try nativeService.decodeImage(data: data, maxDimension: maxDim)
        return TTZipRenderedFrame(from: uniffi)
    }

    /// Extracts a downsampled thumbnail from a local file on disk.
    public func extractThumbnail(
        at path: String,
        maxWidth: Int,
        maxHeight: Int,
        filter: String? = nil
    ) throws -> TTZipThumbnail {
        let uniffi = try nativeService.extractThumbnailFromFile(
            filePath: path,
            maxWidth: UInt32(maxWidth),
            maxHeight: UInt32(maxHeight),
            filterType: filter
        )
        return TTZipThumbnail(from: uniffi)
    }

    /// Extracts a downsampled thumbnail from in-memory bytes.
    public func extractThumbnail(
        from data: Data,
        maxWidth: Int,
        maxHeight: Int,
        filter: String? = nil
    ) throws -> TTZipThumbnail {
        let uniffi = try nativeService.extractThumbnail(
            data: data,
            maxWidth: UInt32(maxWidth),
            maxHeight: UInt32(maxHeight),
            filterType: filter
        )
        return TTZipThumbnail(from: uniffi)
    }

    /// Samples a cropped viewport tile from a local file.
    public func sampleViewport(at path: String, request: TTZipViewportRequest) throws -> TTZipViewportTile {
        let params = UniFfiViewportCropParams(
            cropX: UInt32(request.cropX),
            cropY: UInt32(request.cropY),
            cropWidth: UInt32(request.cropWidth),
            cropHeight: UInt32(request.cropHeight),
            targetWidth: UInt32(request.targetWidth),
            targetHeight: UInt32(request.targetHeight)
        )
        let uniffi = try nativeService.sampleViewportFromFile(
            filePath: path,
            params: params
        )
        return TTZipViewportTile(from: uniffi)
    }

    /// Samples a cropped viewport tile from in-memory bytes.
    public func sampleViewport(from data: Data, request: TTZipViewportRequest) throws -> TTZipViewportTile {
        let params = UniFfiViewportCropParams(
            cropX: UInt32(request.cropX),
            cropY: UInt32(request.cropY),
            cropWidth: UInt32(request.cropWidth),
            cropHeight: UInt32(request.cropHeight),
            targetWidth: UInt32(request.targetWidth),
            targetHeight: UInt32(request.targetHeight)
        )
        let uniffi = try nativeService.sampleViewport(
            data: data,
            params: params
        )
        return TTZipViewportTile(from: uniffi)
    }
}

// MARK: - Swift 6 Observable Facade Service

/// Swift 6 `@Observable` and `Sendable` image decoding, metadata inspection, and viewport rendering service.
///
/// Provides zero-disk streaming thumbnail generation, fast EXIF inspection, full RGBA8 frame decoding,
/// and high-resolution viewport tile sampling for macOS UI inspector panels, Miller Columns, and QuickLook.
@Observable
public final class TTZipImageRenderingService: @unchecked Sendable {

    // MARK: - Shared Singleton

    public static let shared = TTZipImageRenderingService()

    // MARK: - Published Observable Metrics

    /// Indicates whether one or more image decoding or rendering tasks are actively running.
    public private(set) var isProcessing: Bool = false

    /// Number of concurrent rendering operations currently in flight.
    public private(set) var activeOperationsCount: Int = 0

    /// Cumulative total number of images processed across the lifetime of this service.
    public private(set) var totalImagesProcessed: Int = 0

    /// Most recently rendered full-frame image.
    public private(set) var lastRenderedFrame: TTZipRenderedFrame? = nil

    /// Most recently generated thumbnail.
    public private(set) var lastThumbnail: TTZipThumbnail? = nil

    /// Most recently inspected image metadata record.
    public private(set) var lastInspectedMetadata: TTZipImageMetadata? = nil

    /// Most recent localized error encountered during decoding or rendering.
    public private(set) var latestError: String? = nil

    // MARK: - Internal Storage & Actor Worker

    private let worker = TTZipImageWorker()

    private struct CacheState {
        var metadataCache: [String: TTZipImageMetadata] = [:]
        var thumbnailCache: [String: TTZipThumbnail] = [:]
        var frameCache: [String: TTZipRenderedFrame] = [:]
        var activeCount: Int = 0
        var totalCount: Int = 0
    }

    private let lock = OSAllocatedUnfairLock(initialState: CacheState())

    // MARK: - Initialization

    public init() {}

    // MARK: - High-Level Probing & Metadata APIs

    /// Inspects image metadata and EXIF tags from a local file URL with memory caching.
    ///
    /// - Parameter url: Local filesystem file URL.
    /// - Returns: Strongly-typed `TTZipImageMetadata` descriptor.
    public func probeMetadata(url: URL) async throws -> TTZipImageMetadata {
        let path = url.path
        if let cached = lock.withLock({ $0.metadataCache[path] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let meta = try await worker.probeMetadata(at: path)
            lock.withLock {
                $0.metadataCache[path] = meta
                $0.totalCount += 1
            }
            self.lastInspectedMetadata = meta
            self.totalImagesProcessed += 1
            return meta
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Inspects image metadata and EXIF tags directly from an in-memory byte buffer.
    ///
    /// - Parameters:
    ///   - data: Raw bytes of the image file.
    ///   - fileName: Optional filename hint for extension fallback.
    /// - Returns: Strongly-typed `TTZipImageMetadata` descriptor.
    public func probeMetadata(data: Data, fileName: String? = nil) async throws -> TTZipImageMetadata {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let meta = try await worker.probeMetadata(from: data, fileName: fileName)
            self.lastInspectedMetadata = meta
            self.totalImagesProcessed += 1
            return meta
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    // MARK: - Full Image Decoding APIs

    /// Decodes an image from a local file URL into unified RGBA8 format.
    ///
    /// - Parameters:
    ///   - url: Local filesystem file URL.
    ///   - maxDimension: Optional maximum width or height bound for downsampling.
    /// - Returns: Strongly-typed `TTZipRenderedFrame` container.
    public func decodeImage(url: URL, maxDimension: Int? = nil) async throws -> TTZipRenderedFrame {
        let cacheKey = "\(url.path)@dim=\(maxDimension ?? 0)"
        if let cached = lock.withLock({ $0.frameCache[cacheKey] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let frame = try await worker.decodeImage(at: url.path, maxDimension: maxDimension)
            lock.withLock {
                $0.frameCache[cacheKey] = frame
                $0.totalCount += 1
            }
            self.lastRenderedFrame = frame
            self.totalImagesProcessed += 1
            return frame
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Decodes an image from in-memory bytes into unified RGBA8 format.
    ///
    /// - Parameters:
    ///   - data: Raw compressed image bytes.
    ///   - maxDimension: Optional maximum width or height bound.
    /// - Returns: Strongly-typed `TTZipRenderedFrame` container.
    public func decodeImage(data: Data, maxDimension: Int? = nil) async throws -> TTZipRenderedFrame {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let frame = try await worker.decodeImage(from: data, maxDimension: maxDimension)
            self.lastRenderedFrame = frame
            self.totalImagesProcessed += 1
            return frame
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    // MARK: - Streaming Thumbnail APIs

    /// Extracts a high-quality downsampled thumbnail from a local file with caching.
    ///
    /// - Parameters:
    ///   - url: Local filesystem file URL.
    ///   - maxWidth: Bounding box maximum width in pixels.
    ///   - maxHeight: Bounding box maximum height in pixels.
    ///   - filter: Resampling filter algorithm ("bilinear", "nearest", "lanczos3").
    /// - Returns: Strongly-typed `TTZipThumbnail` container.
    public func extractThumbnail(
        url: URL,
        maxWidth: Int = 256,
        maxHeight: Int = 256,
        filter: String? = nil
    ) async throws -> TTZipThumbnail {
        let cacheKey = "\(url.path)@thumb=\(maxWidth)x\(maxHeight)_\(filter ?? "default")"
        if let cached = lock.withLock({ $0.thumbnailCache[cacheKey] }) {
            return cached
        }

        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let thumb = try await worker.extractThumbnail(
                at: url.path,
                maxWidth: maxWidth,
                maxHeight: maxHeight,
                filter: filter
            )
            lock.withLock {
                $0.thumbnailCache[cacheKey] = thumb
                $0.totalCount += 1
            }
            self.lastThumbnail = thumb
            self.totalImagesProcessed += 1
            return thumb
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Extracts a high-quality downsampled thumbnail directly from in-memory bytes.
    ///
    /// - Parameters:
    ///   - data: Raw compressed image bytes.
    ///   - maxWidth: Maximum width.
    ///   - maxHeight: Maximum height.
    ///   - filter: Resampling filter name.
    /// - Returns: Strongly-typed `TTZipThumbnail` container.
    public func extractThumbnail(
        data: Data,
        maxWidth: Int = 256,
        maxHeight: Int = 256,
        filter: String? = nil
    ) async throws -> TTZipThumbnail {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let thumb = try await worker.extractThumbnail(
                from: data,
                maxWidth: maxWidth,
                maxHeight: maxHeight,
                filter: filter
            )
            self.lastThumbnail = thumb
            self.totalImagesProcessed += 1
            return thumb
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    // MARK: - High-Resolution Viewport Sampling APIs

    /// Samples a cropped sub-region viewport tile from a local file URL.
    ///
    /// - Parameters:
    ///   - url: Local filesystem file URL.
    ///   - request: Viewport crop rectangle and target resolution parameters.
    /// - Returns: Sampled `TTZipViewportTile` container.
    public func sampleViewport(url: URL, request: TTZipViewportRequest) async throws -> TTZipViewportTile {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let tile = try await worker.sampleViewport(at: url.path, request: request)
            self.totalImagesProcessed += 1
            return tile
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    /// Samples a cropped sub-region viewport tile directly from an in-memory byte buffer.
    ///
    /// - Parameters:
    ///   - data: Raw compressed image bytes.
    ///   - request: Viewport crop rectangle and target resolution parameters.
    /// - Returns: Sampled `TTZipViewportTile` container.
    public func sampleViewport(data: Data, request: TTZipViewportRequest) async throws -> TTZipViewportTile {
        updateOperationCount(delta: 1)
        defer { updateOperationCount(delta: -1) }

        do {
            let tile = try await worker.sampleViewport(from: data, request: request)
            self.totalImagesProcessed += 1
            return tile
        } catch {
            self.latestError = error.localizedDescription
            throw error
        }
    }

    // MARK: - Cache Invalidation

    /// Purges all in-memory metadata, thumbnail, and frame caches.
    public func clearCache() {
        lock.withLock {
            $0.metadataCache.removeAll(keepingCapacity: false)
            $0.thumbnailCache.removeAll(keepingCapacity: false)
            $0.frameCache.removeAll(keepingCapacity: false)
        }
    }

    // MARK: - Private Helpers

    private func updateOperationCount(delta: Int) {
        let (active, total) = lock.withLock { state -> (Int, Int) in
            state.activeCount = max(0, state.activeCount + delta)
            return (state.activeCount, state.totalCount)
        }
        self.activeOperationsCount = active
        self.isProcessing = active > 0
        if delta > 0 {
            self.totalImagesProcessed = total
        }
    }
}
