// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mathematical synthetic benchmark corpus generators aligned with zlib-ng and 7-Zip standards.
//!
//! Replaces trivial cyclic repetitions with 8 deterministic mathematical generators modeling:
//! 1. Zipf power-law natural language text (`gen_text_data`)
//! 2. 8-slot short match pool with RLE runs (`gen_short_match_data`)
//! 3. 4-symbol DNA alphabet with extreme hash collisions (`gen_dna_data`)
//! 4. Incompressible XorShift128+ high-entropy white noise (`gen_incompressible_noise`)
//! 5. High-entropy literals with Huffman coded blocks (`gen_literals_data`)
//! 6. Mach-O 64-bit binary with ARM64/x86_64 instructions and DWARF records (`gen_binary_macho_data`)
//! 7. Realistic 24-bit RGB raster with 2D spatial gradients and noise (`gen_realistic_rgb_data`)
//! 8. Striped RGB raster with 3-channel extreme long matches (`gen_striped_rgb_data`)

use serde::{Deserialize, Serialize};

/// Target corpus types for matrix benchmarking.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BenchmarkCorpusType {
    Calgary = 0,
    Silesia = 1,
    Xml = 2,
    Random = 3,
    Binary = 4,
    TextData = 5,
    ShortMatch = 6,
    Dna = 7,
    Noise = 8,
    Literals = 9,
    MachOBinary = 10,
    RealisticRgb = 11,
    StripedRgb = 12,
}

impl BenchmarkCorpusType {
    /// Maps an integer ABI identifier into a strongly typed corpus type.
    pub fn from_i32(val: i32) -> Self {
        match val {
            0 => Self::Calgary,
            1 => Self::Silesia,
            2 => Self::Xml,
            3 => Self::Random,
            4 => Self::Binary,
            5 => Self::TextData,
            6 => Self::ShortMatch,
            7 => Self::Dna,
            8 => Self::Noise,
            9 => Self::Literals,
            10 => Self::MachOBinary,
            11 => Self::RealisticRgb,
            12 => Self::StripedRgb,
            _ => Self::Silesia,
        }
    }

    /// Resolves canonical string ID according to manifest contract.
    pub fn from_str_id(id: &str) -> Option<Self> {
        match id {
            "calgary" => Some(Self::Calgary),
            "silesia" => Some(Self::Silesia),
            "xml" => Some(Self::Xml),
            "random" => Some(Self::Random),
            "binary" => Some(Self::Binary),
            "text" | "text_data" => Some(Self::TextData),
            "short_match" => Some(Self::ShortMatch),
            "dna" => Some(Self::Dna),
            "noise" => Some(Self::Noise),
            "literals" => Some(Self::Literals),
            "mixed" | "macho" | "mach_o" => Some(Self::MachOBinary),
            "realistic_rgb" => Some(Self::RealisticRgb),
            "striped_rgb" => Some(Self::StripedRgb),
            _ => None,
        }
    }

    /// Human-readable descriptive name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Calgary => "Calgary (English Text & Source)",
            Self::Silesia => "Silesia (Multi-Modal Mixed)",
            Self::Xml => "XML (Structured Markup & JSON)",
            Self::Random => "Random (High Entropy Noise)",
            Self::Binary => "Binary (Machine Code & Structs)",
            Self::TextData => "Zipf Text (Natural Language)",
            Self::ShortMatch => "Short Match (8-Slot Pattern Pool)",
            Self::Dna => "DNA (4-Symbol High Collision)",
            Self::Noise => "White Noise (Incompressible XorShift128+)",
            Self::Literals => "Literals (High-Entropy Coded)",
            Self::MachOBinary => "Mach-O Binary (ARM64 & DWARF)",
            Self::RealisticRgb => "Realistic RGB (2D Gradient & Noise)",
            Self::StripedRgb => "Striped RGB (3-Channel Long Match)",
        }
    }

    /// Canonical manifest identifier.
    pub fn corpus_id(&self) -> &'static str {
        match self {
            Self::Calgary | Self::TextData => "text",
            Self::ShortMatch => "short_match",
            Self::Dna => "dna",
            Self::Random | Self::Noise => "random",
            Self::Literals => "literals",
            Self::Binary | Self::MachOBinary => "mixed",
            Self::RealisticRgb => "realistic_rgb",
            Self::StripedRgb => "striped_rgb",
            Self::Silesia => "silesia",
            Self::Xml => "xml",
        }
    }
}

