// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Terminal 2D Braille Pareto Frontier Canvas & MIPS Benchmark Plotter.

use ttzip_engine::bench::{compute_pareto_frontier_raw, MIPSHardwareBenchmarkEngine, ParetoPointRaw};

/// Single Braille cell representing a 2x4 subpixel dot matrix (U+2800..U+28FF).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BrailleCell {
    pub mask: u8,
}

impl BrailleCell {
    pub const fn new() -> Self {
        Self { mask: 0 }
    }

    #[inline]
    fn dot_bit(sub_x: usize, sub_y: usize) -> u8 {
        match (sub_x, sub_y) {
            (0, 0) => 0x01,
            (0, 1) => 0x02,
            (0, 2) => 0x04,
            (1, 0) => 0x08,
            (1, 1) => 0x10,
            (1, 2) => 0x20,
            (0, 3) => 0x40,
            (1, 3) => 0x80,
            _ => 0,
        }
    }

    pub fn set_dot(&mut self, sub_x: usize, sub_y: usize) {
        if sub_x < 2 && sub_y < 4 {
            self.mask |= Self::dot_bit(sub_x, sub_y);
        }
    }

    pub fn clear_dot(&mut self, sub_x: usize, sub_y: usize) {
        if sub_x < 2 && sub_y < 4 {
            self.mask &= !Self::dot_bit(sub_x, sub_y);
        }
    }

    pub fn get_dot(&self, sub_x: usize, sub_y: usize) -> bool {
        sub_x < 2 && sub_y < 4 && (self.mask & Self::dot_bit(sub_x, sub_y)) != 0
    }

    pub fn to_char(&self) -> char {
        if self.mask == 0 { ' ' } else { char::from_u32(0x2800 + self.mask as u32).unwrap_or(' ') }
    }

    pub fn to_braille_char(&self) -> char {
        char::from_u32(0x2800 + self.mask as u32).unwrap_or(' ')
    }
}

/// 2D Canvas supporting Bresenham rasterization over virtual subpixels using Braille patterns.
#[derive(Debug, Clone)]
pub struct TerminalBrailleCanvas {
    pub width_chars: usize,
    pub height_chars: usize,
    cells: Vec<BrailleCell>,
}

impl TerminalBrailleCanvas {
    pub fn new(width_chars: usize, height_chars: usize) -> Self {
        let w = width_chars.max(1);
        let h = height_chars.max(1);
        Self { width_chars: w, height_chars: h, cells: vec![BrailleCell::new(); w * h] }
    }

    #[inline]
    pub fn width_dots(&self) -> usize { self.width_chars * 2 }

    #[inline]
    pub fn height_dots(&self) -> usize { self.height_chars * 4 }

    pub fn set_dot(&mut self, dot_x: usize, dot_y: usize) {
        if dot_x < self.width_dots() && dot_y < self.height_dots() {
            let idx = (dot_y / 4) * self.width_chars + (dot_x / 2);
            self.cells[idx].set_dot(dot_x % 2, dot_y % 4);
        }
    }

    pub fn get_dot(&self, dot_x: usize, dot_y: usize) -> bool {
        if dot_x < self.width_dots() && dot_y < self.height_dots() {
            let idx = (dot_y / 4) * self.width_chars + (dot_x / 2);
            self.cells[idx].get_dot(dot_x % 2, dot_y % 4)
        } else {
            false
        }
    }

    /// Bresenham's line algorithm for smooth subpixel trajectory rasterization.
    pub fn draw_line(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        let (mut x0, mut y0) = (x0 as isize, y0 as isize);
        let (x1, y1) = (x1 as isize, y1 as isize);
        let (dx, sx) = ((x1 - x0).abs(), if x0 < x1 { 1 } else { -1 });
        let (dy, sy) = (-(y1 - y0).abs(), if y0 < y1 { 1 } else { -1 });
        let mut err = dx + dy;

        loop {
            if x0 >= 0 && y0 >= 0 { self.set_dot(x0 as usize, y0 as usize); }
            if x0 == x1 && y0 == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x0 += sx; }
            if e2 <= dx { err += dx; y0 += sy; }
        }
    }

    pub fn render_rows(&self) -> Vec<String> {
        (0..self.height_chars)
            .map(|y| (0..self.width_chars).map(|x| self.cells[y * self.width_chars + x].to_char()).collect())
            .collect()
    }
}

