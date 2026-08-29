// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import TTZipCore

/// Mathematical and standard synthetic benchmark corpus identifiers aligned with Rust microkernel single source of truth.
public enum SyntheticCorpusType: String, CaseIterable, Sendable {
    case zipfText = "zipf_text"
    case shortMatch = "short_match"
    case dna = "dna"
    case whiteNoise = "white_noise"
    case literals = "literals"
    case machOBinary = "macho_binary"
    case realisticRgb = "realistic_rgb"
    case stripedRgb = "striped_rgb"

    public var displayName: String {
        switch self {
        case .zipfText: return "Zipf Text (Natural Language & Power-Law)"
        case .shortMatch: return "Short Match (8-Slot Rotating Pattern Pool)"
        case .dna: return "DNA (4-Symbol Extreme Hash Collisions)"
        case .whiteNoise: return "White Noise (Incompressible XorShift128+)"
        case .literals: return "Literals (High-Entropy Huffman Coded)"
        case .machOBinary: return "Mach-O Binary (ARM64 Code & DWARF Records)"
        case .realisticRgb: return "Realistic RGB (2D Gradient & Noise Raster)"
        case .stripedRgb: return "Striped RGB (3-Channel Extreme Long Matches)"
        }
    }

    /// Canonical URI identifier matching the Rust CorpusRegistry.
    public var canonicalId: String {
        switch self {
        case .zipfText: return "synthetic:zipf_text"
        case .shortMatch: return "synthetic:short_match"
        case .dna: return "synthetic:dna"
        case .whiteNoise: return "synthetic:noise"
        case .literals: return "synthetic:literals"
        case .machOBinary: return "synthetic:macho"
        case .realisticRgb: return "synthetic:realistic_rgb"
        case .stripedRgb: return "synthetic:striped_rgb"
        }
    }
}

/// Unified synthetic and real-world benchmark corpus provider wrapping Rust UniFFI microkernel.
public enum SyntheticCorpusGenerator {

    /// Generates corpus byte buffer by typed synthetic corpus enum and requested size.
    public static func generate(type: SyntheticCorpusType, size: Int) -> Data {
        generate(corpusId: type.canonicalId, size: size)
    }

    /// Generates corpus byte buffer by arbitrary corpus ID/URI via Rust microkernel single source of truth.
    public static func generate(corpusId: String, size: Int) -> Data {
        let actualSize = UInt64(max(16, size))
        if let data = try? ttzipBenchGenerateCorpus(corpusId: corpusId, sizeBytes: actualSize) {
            return data
        }
        return Data()
    }

    /// Lists all available standard and registered benchmark corpus identifiers from the Rust microkernel.
    public static func listAvailableCorpusIds() -> [String] {
        ttzipBenchListCorpusIds()
    }
}
