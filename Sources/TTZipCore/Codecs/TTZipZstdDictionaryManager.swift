// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation
import CTTZipBridge

/// Metadata record for a trained Zstandard dictionary.
public struct TTZipZstdDictionaryMeta: Sendable, Hashable, Identifiable {
    public let id: String
    public let name: String
    public let size: Int
    public let sampleCount: Int
    public let trainedDate: Date
    public let dictBytes: Data

    public init(
        id: String = UUID().uuidString,
        name: String,
        sampleCount: Int,
        trainedDate: Date = Date(),
        dictBytes: Data
    ) {
        self.id = id
        self.name = name
        self.size = dictBytes.count
        self.sampleCount = sampleCount
        self.trainedDate = trainedDate
        self.dictBytes = dictBytes
    }
}

/// Swift 6 `@Observable` and `Sendable` Zstandard Dictionary Manager.
/// Provides small-file dictionary training, caching, and acceleration pipelines.
@Observable
public final class TTZipZstdDictionaryManager: @unchecked Sendable {

    public static let shared = TTZipZstdDictionaryManager()

    private let lock = NSLock()
    private var dictionaries: [String: TTZipZstdDictionaryMeta] = [:]

    // MARK: - Published Observable Metrics

    public private(set) var cachedDictionariesCount: Int = 0
    public private(set) var totalAcceleratedBytes: Int = 0
    public private(set) var totalSavedBytes: Int = 0

    public init() {}

    /// Trains a new Zstandard dictionary from an array of sample data chunks.
    public static func trainDictionary(
        samples: [Data],
        targetDictionarySize: Int = 112_640,
        compressionLevel: Int32 = 3
    ) throws -> Data {
        guard !samples.isEmpty else {
            throw TTZipCodecError.invalidParameter
        }

        let nonZeroSamples = samples.filter { !$0.isEmpty }
        guard !nonZeroSamples.isEmpty else {
            throw TTZipCodecError.invalidParameter
        }

        var samplePtrs: [UnsafePointer<UInt8>?] = []
        var sampleLens: [Int] = []

        // Pin sample memory pointers during C-ABI training invocation
        let pinnedData = nonZeroSamples

        var outDict = Data(count: targetDictionarySize)
        var outDictLen: Int = 0

        var status: Int32 = 0

        try outDict.withUnsafeMutableBytes { outBuf in
            guard let outPtr = outBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                throw TTZipCodecError.invalidParameter
            }

            for i in 0..<pinnedData.count {
                pinnedData[i].withUnsafeBytes { sBuf in
                    if let ptr = sBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) {
                        samplePtrs.append(ptr)
                        sampleLens.append(pinnedData[i].count)
                    }
                }
            }

            status = samplePtrs.withUnsafeBufferPointer { ptrsBuf in
                sampleLens.withUnsafeBufferPointer { lensBuf in
                    guard let ptrsPtr = ptrsBuf.baseAddress,
                          let lensPtr = lensBuf.baseAddress else {
                        return -1
                    }
                    return ttzip_rust_zstd_train_dict(
                        ptrsPtr,
                        lensPtr,
                        samplePtrs.count,
                        targetDictionarySize,
                        compressionLevel,
                        outPtr,
                        targetDictionarySize,
                        &outDictLen
                    )
                }
            }
        }

        guard status == 0 && outDictLen > 0 else {
            throw TTZipCodecError.compressionFailed(status: status)
        }

        outDict.count = outDictLen
        return outDict
    }

    /// Registers a pre-trained dictionary into the global manager cache.
    public func registerDictionary(name: String, dictBytes: Data, sampleCount: Int = 0) -> TTZipZstdDictionaryMeta {
        lock.lock()
        defer { lock.unlock() }

        let meta = TTZipZstdDictionaryMeta(
            name: name,
            sampleCount: sampleCount,
            dictBytes: dictBytes
        )
        dictionaries[name] = meta
        cachedDictionariesCount = dictionaries.count
        return meta
    }

    /// Retrieves a registered dictionary by name.
    public func dictionary(named name: String) -> TTZipZstdDictionaryMeta? {
        lock.lock()
        defer { lock.unlock() }
        return dictionaries[name]
    }

    /// Compresses small file payload using a specified pre-digested Zstandard dictionary.
    public static func compressWithDict(
        _ data: Data,
        dictionary: Data,
        level: Int32 = 3
    ) throws -> Data {
        if data.isEmpty {
            return Data()
        }
        guard !dictionary.isEmpty else {
            return try TTZipCodec.compress(data, algorithm: .zstd, level: .custom(level))
        }

        let bound = Int(ttzip_rust_zstd_compress_bound(data.count))
        var destination = Data(count: bound)
        var outLen: Int = 0

        let status: Int32 = try data.withUnsafeBytes { srcBuf in
            guard let srcPtr = srcBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                throw TTZipCodecError.invalidParameter
            }
            return try dictionary.withUnsafeBytes { dictBuf in
                guard let dictPtr = dictBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                    throw TTZipCodecError.invalidParameter
                }
                return try destination.withUnsafeMutableBytes { dstBuf in
                    guard let dstPtr = dstBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                        throw TTZipCodecError.invalidParameter
                    }
                    return ttzip_rust_zstd_dict_compress(
                        srcPtr,
                        data.count,
                        dstPtr,
                        bound,
                        dictPtr,
                        dictionary.count,
                        level,
                        &outLen
                    )
                }
            }
        }

        guard status == 0 else {
            throw TTZipCodecError.compressionFailed(status: status)
        }

        destination.count = outLen
        return destination
    }

    /// Decompresses small file payload using a specified pre-digested Zstandard dictionary.
    public static func decompressWithDict(
        _ data: Data,
        dictionary: Data,
        expectedUncompressedSize: Int? = nil
    ) throws -> Data {
        if data.isEmpty {
            return Data()
        }
        guard !dictionary.isEmpty else {
            return try TTZipCodec.decompress(data, algorithm: .zstd, expectedUncompressedSize: expectedUncompressedSize)
        }

        let capacity = expectedUncompressedSize ?? max(data.count * 8, 65536)
        var destination = Data(count: capacity)
        var outLen: Int = 0

        let status: Int32 = try data.withUnsafeBytes { srcBuf in
            guard let srcPtr = srcBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                throw TTZipCodecError.invalidParameter
            }
            return try dictionary.withUnsafeBytes { dictBuf in
                guard let dictPtr = dictBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                    throw TTZipCodecError.invalidParameter
                }
                return try destination.withUnsafeMutableBytes { dstBuf in
                    guard let dstPtr = dstBuf.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                        throw TTZipCodecError.invalidParameter
                    }
                    return ttzip_rust_zstd_dict_decompress(
                        srcPtr,
                        data.count,
                        dstPtr,
                        capacity,
                        dictPtr,
                        dictionary.count,
                        &outLen
                    )
                }
            }
        }

        guard status == 0 else {
            throw TTZipCodecError.decompressionFailed(status: status)
        }

        destination.count = outLen
        return destination
    }

    /// Records metrics for compression acceleration.
    public func recordAcceleration(originalSize: Int, compressedSize: Int) {
        lock.lock()
        defer { lock.unlock() }
        totalAcceleratedBytes += originalSize
        totalSavedBytes += max(0, originalSize - compressedSize)
    }
}
