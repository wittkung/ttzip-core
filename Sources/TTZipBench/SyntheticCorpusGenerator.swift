// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Mathematical synthetic benchmark corpus generators aligned with zlib-ng and 7-Zip standards.
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
}

/// Deterministic in-memory synthetic corpus builder for TTZip benchmarking.
public enum SyntheticCorpusGenerator {

    public static func generate(type: SyntheticCorpusType, size: Int) -> Data {
        let actualSize = max(16, size)
        switch type {
        case .zipfText: return genZipfText(size: actualSize)
        case .shortMatch: return genShortMatch(size: actualSize)
        case .dna: return genDna(size: actualSize)
        case .whiteNoise: return genIncompressibleNoise(size: actualSize)
        case .literals: return genLiterals(size: actualSize)
        case .machOBinary: return genMachOBinary(size: actualSize)
        case .realisticRgb: return genRealisticRgb(size: actualSize)
        case .stripedRgb: return genStripedRgb(size: actualSize)
        }
    }

    // MARK: - 1. Zipf Power-Law Natural Text Generator

    public static func genZipfText(size: Int) -> Data {
        guard size > 0 else { return Data() }
        let letters = Array("etaoinshrdlucmfwypvbgk".utf8)
        var vocab = [[UInt8]](repeating: [UInt8](repeating: 0, count: 12), count: 128)
        var vlen = [Int](repeating: 0, count: 128)
        var rng: UInt32 = 0x7e47da7a

        for w in 0..<128 {
            rng = rng &* 1103515245 &+ 12345
            let len = Int(3 + ((rng >> 16) % 8))
            vlen[w] = len
            for c in 0..<len {
                rng = rng &* 1103515245 &+ 12345
                vocab[w][c] = letters[Int((rng >> 16)) % letters.count]
            }
        }

        var buf = [UInt8]()
        buf.reserveCapacity(size)
        var words: UInt32 = 0

        while buf.count < size {
            rng = rng &* 1103515245 &+ 12345
            let w = Int(((rng >> 16) & 127) & ((rng >> 22) & 127))
            var word = vocab[w]
            let len = vlen[w]

            rng = rng &* 1103515245 &+ 12345
            if ((rng >> 16) % 6) == 0 {
                let start = len > 4 ? len - 4 : 1
                for c in start..<len {
                    rng = rng &* 1103515245 &+ 12345
                    word[c] = letters[Int((rng >> 16)) % letters.count]
                }
            }

            let rem = size - buf.count
            let cLen = min(rem, len)
            buf.append(contentsOf: word[0..<cLen])
            if buf.count >= size { break }

            rng = rng &* 1103515245 &+ 12345
            words = words &+ 1
            if (words % 12) == 0 {
                let punct = Array(".\n".utf8)
                let pLen = min(size - buf.count, punct.count)
                buf.append(contentsOf: punct[0..<pLen])
            } else if ((rng >> 16) % 16) == 0 {
                let punct = Array(", ".utf8)
                let pLen = min(size - buf.count, punct.count)
                buf.append(contentsOf: punct[0..<pLen])
            } else {
                buf.append(0x20) // space
            }
        }

        return Data(buf)
    }

    // MARK: - 2. 8-Slot Short Match Pool Generator

    public static func genShortMatch(size: Int) -> Data {
        guard size > 0 else { return Data() }
        var buf = [UInt8]()
        buf.reserveCapacity(size)
        var rng: UInt32 = 0xc001cafe
        var pool = [[UInt8]](repeating: [UInt8](repeating: 0, count: 8), count: 8)
        var plens = [Int](repeating: 0, count: 8)

        for s in 0..<8 {
            rng = rng &* 1103515245 &+ 12345
            plens[s] = Int(3 + ((rng >> 16) % 6))
            for j in 0..<plens[s] {
                rng = rng &* 1103515245 &+ 12345
                pool[s][j] = UInt8((rng >> 24) & 0xFF)
            }
        }

        while buf.count < size {
            rng = rng &* 1103515245 &+ 12345
            let r = (rng >> 16) & 0xF
            let slot = Int((rng >> 20) & 7)

            if r == 0 {
                rng = rng &* 1103515245 &+ 12345
                let b = UInt8((rng >> 24) & 0xFF)
                let run = Int(6 + ((rng >> 16) % 18))
                let rem = min(size - buf.count, run)
                buf.append(contentsOf: repeatElement(b, count: rem))
            } else if r <= 2 {
                rng = rng &* 1103515245 &+ 12345
                plens[slot] = Int(3 + ((rng >> 16) % 6))
                for j in 0..<plens[slot] {
                    rng = rng &* 1103515245 &+ 12345
                    pool[slot][j] = UInt8((rng >> 24) & 0xFF)
                    if buf.count < size {
                        buf.append(pool[slot][j])
                    }
                }
            } else if r == 3 {
                rng = rng &* 1103515245 &+ 12345
                buf.append(UInt8((rng >> 24) & 0xFF))
            } else {
                let pLen = plens[slot]
                let rem = min(size - buf.count, pLen)
                buf.append(contentsOf: pool[slot][0..<rem])
            }
        }

        return Data(buf)
    }

