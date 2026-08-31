// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Industrial-grade mathematical synthetic corpus generators for Deflate evaluation.
//!
//! Provides 8 standardized synthetic benchmark corpora modeling real-world data profiles:
//! 1. Silesia text & multi-modal distribution (`generate_silesia_like`)
//! 2. Enwik8 XML & structured encyclopedia distribution (`generate_enwik8_like`)
//! 3. High-entropy incompressible noise (`generate_high_entropy`)
//! 4. Low-entropy periodic wave & cyclic patterns (`generate_low_entropy_periodic`)
//! 5. Zero-sparse data with configurable sparsity ratio (`generate_zero_sparse`)
//! 6. Repetitive LZ77 match clusters with variable offsets (`generate_lz77_clusters`)
//! 7. Source code & structured AST grammar data (`generate_code_structure`)
//! 8. Multimodal interleaved compound container data (`generate_multimodal_interleaved`)

/// Classification of synthetic corpus generation profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntheticCorpusKind {
    /// Multi-modal Silesia distribution (text, code, tabular, binary).
    SilesiaLike,
    /// Wikipedia XML structured markup & multi-lingual text.
    Enwik8Like,
    /// High-entropy white noise (incompressible).
    HighEntropy,
    /// Low-entropy periodic cyclic patterns.
    LowEntropyPeriodic,
    /// Zero-sparse data with high null density.
    ZeroSparse,
    /// Dense LZ77 match clusters and repeating chains.
    Lz77Clusters,
    /// Source code and AST structured tokens.
    CodeStructure,
    /// Multimodal interleaved container stream.
    MultimodalInterleaved,
}

/// Fast, deterministic 64-bit pseudo-random number generator (SplitMix64 / XorShift64).
struct FastPrng {
    state: u64,
}

impl FastPrng {
    #[inline(always)]
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c49e6748fea9b } else { seed },
        }
    }

    #[inline(always)]
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    #[inline(always)]
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    #[inline(always)]
    fn next_u8(&mut self) -> u8 {
        (self.next_u64() & 0xff) as u8
    }

    #[inline(always)]
    fn next_range(&mut self, min_val: usize, max_val: usize) -> usize {
        if min_val >= max_val {
            return min_val;
        }
        let range = (max_val - min_val + 1) as u64;
        min_val + (self.next_u64() % range) as usize
    }

    #[inline(always)]
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() as f64) / (u64::MAX as f64)
    }
}

/// Industrial synthetic corpus factory providing 8 mathematical data distributions.
pub struct SyntheticCorpus;

impl SyntheticCorpus {
    /// Generates corpus data for a given kind and requested size.
    pub fn generate(kind: SyntheticCorpusKind, size: usize) -> Vec<u8> {
        match kind {
            SyntheticCorpusKind::SilesiaLike => Self::generate_silesia_like(size),
            SyntheticCorpusKind::Enwik8Like => Self::generate_enwik8_like(size),
            SyntheticCorpusKind::HighEntropy => Self::generate_high_entropy(size),
            SyntheticCorpusKind::LowEntropyPeriodic => Self::generate_low_entropy_periodic(size, 64),
            SyntheticCorpusKind::ZeroSparse => Self::generate_zero_sparse(size, 0.95),
            SyntheticCorpusKind::Lz77Clusters => Self::generate_lz77_clusters(size, 32, 16),
            SyntheticCorpusKind::CodeStructure => Self::generate_code_structure(size),
            SyntheticCorpusKind::MultimodalInterleaved => Self::generate_multimodal_interleaved(size),
        }
    }

    /// (1) Silesia-like distribution: Multi-modal mixture of English prose, code, and tables.
    pub fn generate_silesia_like(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }

        let mut prng = FastPrng::new(0x511E51A);
        let mut out = Vec::with_capacity(size);

        let words = [
            "the", "of", "and", "a", "to", "in", "is", "you", "that", "it",
            "he", "was", "for", "on", "are", "as", "with", "his", "they", "I",
            "at", "be", "this", "have", "from", "or", "one", "had", "by", "word",
            "but", "not", "what", "all", "were", "we", "when", "your", "can", "said",
            "there", "use", "an", "each", "which", "she", "do", "how", "their", "if",
            "struct", "impl", "fn", "let", "mut", "return", "pub", "match", "async", "await",
        ];