/// Deterministic in-memory synthetic corpus builder.
pub struct BenchmarkCorpusGenerator;

impl BenchmarkCorpusGenerator {
    /// Generates deterministic synthetic corpus buffer matching the requested type and size.
    pub fn generate(corpus_type: BenchmarkCorpusType, size: usize) -> Vec<u8> {
        let size = size.max(16);
        match corpus_type {
            BenchmarkCorpusType::Calgary | BenchmarkCorpusType::TextData => Self::gen_text_data(size),
            BenchmarkCorpusType::Silesia => Self::generate_silesia_synthetic(size),
            BenchmarkCorpusType::Xml => Self::generate_xml_synthetic(size),
            BenchmarkCorpusType::Random | BenchmarkCorpusType::Noise => Self::gen_incompressible_noise(size),
            BenchmarkCorpusType::Binary | BenchmarkCorpusType::MachOBinary => Self::gen_binary_macho_data(size),
            BenchmarkCorpusType::ShortMatch => Self::gen_short_match_data(size),
            BenchmarkCorpusType::Dna => Self::gen_dna_data(size),
            BenchmarkCorpusType::Literals => Self::gen_literals_data(size),
            BenchmarkCorpusType::RealisticRgb => Self::gen_realistic_rgb_data(size),
            BenchmarkCorpusType::StripedRgb => Self::gen_striped_rgb_data(size),
        }
    }

    // MARK: - 1. Zipf Power-Law Natural Text Generator

    /// English-like text: words drawn Zipf-style from a 128-word vocabulary,
    /// with occasional novel words mutated from vocabulary roots.
    pub fn gen_text_data(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }
        const LETTERS: &[u8] = b"etaoinshrdlucmfwypvbgk";
        let mut vocab = [[0u8; 12]; 128];
        let mut vlen = [0u8; 128];
        let mut rng: u32 = 0x7e47da7a;

        for w in 0..128 {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let len = (3 + ((rng >> 16) % 8)) as usize;
            vlen[w] = len as u8;
            for c in 0..len {
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                vocab[w][c] = LETTERS[((rng >> 16) as usize) % LETTERS.len()];
            }
        }

        let mut buf = Vec::with_capacity(size);
        let mut words: u32 = 0;

