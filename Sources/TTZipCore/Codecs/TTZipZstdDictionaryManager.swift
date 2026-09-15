// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation

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

    /// Trains a new Zstandard dictionary from an array of sample data chunks via UniFFI.
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

        return try uniffiZstdTrainDict(
            samples: nonZeroSamples,
            targetDictSize: UInt64(targetDictionarySize),
            level: compressionLevel
        )
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

    /// Compresses small file payload using a specified pre-digested Zstandard dictionary via UniFFI.
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

        return try uniffiZstdDictCompress(src: data, dictBytes: dictionary, level: level)
    }

    /// Decompresses small file payload using a specified pre-digested Zstandard dictionary via UniFFI.
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

        let expSize = expectedUncompressedSize.map { UInt64($0) }
        return try uniffiZstdDictDecompress(src: data, dictBytes: dictionary, expectedUncompressedSize: expSize)
    }

    /// Records metrics for compression acceleration.
    public func recordAcceleration(originalSize: Int, compressedSize: Int) {
        lock.lock()
        defer { lock.unlock() }
        totalAcceleratedBytes += originalSize
        totalSavedBytes += max(0, originalSize - compressedSize)
    }
}