        while out.len() < size {
            let mode = prng.next_range(0, 100);
            if mode < 60 {
                // Natural English / Zipf word stream
                let word_idx = prng.next_range(0, words.len() - 1);
                let w = words[word_idx].as_bytes();
                let remaining = size - out.len();
                let to_copy = w.len().min(remaining);
                out.extend_from_slice(&w[..to_copy]);
                if out.len() < size {
                    out.push(b' ');
                }
            } else if mode < 85 {
                // Numeric / hex tabular sequence
                let num = format!("{:08x}\t{:10}\n", prng.next_u32(), prng.next_u64() % 1000000);
                let b = num.as_bytes();
                let remaining = size - out.len();
                let to_copy = b.len().min(remaining);
                out.extend_from_slice(&b[..to_copy]);
            } else {
                // Repetitive phrase chunk
                let repeat_chunk = b"--- [SILESIA RECORD SECTION] ---\n";
                let remaining = size - out.len();
                let to_copy = repeat_chunk.len().min(remaining);
                out.extend_from_slice(&repeat_chunk[..to_copy]);
            }
        }

        out.truncate(size);
        out
    }

    /// (2) Enwik8-like distribution: Structured XML elements, links, templates, and UTF-8 text.
    pub fn generate_enwik8_like(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }

        let mut prng = FastPrng::new(0xE8171C8);
        let mut out = Vec::with_capacity(size);

        let titles = [
            "Anarchism", "Autism", "Albedo", "A_Huge_Ever_Growing_Pulsating_Brain",
            "Apple_Silicon", "Deflate_Algorithm", "Zlib_Ng_Optimization", "Quantum_Computing",
        ];

        let mut page_id = 1u32;

        while out.len() < size {
            let title = titles[prng.next_range(0, titles.len() - 1)];
            let header = format!(
                "<page>\n  <title>{}</title>\n  <id>{}</id>\n  <revision>\n    <id>{}</id>\n    <text xml:space=\"preserve\">",
                title, page_id, page_id * 107
            );
            page_id += 1;

            let remaining = size - out.len();
            let to_copy = header.len().min(remaining);
            out.extend_from_slice(&header.as_bytes()[..to_copy]);

            // Article body paragraphs with wiki markup
            let body_paragraphs = prng.next_range(2, 6);
            for _ in 0..body_paragraphs {
                if out.len() >= size {
                    break;
                }
                let wiki_chunk = format!(
                    "'''{}''' is a notable topic with [[hyperlink]] references and {{{{template_cite|id={}}}}}. &amp;amp; &quot;quote&quot;.\n",
                    title, prng.next_u32() % 500
                );
                let rem = size - out.len();
                let n = wiki_chunk.len().min(rem);
                out.extend_from_slice(&wiki_chunk.as_bytes()[..n]);
            }

            if out.len() < size {
                let footer = "</text>\n  </revision>\n</page>\n";
                let rem = size - out.len();
                let n = footer.len().min(rem);
                out.extend_from_slice(&footer.as_bytes()[..n]);
            }
        }

        out.truncate(size);
        out
    }

    /// (3) High-Entropy distribution: Deterministic incompressible white noise.
    pub fn generate_high_entropy(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }

        let mut prng = FastPrng::new(0x4816_8E47);
        let mut out = vec![0u8; size];

        let mut i = 0;
        while i + 8 <= size {
            let val = prng.next_u64();
            out[i..i + 8].copy_from_slice(&val.to_le_bytes());
            i += 8;
        }

        while i < size {
            out[i] = prng.next_u8();
            i += 1;
        }

        out
    }

    /// (4) Low-Entropy Periodic: Deterministic cyclic wave patterns with configurable period.
    pub fn generate_low_entropy_periodic(size: usize, period: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }

        let eff_period = period.clamp(1, 4096);
        let mut base_pattern = Vec::with_capacity(eff_period);

        for i in 0..eff_period {
            // Sinusoidal wave + saw-tooth shape
            let angle = (i as f64) * 2.0 * std::f64::consts::PI / (eff_period as f64);
            let val = ((angle.sin() + 1.0) * 127.5) as u8;
            base_pattern.push(val);
        }

        let mut out = Vec::with_capacity(size);
        while out.len() < size {
            let remaining = size - out.len();
            let to_copy = eff_period.min(remaining);
            out.extend_from_slice(&base_pattern[..to_copy]);
        }

        out
    }

    /// (5) Zero-Sparse: Long stretches of null bytes with sparse deterministic entropy bursts.
    pub fn generate_zero_sparse(size: usize, sparsity_ratio: f64) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }

        let ratio = sparsity_ratio.clamp(0.0, 1.0);
        let mut prng = FastPrng::new(0x2E80_59A85E);
        let mut out = vec![0u8; size];

        let non_zero_prob = 1.0 - ratio;
        for byte in out.iter_mut() {
            if prng.next_f64() < non_zero_prob {
                *byte = (prng.next_u32() % 255 + 1) as u8;
            }
        }

        out
    }

    /// (6) Repetitive LZ77 Clusters: High match density with clustered repetition.
    pub fn generate_lz77_clusters(
        size: usize,
        cluster_size: usize,
        repeat_count: usize,
    ) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }

        let eff_cluster = cluster_size.clamp(4, 256);
        let eff_repeats = repeat_count.clamp(2, 1024);
        let mut prng = FastPrng::new(0x1277_C1057);
        let mut out = Vec::with_capacity(size);

        while out.len() < size {
            // Generate a distinct cluster template
            let mut template = vec![0u8; eff_cluster];
            for b in template.iter_mut() {
                *b = (prng.next_range(32, 126)) as u8;
            }

            // Repeat the cluster `eff_repeats` times
            for _ in 0..eff_repeats {
                if out.len() >= size {
                    break;
                }
                let remaining = size - out.len();
                let to_copy = eff_cluster.min(remaining);
                out.extend_from_slice(&template[..to_copy]);
            }

            // Insert random bridging bytes
            let bridge_len = prng.next_range(2, 16);
            for _ in 0..bridge_len {
                if out.len() >= size {
                    break;
                }
                out.push(prng.next_u8());
            }
        }

        out.truncate(size);
        out
    }

    /// (7) Code & Structured AST Data: Indented keyword streams, tokens, and symbols.
    pub fn generate_code_structure(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }

        let mut prng = FastPrng::new(0x0C0D_EA57);
        let mut out = Vec::with_capacity(size);

        let keywords = [
            "fn ", "pub fn ", "let mut ", "let ", "struct ", "enum ", "impl ",
            "match ", "if ", "else { ", "return ", "Ok(", "Err(", "self.", "async ",
        ];

        let idents = [
            "buffer", "offset", "length", "capacity", "stream", "header", "payload",
            "context", "handle", "result", "status", "encoder", "decoder", "matcher",
        ];

        let operators = [" = ", " + ", " - ", " * ", " & ", " | ", " >> ", " << ", " != "];

        let mut indent_level = 0usize;

        while out.len() < size {
            let mut line = String::new();

            // Indentation
            for _ in 0..indent_level {
                line.push_str("    ");
            }

            let line_type = prng.next_range(0, 5);
            match line_type {
                0 => {
                    // Function declaration
                    let fn_name = idents[prng.next_range(0, idents.len() - 1)];
                    line.push_str(&format!("pub fn {}_handler(&mut self) -> Result<usize, Error> {{\n", fn_name));
                    indent_level = (indent_level + 1).min(6);
                }
                1 => {
                    // Variable assignment
                    let id_left = idents[prng.next_range(0, idents.len() - 1)];
                    let id_right = idents[prng.next_range(0, idents.len() - 1)];
                    let op = operators[prng.next_range(0, operators.len() - 1)];
                    line.push_str(&format!("let {} = {}{}{};\n", id_left, id_right, op, prng.next_range(1, 1024)));
                }
                2 => {
                    // Match expression
                    let id = idents[prng.next_range(0, idents.len() - 1)];
                    line.push_str(&format!("match self.{} {{\n", id));
                    indent_level = (indent_level + 1).min(6);
                }
                3 => {
                    // Block closing
                    if indent_level > 0 {
                        indent_level -= 1;
                        line.clear();
                        for _ in 0..indent_level {
                            line.push_str("    ");
                        }
                    }
                    line.push_str("}\n");
                }
                _ => {
                    // Method invocation
                    let kw = keywords[prng.next_range(0, keywords.len() - 1)];
                    let id = idents[prng.next_range(0, idents.len() - 1)];
                    line.push_str(&format!("{}{}.compute_bound();\n", kw, id));
                }
            }

            let b = line.as_bytes();
            let remaining = size - out.len();
            let to_copy = b.len().min(remaining);
            out.extend_from_slice(&b[..to_copy]);
        }

        out.truncate(size);
        out
    }

    /// (8) Multimodal Interleaved: Compound file container simulation (text, binary, sparse, noise).
    pub fn generate_multimodal_interleaved(size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }

        let mut prng = FastPrng::new(0x30D4_1107);
        let mut out = Vec::with_capacity(size);

        while out.len() < size {
            let section_type = prng.next_range(0, 4);
            let section_size = prng.next_range(64, 4096).min(size - out.len());

            let chunk = match section_type {
                0 => Self::generate_code_structure(section_size),
                1 => Self::generate_zero_sparse(section_size, 0.98),
                2 => Self::generate_high_entropy(section_size),
                _ => Self::generate_enwik8_like(section_size),
            };

            let remaining = size - out.len();
            let to_copy = chunk.len().min(remaining);
            out.extend_from_slice(&chunk[..to_copy]);
        }

        out.truncate(size);
        out
    }
}