/// Coordinate mapping engine with log10 X projection and linear Y projection.
#[derive(Debug, Clone)]
pub struct ParetoPlotCoordinateEngine {
    pub min_throughput_mbs: f64,
    pub max_throughput_mbs: f64,
    pub min_savings_pct: f64,
    pub max_savings_pct: f64,
    pub width_dots: usize,
    pub height_dots: usize,
}

impl ParetoPlotCoordinateEngine {
    pub fn new(min_t: f64, max_t: f64, min_s: f64, max_s: f64, w_dots: usize, h_dots: usize) -> Self {
        let t_min = if min_t <= 0.0 { 1.0 } else { min_t };
        let t_max = if max_t <= t_min { t_min * 10.0 } else { max_t };
        let s_min = min_s.max(0.0);
        let s_max = if max_s <= s_min { (s_min + 10.0).min(100.0) } else { max_s.min(100.0) };
        Self {
            min_throughput_mbs: t_min,
            max_throughput_mbs: t_max,
            min_savings_pct: s_min,
            max_savings_pct: s_max,
            width_dots: w_dots.max(2),
            height_dots: h_dots.max(4),
        }
    }

    pub fn auto_fit(points: &[ParetoPointRaw], width_dots: usize, height_dots: usize) -> Self {
        let (mut min_t, mut max_t) = (f64::MAX, f64::MIN);
        let (mut min_s, mut max_s) = (0.0_f64, 0.0_f64);

        for p in points {
            if p.throughput_mbs > 0.0 {
                min_t = min_t.min(p.throughput_mbs);
                max_t = max_t.max(p.throughput_mbs);
            }
            min_s = min_s.min(p.space_savings_pct);
            max_s = max_s.max(p.space_savings_pct);
        }

        let (t_min, t_max) = if min_t == f64::MAX { (10.0, 2500.0) } else { ((min_t * 0.8).max(1.0), max_t * 1.2) };
        Self::new(t_min, t_max, (min_s - 5.0).max(0.0), (max_s + 5.0).min(100.0), width_dots, height_dots)
    }

