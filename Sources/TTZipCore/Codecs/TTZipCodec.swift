// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Swift 6 strongly-typed facade for all native compression codecs via UniFFI bridge.
public struct TTZipCodec: Sendable {

    /// Calculates the maximum theoretical compressed output buffer size in bytes for a given input size.
    @inlinable
    public static func compressBound(
        uncompressedSize: Int,
        algorithm: TTZipCodecAlgorithm,
        level: TTZipCompressionLevel = .normal
    ) -> Int {
        let rawLvl = level.rawLevel(for: algorithm)
        switch algorithm {
        case .fastLzma2:
            return Int(uniffiFl2CompressBound(srcLen: UInt64(max(0, uncompressedSize))))
        default:
            let codec = algorithm.uniffiCodec(level: rawLvl)
            return Int(uniffiCompressBound(codec: codec, srcLen: UInt64(max(0, uncompressedSize)), level: rawLvl))
        }
    }

    /// Compresses in-memory byte buffer using the specified algorithm and level via UniFFI.
    @inlinable
    public static func compress(
        _ data: Data,
        algorithm: TTZipCodecAlgorithm,
        level: TTZipCompressionLevel = .normal
    ) throws -> Data {
        if data.isEmpty {
            return Data()
        }

        let rawLvl = level.rawLevel(for: algorithm)
        switch algorithm {
        case .fastLzma2:
            return try uniffiFl2Compress(src: data, level: rawLvl, nbThreads: 1)
        default:
            let codec = algorithm.uniffiCodec(level: rawLvl)
            let opts = UniFfiCompressionOptions(
                level: rawLvl,
                acceleration: algorithm == .lz4 ? 1 : nil,
                windowMb: nil,
                ppmdOrder: algorithm == .ppmd ? 6 : nil,
                ppmdMemMb: algorithm == .ppmd ? 16 : nil
            )
            return try uniffiCompressBuffer(codec: codec, src: data, options: opts)
        }
    }

    /// Decompresses an in-memory compressed byte buffer via UniFFI.
    @inlinable
    public static func decompress(
        _ data: Data,
        algorithm: TTZipCodecAlgorithm,
        expectedUncompressedSize: Int? = nil
    ) throws -> Data {
        if data.isEmpty {
            return Data()
        }

        let expSize: UInt64?
        if let expected = expectedUncompressedSize, expected > 0 {
            expSize = UInt64(expected)
        } else {
            switch algorithm {
            case .fastLzma2, .lzma:
                expSize = uniffiFl2FindDecompressedSize(src: data)
            default:
                expSize = nil
            }
        }

        let codec = algorithm.uniffiCodec()
        switch algorithm {
        case .fastLzma2:
            return try uniffiFl2Decompress(src: data, expectedUncompressedSize: expSize, nbThreads: 1)
        case .deflate, .zlib, .gzip, .lz4, .lzfse, .ppmd:
            if let exp = expSize {
                return try uniffiDecompressBuffer(codec: codec, src: data, expectedUncompressedSize: exp, options: nil)
            } else {
                var capacity = UInt64(max(data.count * 4, 65536))
                for _ in 0..<4 {
                    do {
                        return try uniffiDecompressBuffer(codec: codec, src: data, expectedUncompressedSize: capacity, options: nil)
                    } catch {
                        capacity *= 4
                    }
                }
                throw TTZipCodecError.decompressionFailed(status: -1)
            }
        default:
            return try uniffiDecompressBuffer(codec: codec, src: data, expectedUncompressedSize: expSize, options: nil)
        }
    }

    /// Asynchronously streams compressed chunks through an `AsyncThrowingStream` pipeline.
    public static func streamCompress(
        source: AsyncThrowingStream<Data, Error>,
        algorithm: TTZipCodecAlgorithm,
        level: TTZipCompressionLevel = .normal
    ) -> AsyncThrowingStream<Data, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    for try await chunk in source {
                        try Task.checkCancellation()
                        let compressedChunk = try compress(chunk, algorithm: algorithm, level: level)
                        try Task.checkCancellation()
                        continuation.yield(compressedChunk)
                    }
                    try Task.checkCancellation()
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    /// Asynchronously streams decompressed chunks through an `AsyncThrowingStream` pipeline.
    public static func streamDecompress(
        source: AsyncThrowingStream<Data, Error>,
        algorithm: TTZipCodecAlgorithm
    ) -> AsyncThrowingStream<Data, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    for try await chunk in source {
                        try Task.checkCancellation()
                        let decompressedChunk = try decompress(chunk, algorithm: algorithm)
                        try Task.checkCancellation()
                        continuation.yield(decompressedChunk)
                    }
                    try Task.checkCancellation()
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    /// Creates a pull-based, demand-driven asynchronous sequence that compresses chunks on demand.
    public static func demandCompress<Upstream: AsyncSequence & Sendable>(
        source: Upstream,
        algorithm: TTZipCodecAlgorithm,
        level: TTZipCompressionLevel = .normal
    ) -> TTZipDemandCompressSequence<Upstream> where Upstream.Element == Data {
        TTZipDemandCompressSequence(upstream: source, algorithm: algorithm, level: level)
    }