        while buf.len() < size {
            // AND of two 7-bit draws biases toward low ranks (Zipf-like distribution)
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let w = (((rng >> 16) & 127) & ((rng >> 22) & 127)) as usize;
            let mut word = vocab[w];
            let len = vlen[w] as usize;

            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            if (rng >> 16).is_multiple_of(6) {
                // Novel word: mutate the tail into fresh literals
                let start = if len > 4 { len - 4 } else { 1 };
                for c in start..len {
                    rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                    word[c] = LETTERS[((rng >> 16) as usize) % LETTERS.len()];
                }
            }

            let rem = size - buf.len();
            let c_len = rem.min(len);
            buf.extend_from_slice(&word[..c_len]);
            if buf.len() >= size {
                break;
            }

            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            words = words.wrapping_add(1);
            if words.is_multiple_of(12) {
                let punct = b".\n";
                let p_len = (size - buf.len()).min(punct.len());
                buf.extend_from_slice(&punct[..p_len]);
            } else if (rng >> 16).is_multiple_of(16) {
                let punct = b", ";
                let p_len = (size - buf.len()).min(punct.len());
                buf.extend_from_slice(&punct[..p_len]);
            } else {
                buf.push(b' ');
            }
        }
        buf
    }

    // MARK: - 2. 8-Slot Short Match Pool Generator

    /// Rotating pool of eight 3..8-byte patterns emitted in random order.
    /// Re-emitted patterns produce short back-references at small distances;
    /// pool refreshes and separator bytes leave short literal runs; ~1/16 of iterations emit RLE runs.
    pub fn gen_short_match_data(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }
        let mut buf = Vec::with_capacity(size);
        let mut rng: u32 = 0xc001cafe;
        let mut pool = [[0u8; 8]; 8];
        let mut plens = [0usize; 8];

        for s in 0..8 {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            plens[s] = (3 + ((rng >> 16) % 6)) as usize;
            for j in 0..plens[s] {
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                pool[s][j] = (rng >> 24) as u8;
            }
        }

        while buf.len() < size {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let r = (rng >> 16) & 0xF;
            let slot = ((rng >> 20) & 7) as usize;

            if r == 0 {
                // RLE run: one byte repeated, matched at dist=1
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                let b = (rng >> 24) as u8;
                let run = (6 + ((rng >> 16) % 18)) as usize;
                let rem = (size - buf.len()).min(run);
                buf.resize(buf.len() + rem, b);
            } else if r <= 2 {
                // Refresh a pool slot with a fresh pattern and emit it: literals
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                plens[slot] = (3 + ((rng >> 16) % 6)) as usize;
                for j in 0..plens[slot] {
                    rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                    pool[slot][j] = (rng >> 24) as u8;
                    if buf.len() < size {
                        buf.push(pool[slot][j]);
                    }
                }
            } else if r == 3 {
                // Separator literal
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                buf.push((rng >> 24) as u8);
            } else {
                // Re-emit a pool pattern: a short match, often chaining
                let p_len = plens[slot];
                let rem = (size - buf.len()).min(p_len);
                buf.extend_from_slice(&pool[slot][..rem]);
            }
        }
        buf
    }

    // MARK: - 3. DNA 4-Symbol High Collision Generator

    /// Random bytes over a 4-symbol (DNA base) alphabet. Nearly every 3-byte prefix
    /// collides, stressing longest_match hash chains with deep chain walks.
    pub fn gen_dna_data(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }
        const BASES: &[u8; 4] = b"ACGT";
        let mut buf = Vec::with_capacity(size);
        let mut rng: u32 = 0x01234567;
        for _ in 0..size {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            buf.push(BASES[((rng >> 24) & 3) as usize]);
        }
        buf
    }

    // MARK: - 4. Incompressible White Noise (XorShift128+)

    /// Pure incompressible bytes using XorShift128+ PRNG (~7.999 bits/byte Shannon entropy).
    /// Forces deflate into stored/uncompressed blocks.
    pub fn gen_incompressible_noise(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }
        let mut buf = vec![0u8; size];
        let mut s0: u64 = 0x853c49e65ed8de24;
        let mut s1: u64 = 0x1e3779b97f4a7c15;

        let mut offset = 0;
        while offset + 8 <= size {
            let mut a = s0;
            let b = s1;
            let result = a.wrapping_add(b);
            s0 = b;
            a ^= a << 23;
            s1 = a ^ b ^ (a >> 17) ^ (b >> 26);

            buf[offset..offset + 8].copy_from_slice(&result.to_le_bytes());
            offset += 8;
        }
        if offset < size {
            let a = s0;
            let b = s1;
            let result = a.wrapping_add(b);
            let bytes = result.to_le_bytes();
            let rem = size - offset;
            buf[offset..size].copy_from_slice(&bytes[..rem]);
        }
        buf
    }

    // MARK: - 5. High-Entropy Literals Generator

    /// High-entropy literals from the bitwise AND of two uniform bytes (~6.5 bits per byte).
    /// Deflate emits Huffman-coded blocks (ratio ~1.1) with few matches.
    pub fn gen_literals_data(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }
        let mut buf = Vec::with_capacity(size);
        let mut rng: u32 = 0x600dd1ce;
        for _ in 0..size {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let a = (rng >> 24) as u8;
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let byte = if ((rng >> 8) & 3) != 0 {
                a & ((rng >> 24) as u8)
            } else {
                a
            };
            buf.push(byte);
        }
        buf
    }

    // MARK: - 6. Mach-O Binary & ARM64/x86_64 Machine Code Generator

    /// Interleaves 64-bit Mach-O headers, 4-byte ARM64/x86_64 fixed-width instructions,
    /// 24-byte DWARF/symbol table records with small variations, and zero-padding blocks.
    pub fn gen_binary_macho_data(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }
        let mut buf = Vec::with_capacity(size);

        // 64-bit Mach-O header prefix (MH_MAGIC_64, CPU_TYPE_ARM64, etc.)
        const MACHO_HEADER: &[u8] = &[
            0xCF, 0xFA, 0xED, 0xFE, // MH_MAGIC_64
            0x0C, 0x00, 0x00, 0x01, // CPU_TYPE_ARM64
            0x00, 0x00, 0x00, 0x00, // cpusubtype
            0x02, 0x00, 0x00, 0x00, // MH_EXECUTE
            0x10, 0x00, 0x00, 0x00, // ncmds (16 commands)
            0x00, 0x10, 0x00, 0x00, // sizeofcmds
            0x85, 0x00, 0x20, 0x00, // flags
            0x00, 0x00, 0x00, 0x00, // reserved
        ];
        let h_len = size.min(MACHO_HEADER.len());
        buf.extend_from_slice(&MACHO_HEADER[..h_len]);

        // Real ARM64 4-byte machine instruction words
        const ARM_INSTRUCTIONS: &[u32] = &[
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
            0xD2800000, // mov x0, #0
        ];

        let mut rng: u32 = 0xb1a5b1a5;
        let mut dword_rec = [0u8; 24];
        for b in dword_rec.iter_mut() {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            *b = (rng >> 16) as u8;
        }

        while buf.len() < size {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let kind = (rng >> 16) % 100;

            if kind < 80 {
                // Code: 4-byte machine instructions and occasional local branch loops
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                let instr = ARM_INSTRUCTIONS[((rng >> 16) as usize) % ARM_INSTRUCTIONS.len()];
                let bytes = instr.to_le_bytes();
                let rem = (size - buf.len()).min(4);
                buf.extend_from_slice(&bytes[..rem]);

                if buf.len() >= size {
                    break;
                }

                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                if ((rng >> 16) & 3) == 0 && buf.len() > 32 {
                    let length = (4 + ((rng >> 20) % 16)) as usize;
                    let maxd = buf.len().min(4096);
                    let dist = if maxd > 8 {
                        8 + (((rng >> 8) as usize) % (maxd - 7))
                    } else {
                        buf.len()
                    };
                    let start = buf.len().saturating_sub(dist);
                    let copy_len = (size - buf.len()).min(length);
                    for i in 0..copy_len {
                        let b = buf[start + i];
                        buf.push(b);
                    }
                }
            } else if kind < 95 {
                // DWARF symbol table records (24-byte records with small mutations)
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                let mut mutated = dword_rec;
                let nvary = 2 + ((rng >> 16) & 1) as usize;
                for _ in 0..nvary {
                    rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                    let idx = ((rng >> 16) as usize) % 24;
                    mutated[idx] = (rng >> 8) as u8;
                }
                let rem = (size - buf.len()).min(24);
                buf.extend_from_slice(&mutated[..rem]);
            } else {
                // Zero-padding section (.bss / page alignment)
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                let z_len = (32 + ((rng >> 16) % 128)) as usize;
                let rem = (size - buf.len()).min(z_len);
                buf.resize(buf.len() + rem, 0);
            }
        }
        buf
    }

    // MARK: - 7. Realistic 24-bit RGB Smooth Gradient Generator

    /// 24-bit RGB pixel image array with 2D spatial gradients and per-pixel noise (+/- 15 levels).
    /// Yields short matches at distance=3 (RGB stride) and inter-row matches.
    pub fn gen_realistic_rgb_data(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }
        let mut buf = vec![0u8; size];
        let pixels = size / 3;
        let width = if pixels >= 256 { 256 } else { pixels.max(1) };
        let height = pixels.checked_div(width).unwrap_or(0);

        if height == 0 {
            return buf;
        }

        let mut seed: u32 = 0x12345678;
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 3;
                if idx + 2 >= size {
                    break;
                }
                let base_r = ((x + y) * 179 / (width + height)) as i32;
                let base_g = ((x * 2 + y) * 131 / (width + height)) as i32;
                let base_b = (y * 241 / height.max(1)) as i32;

                // XorShift noise +/- 15 levels
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let noise = ((seed & 0x1F) as i32) - 15;

                buf[idx] = (base_r + noise).clamp(0, 255) as u8;
                buf[idx + 1] = (base_g + (noise >> 1)).clamp(0, 255) as u8;
                buf[idx + 2] = (base_b - noise).clamp(0, 255) as u8;
            }
        }
        buf
    }

    // MARK: - 8. Striped RGB Raster Generator

    /// RGB pixels arranged as 3 solid R/G/B stripes.
    /// Yields long matches at distance=3 within each stripe and large back-references across boundaries.
    pub fn gen_striped_rgb_data(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }
        let mut buf = vec![0u8; size];
        let pixels = size / 3;
        if pixels == 0 {
            return buf;
        }
        let red_stop = pixels / 3;
        let blue_stop = 2 * pixels / 3;

        for i in 0..red_stop {
            let x = i * 3;
            if x + 2 < size {
                buf[x] = 255;
                buf[x + 1] = 0;
                buf[x + 2] = 0;
            }
        }
        for i in red_stop..blue_stop {
            let x = i * 3;
            if x + 2 < size {
                buf[x] = 0;
                buf[x + 1] = 255;
                buf[x + 2] = 0;
            }
        }
        for i in blue_stop..pixels {
            let x = i * 3;
            if x + 2 < size {
                buf[x] = 0;
                buf[x + 1] = 0;
                buf[x + 2] = 255;
            }
        }
        buf
    }

    // MARK: - Legacy / Composite Synthetic Helpers

    fn generate_xml_synthetic(size: usize) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size);
        buf.extend_from_slice(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root version=\"2.0\">\n");
        let mut id = 1000u32;
        let mut rng = 0x5a5a5a5au32;
        while buf.len() < size {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let throughput = 1000.0 + ((rng >> 16) % 1500) as f64 + ((rng & 0xFF) as f64 / 100.0);
            let algos = ["deflate", "zstd", "lz4", "lzfse", "brotli", "bzip2", "snappy"];
            let algo = algos[((rng >> 20) as usize) % algos.len()];
            let record = format!(
                "  <record id=\"{}\" status=\"active\" timestamp=\"2026-08-28T12:{:02}:{:02}Z\">\n    <title>Benchmark Payload Data Unit {}</title>\n    <attributes compression=\"high\" level=\"{}\" algorithm=\"{}\"/>\n    <metric value=\"{:.2}\" unit=\"MB/s\"/>\n  </record>\n",
                id, (id / 60) % 60, id % 60, id, ((rng >> 8) % 9) + 1, algo, throughput
            );
            id += 1;
            let bytes = record.as_bytes();
            let rem = size - buf.len();
            let c_len = rem.min(bytes.len());
            buf.extend_from_slice(&bytes[..c_len]);
        }
        buf
    }

    fn generate_silesia_synthetic(size: usize) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size);
        let mut toggle = 0usize;
        while buf.len() < size {
            let chunk_size = (4096).min(size - buf.len());
            let chunk = match toggle % 4 {
                0 => Self::gen_text_data(chunk_size),
                1 => Self::gen_binary_macho_data(chunk_size),
                2 => Self::gen_short_match_data(chunk_size),
                _ => Self::gen_realistic_rgb_data(chunk_size),
            };
            toggle += 1;
            let rem = size - buf.len();
            let c_len = rem.min(chunk.len());
            buf.extend_from_slice(&chunk[..c_len]);
        }
        buf
    }
}