    pub fn map_point(&self, throughput_mbs: f64, space_savings_pct: f64) -> (usize, usize) {
        let (log_min, log_max) = (self.min_throughput_mbs.log10(), self.max_throughput_mbs.log10());
        let log_val = throughput_mbs.max(self.min_throughput_mbs).min(self.max_throughput_mbs).log10();
        let norm_x = if log_max > log_min { ((log_val - log_min) / (log_max - log_min)).clamp(0.0, 1.0) } else { 0.0 };
        let norm_y = if self.max_savings_pct > self.min_savings_pct {
            ((space_savings_pct - self.min_savings_pct) / (self.max_savings_pct - self.min_savings_pct)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let dot_x = ((norm_x * ((self.width_dots - 1) as f64)).round() as usize).min(self.width_dots - 1);
        let dot_y = (((1.0 - norm_y) * ((self.height_dots - 1) as f64)).round() as usize).min(self.height_dots - 1);
        (dot_x, dot_y)
    }
}

use std::time::Instant;
use ttzip_engine::bench::{
    BenchmarkCorpusGenerator, BenchmarkCorpusType, MatrixCodecConfig, MatrixCodecDriver,
};

/// Benchmark item with metadata and Pareto analysis fields.
#[derive(Debug, Clone)]
pub struct BenchmarkCodecItem {
    pub name: String,
    pub level: String,
    pub throughput_mbs: f64,
    pub space_savings_pct: f64,
    pub raw: ParetoPointRaw,
}

pub fn get_live_benchmark_dataset() -> Vec<BenchmarkCodecItem> {
    let corpus = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::Silesia, 256 * 1024);
    let orig_len = corpus.len();

    let configs = vec![
        MatrixCodecConfig::new("Snappy", 1, "Snappy"),
        MatrixCodecConfig::new("LZ4", 1, "LZ4 Fast 1"),
        MatrixCodecConfig::new("LZFSE", 1, "Apple LZFSE"),
        MatrixCodecConfig::new("Zstd", 1, "Zstd L1"),
        MatrixCodecConfig::new("Zstd", 3, "Zstd L3"),
        MatrixCodecConfig::new("Zstd", 9, "Zstd L9"),
        MatrixCodecConfig::new("Libdeflate", 1, "Deflate L1"),
        MatrixCodecConfig::new("Libdeflate", 6, "Deflate L6"),
        MatrixCodecConfig::new("Libdeflate", 9, "Deflate L9"),
        MatrixCodecConfig::new("Brotli", 1, "Brotli L1"),
        MatrixCodecConfig::new("Brotli", 6, "Brotli L6"),
        MatrixCodecConfig::new("Brotli", 9, "Brotli L9"),
        MatrixCodecConfig::new("Bzip2", 1, "Bzip2 L1"),
        MatrixCodecConfig::new("Bzip2", 6, "Bzip2 L6"),
        MatrixCodecConfig::new("Bzip2", 9, "Bzip2 L9"),
    ];

    let mut raw_points = Vec::with_capacity(configs.len());
    let mut items_meta = Vec::with_capacity(configs.len());

    for (idx, cfg) in configs.iter().enumerate() {
        let _ = MatrixCodecDriver::compress(cfg, &corpus);

        let iterations = 2;
        let start = Instant::now();
        let mut comp_len = orig_len;
        for _ in 0..iterations {
            if let Ok(comp) = MatrixCodecDriver::compress(cfg, &corpus) {
                comp_len = comp.len();
            }
        }
        let elapsed = start.elapsed().as_secs_f64().max(0.00001);
        let total_mb = ((orig_len * iterations) as f64) / (1024.0 * 1024.0);
        let throughput = total_mb / elapsed;
        let space_savings = ((1.0 - (comp_len as f64 / orig_len as f64)) * 100.0).clamp(0.0, 99.9);

        let raw_p = ParetoPointRaw::new(idx as u64, throughput, space_savings);
        raw_points.push(raw_p);
        items_meta.push((cfg.algorithm.clone(), format!("L{}", cfg.level), throughput, space_savings, idx as u64));
    }

    compute_pareto_frontier_raw(&mut raw_points);

    items_meta
        .into_iter()
        .map(|(algo, lvl, tp, ss, tag)| {
            let p_raw = raw_points
                .iter()
                .find(|p| p.tag == tag)
                .copied()
                .unwrap_or(ParetoPointRaw::new(tag, tp, ss));
            BenchmarkCodecItem {
                name: algo,
                level: lvl,
                throughput_mbs: tp,
                space_savings_pct: ss,
                raw: p_raw,
            }
        })
        .collect()
}

pub fn get_standard_benchmark_dataset() -> Vec<BenchmarkCodecItem> {
    get_live_benchmark_dataset()
}

pub fn render_pareto_chart(items: &[BenchmarkCodecItem], width_chars: usize, height_chars: usize) -> Vec<String> {
    let mut canvas = TerminalBrailleCanvas::new(width_chars, height_chars);
    let raw_pts: Vec<ParetoPointRaw> = items.iter().map(|it| it.raw).collect();
    let engine = ParetoPlotCoordinateEngine::auto_fit(&raw_pts, canvas.width_dots(), canvas.height_dots());

    let mut hull_items: Vec<&BenchmarkCodecItem> = items.iter().filter(|it| it.raw.is_on_convex_envelope).collect();
    hull_items.sort_by(|a, b| a.throughput_mbs.partial_cmp(&b.throughput_mbs).unwrap_or(std::cmp::Ordering::Equal));

    for window in hull_items.windows(2) {
        let (x0, y0) = engine.map_point(window[0].throughput_mbs, window[0].space_savings_pct);
        let (x1, y1) = engine.map_point(window[1].throughput_mbs, window[1].space_savings_pct);
        canvas.draw_line(x0, y0, x1, y1);
    }

    for it in items {
        let (x, y) = engine.map_point(it.throughput_mbs, it.space_savings_pct);
        canvas.set_dot(x, y);
    }

    let mut output = Vec::new();
    output.push("  Savings (%)".to_string());
    for (r, row_str) in canvas.render_rows().iter().enumerate() {
        let pct = engine.max_savings_pct - (r as f64 / (height_chars - 1).max(1) as f64) * (engine.max_savings_pct - engine.min_savings_pct);
        output.push(format!("{:>5.1}% │ {}", pct, row_str));
    }
    output.push(format!("{:>7}└{}► Throughput", "", "─".repeat(width_chars)));

    let (min_t, max_t) = (engine.min_throughput_mbs, engine.max_throughput_mbs);
    let mid_t = 10.0_f64.powf((min_t.log10() + max_t.log10()) / 2.0);
    output.push(format!("{:>7} {:<15.1} {:<15.1} {:.1} MB/s (log10)", "", min_t, mid_t, max_t));
    output
}

pub fn render_summary_table(items: &[BenchmarkCodecItem]) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("+{:-<20}+{:-<9}+{:-<18}+{:-<15}+{:-<6}+{:-<9}+{:-<13}+", "", "", "", "", "", "", ""));
    lines.push(format!("| {:<18} | {:<7} | {:>16} | {:>13} | {:^4} | {:^7} | {:^11} |", "Algorithm / Codec", "Level", "Throughput(MB/s)", "Space Savings", "Rank", "Optimal", "Convex Hull"));
    lines.push(format!("+{:-<20}+{:-<9}+{:-<18}+{:-<15}+{:-<6}+{:-<9}+{:-<13}+", "", "", "", "", "", "", ""));

    let mut sorted = items.to_vec();
    sorted.sort_by(|a, b| a.raw.pareto_rank.cmp(&b.raw.pareto_rank).then_with(|| b.throughput_mbs.partial_cmp(&a.throughput_mbs).unwrap_or(std::cmp::Ordering::Equal)));

    for it in &sorted {
        let opt_str = if it.raw.is_pareto_optimal { "Yes" } else { "No" };
        let hull_str = if it.raw.is_on_convex_envelope { "Yes" } else { "No" };
        lines.push(format!(
            "| {:<18} | {:<7} | {:>16.2} | {:>12.2}% | {:^4} | {:^7} | {:^11} |",
            it.name, it.level, it.throughput_mbs, it.space_savings_pct, it.raw.pareto_rank, opt_str, hull_str
        ));
    }
    lines.push(format!("+{:-<20}+{:-<9}+{:-<18}+{:-<15}+{:-<6}+{:-<9}+{:-<13}+", "", "", "", "", "", "", ""));
    lines
}

