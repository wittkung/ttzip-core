// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Synthetic Canterbury and Silesia corpus generator and benchmark repetition harness.

use std::time::{Duration, Instant};

use ttzip_engine::bench_telemetry::{MemoryMetrics, MemoryTracker};
use ttzip_engine::zip::ZipInputItem;

pub const WARMUP_RUNS: usize = 2;
pub const MEASURE_RUNS: usize = 5;
pub const CHUNK_SIZE_64KB: usize = 64 * 1024;

/// Deterministic synthetic workload generator with controllable Shannon entropy.
pub struct SyntheticCorpus {
    pub files: Vec<(String, Vec<u8>)>,
    pub total_uncompressed_bytes: usize,
    pub empirical_entropy: f64,
}

impl SyntheticCorpus {
    /// Generates a controllable entropy corpus modeling text, code, binary tables, and records.
    pub fn new(target_size_bytes: usize, entropy_factor: f64) -> Self {
        let clamped_entropy = entropy_factor.clamp(0.0, 1.0);
        let part_size = (target_size_bytes / 4).max(1024);
        let mut rng = 0x8543_291a_ef01_472b_u64;

        // File 1: Canterbury-like English prose / text (Zipf power-law distribution)
        let text_data = Self::gen_text_stream(part_size, clamped_entropy, &mut rng);

        // File 2: Source code / structured syntax tokens
        let code_data = Self::gen_code_stream(part_size, clamped_entropy, &mut rng);

        // File 3: Structured tabular / XML / JSON records
        let xml_data = Self::gen_xml_records(part_size, clamped_entropy, &mut rng);

        // File 4: Mach-O binary instructions and DWARF tables
        let bin_data = Self::gen_binary_stream(part_size, clamped_entropy, &mut rng);

        let files = vec![
            ("canterbury/text.txt".to_string(), text_data),
            ("canterbury/source.rs".to_string(), code_data),
            ("silesia/records.xml".to_string(), xml_data),
            ("silesia/macho.bin".to_string(), bin_data),
        ];

        let total_uncompressed_bytes = files.iter().map(|(_, data)| data.len()).sum();
        let empirical_entropy = Self::calculate_empirical_entropy(&files);

        Self {
            files,
            total_uncompressed_bytes,
            empirical_entropy,
        }
    }

    /// Converts files into canonical `ZipInputItem` descriptors.
    pub fn to_zip_input_items(&self) -> Vec<ZipInputItem> {
        self.files
            .iter()
            .map(|(path, data)| ZipInputItem {
                rel_path: path.clone(),
                data: data.clone(),
                mtime_epoch_secs: 1770000000,
                mode: 0o644,
                is_directory: false,
            })
            .collect()
    }

    fn next_u64(rng: &mut u64) -> u64 {
        *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *rng
    }

    fn gen_text_stream(size: usize, entropy: f64, rng: &mut u64) -> Vec<u8> {
        let dictionary = [
            "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog",
            "benchmark", "kernel", "throughput", "compression", "vector", "pipeline",
            "microarchitecture", "register", "cache", "resident", "telemetry", "entropy",
        ];
        let mut buf = Vec::with_capacity(size);
        while buf.len() < size {
            let r = Self::next_u64(rng);
            let word_idx = ((r >> 16) as usize) % dictionary.len();
            let word = dictionary[word_idx];
            buf.extend_from_slice(word.as_bytes());
            buf.push(b' ');

            if (r as f64 / u64::MAX as f64) < entropy * 0.15 {
                buf.push((r & 0xFF) as u8);
            }
        }
        buf.truncate(size);
        buf
    }

    fn gen_code_stream(size: usize, entropy: f64, rng: &mut u64) -> Vec<u8> {
        let tokens = [
            "pub fn ", "let mut ", "match ", "impl Send for ", "unsafe { ",
            "return Ok(());", "const BLOCK: usize = 64 * 1024;\n", "if x > 0 { ",
            "} else { ", "for item in &items { ", "assert_eq!(a, b);\n",
        ];
        let mut buf = Vec::with_capacity(size);
        while buf.len() < size {
            let r = Self::next_u64(rng);
            let token = tokens[((r >> 20) as usize) % tokens.len()];
            buf.extend_from_slice(token.as_bytes());
            if (r as f64 / u64::MAX as f64) < entropy * 0.10 {
                buf.push(((r >> 8) & 0xFF) as u8);
            }
        }
        buf.truncate(size);
        buf
    }

    fn gen_xml_records(size: usize, entropy: f64, rng: &mut u64) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size);
        let mut id = 1000u32;
        while buf.len() < size {
            let r = Self::next_u64(rng);
            let record = format!(
                "<record id=\"{}\" flag=\"{}\"><val>{:.2}</val></record>\n",
                id,
                if (r & 1) == 0 { "true" } else { "false" },
                (r % 10000) as f64 / 100.0
            );
            id += 1;
            buf.extend_from_slice(record.as_bytes());
            if (r as f64 / u64::MAX as f64) < entropy * 0.20 {
                buf.push((r & 0xFF) as u8);
            }
        }
        buf.truncate(size);
        buf
    }

    fn gen_binary_stream(size: usize, entropy: f64, rng: &mut u64) -> Vec<u8> {
        let mut buf = vec![0u8; size];
        for chunk in buf.chunks_mut(8) {
            let r = Self::next_u64(rng);
            if (r as f64 / u64::MAX as f64) < entropy {
                chunk.copy_from_slice(&r.to_le_bytes()[..chunk.len()]);
            } else {
                chunk[0] = 0xAA;
                chunk[chunk.len() - 1] = 0x55;
            }
        }
        buf
    }

    fn calculate_empirical_entropy(files: &[(String, Vec<u8>)]) -> f64 {
        let mut counts = [0usize; 256];
        let mut total = 0usize;
        for (_, data) in files {
            for &byte in data {
                counts[byte as usize] += 1;
                total += 1;
            }
        }
        if total == 0 {
            return 0.0;
        }
        let total_f = total as f64;
        let mut entropy = 0.0;
        for &count in &counts {
            if count > 0 {
                let p = count as f64 / total_f;
                entropy -= p * p.log2();
            }
        }
        entropy
    }
}

/// Runs repeated measurements collecting throughput and latency sample series.
pub fn benchmark_repeats<F: FnMut()>(runs: usize, mut op: F) -> (Duration, MemoryMetrics, Vec<f64>) {
    for _ in 0..WARMUP_RUNS {
        op();
    }
    let mut samples_duration = Vec::with_capacity(runs);
    let mut tracker = MemoryTracker::start();
    for _ in 0..runs {
        let t0 = Instant::now();
        op();
        samples_duration.push(t0.elapsed());
    }
    let mem = tracker.finish();
    let mean_duration = samples_duration.iter().sum::<Duration>() / (runs as u32);
    let latencies_micros: Vec<f64> = samples_duration
        .iter()
        .map(|d| d.as_secs_f64() * 1_000_000.0)
        .collect();
    (mean_duration, mem, latencies_micros)
}