    /// Creates a pull-based, demand-driven asynchronous sequence that decompresses chunks on demand.
    public static func demandDecompress<Upstream: AsyncSequence & Sendable>(
        source: Upstream,
        algorithm: TTZipCodecAlgorithm,
        expectedUncompressedSize: Int? = nil
    ) -> TTZipDemandDecompressSequence<Upstream> where Upstream.Element == Data {
        TTZipDemandDecompressSequence(upstream: source, algorithm: algorithm, expectedUncompressedSize: expectedUncompressedSize)
    }
}

// MARK: - Demand-Driven Pull-Based Async Sequences

/// Pull-based, demand-driven asynchronous sequence that compresses data chunks on demand.
/// Provides zero-buffering backpressure by executing compression only when the consumer calls `next()`.
public struct TTZipDemandCompressSequence<Upstream: AsyncSequence & Sendable>: AsyncSequence, Sendable where Upstream.Element == Data {
    public typealias Element = Data

    public struct AsyncIterator: AsyncIteratorProtocol {
        private var upstreamIterator: Upstream.AsyncIterator
        private let algorithm: TTZipCodecAlgorithm
        private let level: TTZipCompressionLevel

        @usableFromInline
        init(upstreamIterator: Upstream.AsyncIterator, algorithm: TTZipCodecAlgorithm, level: TTZipCompressionLevel) {
            self.upstreamIterator = upstreamIterator
            self.algorithm = algorithm
            self.level = level
        }

        public mutating func next() async throws -> Data? {
            try Task.checkCancellation()
            guard let chunk = try await upstreamIterator.next() else {
                return nil
            }
            try Task.checkCancellation()
            return try TTZipCodec.compress(chunk, algorithm: algorithm, level: level)
        }
    }

    private let upstream: Upstream
    private let algorithm: TTZipCodecAlgorithm
    private let level: TTZipCompressionLevel

    public init(
        upstream: Upstream,
        algorithm: TTZipCodecAlgorithm,
        level: TTZipCompressionLevel = .normal
    ) {
        self.upstream = upstream
        self.algorithm = algorithm
        self.level = level
    }

    public init(
        source: Upstream,
        algorithm: TTZipCodecAlgorithm,
        level: TTZipCompressionLevel = .normal
    ) {
        self.init(upstream: source, algorithm: algorithm, level: level)
    }

    public func makeAsyncIterator() -> AsyncIterator {
        AsyncIterator(
            upstreamIterator: upstream.makeAsyncIterator(),
            algorithm: algorithm,
            level: level
        )
    }
}

/// Pull-based, demand-driven asynchronous sequence that decompresses data chunks on demand.
/// Provides zero-buffering backpressure by executing decompression only when the consumer calls `next()`.
public struct TTZipDemandDecompressSequence<Upstream: AsyncSequence & Sendable>: AsyncSequence, Sendable where Upstream.Element == Data {
    public typealias Element = Data

    public struct AsyncIterator: AsyncIteratorProtocol {
        private var upstreamIterator: Upstream.AsyncIterator
        private let algorithm: TTZipCodecAlgorithm
        private let expectedUncompressedSize: Int?

        @usableFromInline
        init(
            upstreamIterator: Upstream.AsyncIterator,
            algorithm: TTZipCodecAlgorithm,
            expectedUncompressedSize: Int?
        ) {
            self.upstreamIterator = upstreamIterator
            self.algorithm = algorithm
            self.expectedUncompressedSize = expectedUncompressedSize
        }

        public mutating func next() async throws -> Data? {
            try Task.checkCancellation()
            guard let chunk = try await upstreamIterator.next() else {
                return nil
            }
            try Task.checkCancellation()
            return try TTZipCodec.decompress(
                chunk,
                algorithm: algorithm,
                expectedUncompressedSize: expectedUncompressedSize
            )
        }
    }

    private let upstream: Upstream
    private let algorithm: TTZipCodecAlgorithm
    private let expectedUncompressedSize: Int?

    public init(
        upstream: Upstream,
        algorithm: TTZipCodecAlgorithm,
        expectedUncompressedSize: Int? = nil
    ) {
        self.upstream = upstream
        self.algorithm = algorithm
        self.expectedUncompressedSize = expectedUncompressedSize
    }

    public init(
        source: Upstream,
        algorithm: TTZipCodecAlgorithm,
        expectedUncompressedSize: Int? = nil
    ) {
        self.init(upstream: source, algorithm: algorithm, expectedUncompressedSize: expectedUncompressedSize)
    }

    public func makeAsyncIterator() -> AsyncIterator {
        AsyncIterator(
            upstreamIterator: upstream.makeAsyncIterator(),
            algorithm: algorithm,
            expectedUncompressedSize: expectedUncompressedSize
        )
    }
}

public typealias TTZipDemandCompressStream = TTZipDemandCompressSequence<AsyncThrowingStream<Data, Error>>
public typealias TTZipDemandDecompressStream = TTZipDemandDecompressSequence<AsyncThrowingStream<Data, Error>>