pub fn run_cli_benchmark(mips: bool, pareto: bool, threads: u32, dict_mb: u32, iterations: u32) -> Result<(), String> {
    let run_all = !mips && !pareto;

    if mips || run_all {
        println!("\n=== 7-Zip Standard Hardware MIPS Benchmark ===");
        println!("Dictionary Size: {} MB | Threads: {} | Iterations: {}", dict_mb, threads, iterations);
        let res = MIPSHardwareBenchmarkEngine::run_benchmark(dict_mb, threads, iterations)
            .map_err(|e| format!("MIPS benchmark execution failed: {:?}", e))?;
        println!("{:-<76}", "");
        println!("Compress Speed   : {:>10.2} MB/s  | Compress Rating   : {:>10.0} MIPS", res.compress_speed_mbs, res.compress_mips);
        println!("Decompress Speed : {:>10.2} MB/s  | Decompress Rating : {:>10.0} MIPS", res.decompress_speed_mbs, res.decompress_mips);
        println!("Total Rating     : {:>10.0} MIPS  | CPU Usage         : {:>10.0} %", res.total_mips, res.cpu_usage_percent);
        println!("Rating / Usage   : {:>10.0} MIPS/Core", res.rating_per_usage_mips);
        println!("{:-<76}", "");
    }

    if pareto || run_all {
        println!("\n=== 2D Braille Pareto Frontier & Upper Convex Hull Plotter ===");
        let dataset = get_standard_benchmark_dataset();
        for line in render_pareto_chart(&dataset, 54, 14) {
            println!("{}", line);
        }
        println!("\n=== Multi-Algorithm Pareto Rank & Convex Envelope Summary ===");
        for line in render_summary_table(&dataset) {
            println!("{}", line);
        }
    }

    Ok(())
}