    // MARK: - 3. DNA 4-Symbol High Collision Generator

    public static func genDna(size: Int) -> Data {
        guard size > 0 else { return Data() }
        let bases: [UInt8] = [0x41, 0x43, 0x47, 0x54] // 'A', 'C', 'G', 'T'
        var buf = [UInt8]()
        buf.reserveCapacity(size)
        var rng: UInt32 = 0x01234567

        for _ in 0..<size {
            rng = rng &* 1103515245 &+ 12345
            let idx = Int((rng >> 24) & 3)
            buf.append(bases[idx])
        }

        return Data(buf)
    }

    // MARK: - 4. Incompressible White Noise (XorShift128+)

    public static func genIncompressibleNoise(size: Int) -> Data {
        guard size > 0 else { return Data() }
        var buf = [UInt8](repeating: 0, count: size)
        var s0: UInt64 = 0x853c49e65ed8de24
        var s1: UInt64 = 0x1e3779b97f4a7c15

        var offset = 0
        while offset + 8 <= size {
            var a = s0
            let b = s1
            let result = a &+ b
            s0 = b
            a ^= a << 23
            s1 = a ^ b ^ (a >> 17) ^ (b >> 26)

            withUnsafeBytes(of: result.littleEndian) { ptr in
                for i in 0..<8 {
                    buf[offset + i] = ptr[i]
                }
            }
            offset += 8
        }
        if offset < size {
            let a = s0
            let b = s1
            let result = a &+ b
            withUnsafeBytes(of: result.littleEndian) { ptr in
                let rem = size - offset
                for i in 0..<rem {
                    buf[offset + i] = ptr[i]
                }
            }
        }


        return Data(buf)
    }

    // MARK: - 5. High-Entropy Literals Generator

    public static func genLiterals(size: Int) -> Data {
        guard size > 0 else { return Data() }
        var buf = [UInt8]()
        buf.reserveCapacity(size)
        var rng: UInt32 = 0x600dd1ce

        for _ in 0..<size {
            rng = rng &* 1103515245 &+ 12345
            let a = UInt8((rng >> 24) & 0xFF)
            rng = rng &* 1103515245 &+ 12345
            let byte: UInt8
            if ((rng >> 8) & 3) != 0 {
                byte = a & UInt8((rng >> 24) & 0xFF)
            } else {
                byte = a
            }
            buf.append(byte)
        }

        return Data(buf)
    }

    // MARK: - 6. Mach-O Binary & ARM64 Code Generator

    public static func genMachOBinary(size: Int) -> Data {
        guard size > 0 else { return Data() }
        var buf = [UInt8]()
        buf.reserveCapacity(size)

        let machoHeader: [UInt8] = [
            0xCF, 0xFA, 0xED, 0xFE, // MH_MAGIC_64
            0x0C, 0x00, 0x00, 0x01, // CPU_TYPE_ARM64
            0x00, 0x00, 0x00, 0x00, // cpusubtype
            0x02, 0x00, 0x00, 0x00, // MH_EXECUTE
            0x10, 0x00, 0x00, 0x00, // ncmds (16 commands)
            0x00, 0x10, 0x00, 0x00, // sizeofcmds
            0x85, 0x00, 0x20, 0x00, // flags
            0x00, 0x00, 0x00, 0x00  // reserved
        ]
        let hLen = min(size, machoHeader.count)
        buf.append(contentsOf: machoHeader[0..<hLen])

        let armInstructions: [UInt32] = [
            0xD503201F, // nop
            0xD65F03C0, // ret
            0x910003FD, // mov x29, sp
            0xA9BF7BFD, // stp x29, x30, [sp, #-16]!
            0xA8C17BFD, // ldp x29, x30, [sp], #16
            0x58000040, // ldr x0, [pc, #8]
            0x14000004, // b +16
            0x94000008, // bl +32
            0x8B010000, // add x0, x0, x1
            0xCB010000, // sub x0, x0, x1
            0xAA0103E0, // mov x0, x1
            0xD2800000  // mov x0, #0
        ]

        var rng: UInt32 = 0xb1a5b1a5
        var dwordRec = [UInt8](repeating: 0, count: 24)
        for i in 0..<24 {
            rng = rng &* 1103515245 &+ 12345
            dwordRec[i] = UInt8((rng >> 16) & 0xFF)
        }

        while buf.count < size {
            rng = rng &* 1103515245 &+ 12345
            let kind = (rng >> 16) % 100

            if kind < 80 {
                rng = rng &* 1103515245 &+ 12345
                let instr = armInstructions[Int((rng >> 16)) % armInstructions.count]
                withUnsafeBytes(of: instr.littleEndian) { ptr in
                    let rem = min(size - buf.count, 4)
                    for i in 0..<rem {
                        buf.append(ptr[i])
                    }
                }
                if buf.count >= size { break }

                rng = rng &* 1103515245 &+ 12345
                if ((rng >> 16) & 3) == 0 && buf.count > 32 {
                    let length = Int(4 + ((rng >> 20) % 16))
                    let maxd = min(buf.count, 4096)
                    let dist = maxd > 8 ? 8 + Int(((rng >> 8)) % UInt32(maxd - 7)) : buf.count
                    let start = max(0, buf.count - dist)
                    let copyLen = min(size - buf.count, length)
                    for i in 0..<copyLen {
                        buf.append(buf[start + i])
                    }
                }
            } else if kind < 95 {
                rng = rng &* 1103515245 &+ 12345
                var mutated = dwordRec
                let nvary = Int(2 + ((rng >> 16) & 1))
                for _ in 0..<nvary {
                    rng = rng &* 1103515245 &+ 12345
                    let idx = Int((rng >> 16) % 24)
                    mutated[idx] = UInt8((rng >> 8) & 0xFF)
                }
                let rem = min(size - buf.count, 24)
                buf.append(contentsOf: mutated[0..<rem])
            } else {
                rng = rng &* 1103515245 &+ 12345
                let zLen = Int(32 + ((rng >> 16) % 128))
                let rem = min(size - buf.count, zLen)
                buf.append(contentsOf: repeatElement(0, count: rem))
            }
        }

        return Data(buf)
    }

    // MARK: - 7. Realistic 24-bit RGB Gradient Generator

    public static func genRealisticRgb(size: Int) -> Data {
        guard size > 0 else { return Data() }
        var buf = [UInt8](repeating: 0, count: size)
        let pixels = size / 3
        let width = pixels >= 256 ? 256 : max(1, pixels)
        let height = width > 0 ? pixels / width : 0
        guard height > 0 else { return Data(buf) }

        var seed: UInt32 = 0x12345678
        for y in 0..<height {
            for x in 0..<width {
                let idx = (y * width + x) * 3
                if idx + 2 >= size { break }
                let baseR = Int((x + y) * 179 / (width + height))
                let baseG = Int((x * 2 + y) * 131 / (width + height))
                let baseB = Int(y * 241 / max(1, height))

                seed ^= seed << 13
                seed ^= seed >> 17
                seed ^= seed << 5
                let noise = Int(seed & 0x1F) - 15

                buf[idx] = UInt8(clamping: max(0, min(255, baseR + noise)))
                buf[idx + 1] = UInt8(clamping: max(0, min(255, baseG + (noise >> 1))))
                buf[idx + 2] = UInt8(clamping: max(0, min(255, baseB - noise)))
            }
        }

        return Data(buf)
    }

    // MARK: - 8. Striped RGB Raster Generator

    public static func genStripedRgb(size: Int) -> Data {
        guard size > 0 else { return Data() }
        var buf = [UInt8](repeating: 0, count: size)
        let pixels = size / 3
        guard pixels > 0 else { return Data(buf) }
        let redStop = pixels / 3
        let blueStop = 2 * pixels / 3

        for i in 0..<redStop {
            let x = i * 3
            if x + 2 < size {
                buf[x] = 255
                buf[x + 1] = 0
                buf[x + 2] = 0
            }
        }
        for i in redStop..<blueStop {
            let x = i * 3
            if x + 2 < size {
                buf[x] = 0
                buf[x + 1] = 255
                buf[x + 2] = 0
            }
        }
        for i in blueStop..<pixels {
            let x = i * 3
            if x + 2 < size {
                buf[x] = 0
                buf[x + 1] = 0
                buf[x + 2] = 255
            }
        }

        return Data(buf)
    }
}
