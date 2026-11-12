// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 2D Pareto non-dominated frontier and Andrew's Monotone Chain upper convex hull algorithms.

use std::ffi::CStr;
use std::os::raw::c_char;

const EPSILON: f64 = 1e-7;

/// 2D Pareto and Upper Convex Hull point representation for compression codecs.
#[derive(Debug, Clone, PartialEq)]
pub struct ParetoCodecPoint {
    pub codec_name: String,
    pub compression_ratio: f64,
    pub speed_mb_s: f64,
    pub memory_mb: f64,
    pub pareto_rank: u32,
    pub is_pareto_optimal: bool,
    pub is_on_convex_hull: bool,
}

impl ParetoCodecPoint {
    pub fn new(
        codec_name: impl Into<String>,
        compression_ratio: f64,
        speed_mb_s: f64,
        memory_mb: f64,
    ) -> Self {
        Self {
            codec_name: codec_name.into(),
            compression_ratio,
            speed_mb_s,
            memory_mb,
            pareto_rank: 1,
            is_pareto_optimal: false,
            is_on_convex_hull: false,
        }
    }
}

/// Raw C-ABI compatible Pareto codec point for zero-copy FFI bridging.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TTZipParetoCodecPointRaw {
    pub codec_name: [c_char; 64],
    pub compression_ratio: f64,
    pub speed_mb_s: f64,
    pub memory_mb: f64,
    pub pareto_rank: u32,
    pub is_pareto_optimal: bool,
    pub is_on_convex_hull: bool,
}

impl TTZipParetoCodecPointRaw {
    pub fn new(codec_name_str: &str, compression_ratio: f64, speed_mb_s: f64, memory_mb: f64) -> Self {
        let mut name_buf = [0 as c_char; 64];
        let bytes = codec_name_str.as_bytes();
        let copy_len = bytes.len().min(63);
        for i in 0..copy_len {
            name_buf[i] = bytes[i] as c_char;
        }
        Self {
            codec_name: name_buf,
            compression_ratio,
            speed_mb_s,
            memory_mb,
            pareto_rank: 1,
            is_pareto_optimal: false,
            is_on_convex_hull: false,
        }
    }

    pub fn name_as_str(&self) -> &str {
        unsafe {
            CStr::from_ptr(self.codec_name.as_ptr())
                .to_str()
                .unwrap_or("")
        }
    }
}

/// Raw C-ABI compatible Pareto point structure for high-performance zero-copy bridging.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParetoPointRaw {
    pub tag: u64,
    pub throughput_mbs: f64,
    pub space_savings_pct: f64,
    pub pareto_rank: u32,
    pub is_pareto_optimal: bool,
    pub is_on_convex_envelope: bool,
}

impl ParetoPointRaw {
    pub fn new(tag: u64, throughput_mbs: f64, space_savings_pct: f64) -> Self {
        Self {
            tag,
            throughput_mbs,
            space_savings_pct,
            pareto_rank: 1,
            is_pareto_optimal: false,
            is_on_convex_envelope: false,
        }
    }
}

trait ParetoPoint2D {
    fn x_coord(&self) -> f64;
    fn y_coord(&self) -> f64;
    fn set_pareto_state(&mut self, rank: u32, is_optimal: bool, is_on_hull: bool);
    fn is_optimal(&self) -> bool;
}

impl ParetoPoint2D for ParetoCodecPoint {
    fn x_coord(&self) -> f64 {
        self.speed_mb_s
    }
    fn y_coord(&self) -> f64 {
        self.compression_ratio
    }
    fn set_pareto_state(&mut self, rank: u32, is_optimal: bool, is_on_hull: bool) {
        self.pareto_rank = rank;
        self.is_pareto_optimal = is_optimal;
        self.is_on_convex_hull = is_on_hull;
    }
    fn is_optimal(&self) -> bool {
        self.is_pareto_optimal
    }
}

impl ParetoPoint2D for TTZipParetoCodecPointRaw {
    fn x_coord(&self) -> f64 {
        self.speed_mb_s
    }
    fn y_coord(&self) -> f64 {
        self.compression_ratio
    }
    fn set_pareto_state(&mut self, rank: u32, is_optimal: bool, is_on_hull: bool) {
        self.pareto_rank = rank;
        self.is_pareto_optimal = is_optimal;
        self.is_on_convex_hull = is_on_hull;
    }
    fn is_optimal(&self) -> bool {
        self.is_pareto_optimal
    }
}

impl ParetoPoint2D for ParetoPointRaw {
    fn x_coord(&self) -> f64 {
        self.throughput_mbs
    }
    fn y_coord(&self) -> f64 {
        self.space_savings_pct
    }
    fn set_pareto_state(&mut self, rank: u32, is_optimal: bool, is_on_hull: bool) {
        self.pareto_rank = rank;
        self.is_pareto_optimal = is_optimal;
        self.is_on_convex_envelope = is_on_hull;
    }
    fn is_optimal(&self) -> bool {
        self.is_pareto_optimal
    }
}

fn compute_pareto_2d<P: ParetoPoint2D>(points: &mut [P]) {
    if points.is_empty() {
        return;
    }
    if points.len() == 1 {
        points[0].set_pareto_state(1, true, true);
        return;
    }

    points.sort_by(|a, b| {
        let diff_x = a.x_coord() - b.x_coord();
        if diff_x.abs() > EPSILON {
            b.x_coord()
                .partial_cmp(&a.x_coord())
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            b.y_coord()
                .partial_cmp(&a.y_coord())
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    assign_patience_pareto_ranks(points);
    let frontier_indices = extract_sorted_frontier_indices(points);
    mark_upper_convex_hull(points, &frontier_indices);
}

fn assign_patience_pareto_ranks<P: ParetoPoint2D>(points: &mut [P]) {
    let mut target_tiers: Vec<f64> = Vec::new();
    for pt in points.iter_mut() {
        let cur_y = pt.y_coord();
        let mut left = 0;
        let mut right = target_tiers.len();
        while left < right {
            let mid = (left + right) / 2;
            if cur_y > target_tiers[mid] + EPSILON {
                right = mid;
            } else {
                left = mid + 1;
            }
        }

        let rank = if left < target_tiers.len() {
            target_tiers[left] = cur_y;
            (left + 1) as u32
        } else {
            target_tiers.push(cur_y);
            target_tiers.len() as u32
        };
        pt.set_pareto_state(rank, rank == 1, false);
    }
}

fn extract_sorted_frontier_indices<P: ParetoPoint2D>(points: &[P]) -> Vec<usize> {
    let mut frontier_indices: Vec<usize> = points
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_optimal())
        .map(|(idx, _)| idx)
        .collect();

    frontier_indices.sort_by(|&i, &j| {
        let diff_x = points[i].x_coord() - points[j].x_coord();
        if diff_x.abs() > EPSILON {
            points[i]
                .x_coord()
                .partial_cmp(&points[j].x_coord())
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            points[i]
                .y_coord()
                .partial_cmp(&points[j].y_coord())
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    frontier_indices
}

fn mark_upper_convex_hull<P: ParetoPoint2D>(points: &mut [P], frontier_indices: &[usize]) {
    if frontier_indices.len() <= 2 {
        for &idx in frontier_indices {
            points[idx].set_pareto_state(1, true, true);
        }
        return;
    }

    let mut upper_hull_indices: Vec<usize> = Vec::with_capacity(frontier_indices.len());
    for &idx in frontier_indices {
        let p = &points[idx];
        while upper_hull_indices.len() >= 2 {
            let a = &points[upper_hull_indices[upper_hull_indices.len() - 2]];
            let b = &points[upper_hull_indices[upper_hull_indices.len() - 1]];
            let cross_product = (b.x_coord() - a.x_coord()) * (p.y_coord() - a.y_coord())
                - (b.y_coord() - a.y_coord()) * (p.x_coord() - a.x_coord());
            if cross_product >= -EPSILON {
                upper_hull_indices.pop();
            } else {
                break;
            }
        }
        upper_hull_indices.push(idx);
    }

    for &idx in &upper_hull_indices {
        points[idx].set_pareto_state(1, true, true);
    }
}

/// Calculates 2D Pareto frontier and Andrew's Monotone Chain Upper Convex Hull on `ParetoCodecPoint` slice.
pub fn calculate_pareto_frontier(points: &mut [ParetoCodecPoint]) {
    compute_pareto_2d(points);
}

/// Calculates 2D Pareto frontier and Andrew's Monotone Chain Upper Convex Hull on raw C-ABI codec points in-place.
pub fn compute_codec_pareto_frontier_raw(points: &mut [TTZipParetoCodecPointRaw]) {
    compute_pareto_2d(points);
}

/// Computes multi-tier Pareto ranks ($O(N \log K)$ via Dilworth/Patience sorting)
/// and 2D Upper Convex Hull ($O(M \log M)$ via Andrew's Monotone Chain) on `ParetoPointRaw` in-place.
pub fn compute_pareto_frontier_raw(points: &mut [ParetoPointRaw]) {
    compute_pareto_2d(points);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pareto_empty_and_single() {
        let mut empty: Vec<ParetoPointRaw> = vec![];
        compute_pareto_frontier_raw(&mut empty);
        assert!(empty.is_empty());

        let mut single = vec![ParetoPointRaw::new(1, 100.0, 50.0)];
        compute_pareto_frontier_raw(&mut single);
        assert_eq!(single[0].pareto_rank, 1);
        assert!(single[0].is_pareto_optimal);
        assert!(single[0].is_on_convex_envelope);
    }

    #[test]
    fn test_pareto_dominated_points() {
        let mut points = vec![
            ParetoPointRaw::new(1, 100.0, 50.0), // Dominated by point 2 (200, 60)
            ParetoPointRaw::new(2, 200.0, 60.0), // Dominant
            ParetoPointRaw::new(3, 50.0, 80.0),  // Dominant (higher savings, lower speed)
            ParetoPointRaw::new(4, 30.0, 40.0),  // Dominated by all
        ];

        compute_pareto_frontier_raw(&mut points);

        let p2 = points.iter().find(|p| p.tag == 2).unwrap();
        let p3 = points.iter().find(|p| p.tag == 3).unwrap();
        let p1 = points.iter().find(|p| p.tag == 1).unwrap();
        let p4 = points.iter().find(|p| p.tag == 4).unwrap();

        assert_eq!(p2.pareto_rank, 1);
        assert!(p2.is_pareto_optimal);

        assert_eq!(p3.pareto_rank, 1);
        assert!(p3.is_pareto_optimal);

        assert!(p1.pareto_rank > 1);
        assert!(!p1.is_pareto_optimal);

        assert!(p4.pareto_rank > 1);
        assert!(!p4.is_pareto_optimal);
    }

    #[test]
    fn test_upper_convex_hull_monotone_chain() {
        let mut points = vec![
            ParetoPointRaw::new(1, 10.0, 90.0),  // extreme point A
            ParetoPointRaw::new(2, 50.0, 50.0),  // concave point below line AB -> not on hull
            ParetoPointRaw::new(3, 100.0, 10.0), // extreme point B
        ];

        compute_pareto_frontier_raw(&mut points);

        let p1 = points.iter().find(|p| p.tag == 1).unwrap();
        let p2 = points.iter().find(|p| p.tag == 2).unwrap();
        let p3 = points.iter().find(|p| p.tag == 3).unwrap();

        assert!(p1.is_pareto_optimal && p1.is_on_convex_envelope);
        assert!(p3.is_pareto_optimal && p3.is_on_convex_envelope);
        assert!(p2.is_pareto_optimal);
        assert!(!p2.is_on_convex_envelope); // Concave vertex correctly excluded from upper envelope
    }

    #[test]
    fn test_codec_pareto_frontier_calculation() {
        let mut points = vec![
            ParetoCodecPoint::new("Snappy", 0.55, 2500.0, 16.0),
            ParetoCodecPoint::new("Zstd L1", 0.65, 1200.0, 32.0),
            ParetoCodecPoint::new("Zstd L3", 0.70, 800.0, 64.0),
            ParetoCodecPoint::new("LZMA2 L9", 0.82, 50.0, 128.0),
            ParetoCodecPoint::new("SlowDeflate", 0.60, 200.0, 32.0), // Dominated by Zstd L1 & L3
        ];

        calculate_pareto_frontier(&mut points);

        let snappy = points.iter().find(|p| p.codec_name == "Snappy").unwrap();
        let zstd1 = points.iter().find(|p| p.codec_name == "Zstd L1").unwrap();
        let zstd3 = points.iter().find(|p| p.codec_name == "Zstd L3").unwrap();
        let lzma9 = points.iter().find(|p| p.codec_name == "LZMA2 L9").unwrap();
        let slow = points.iter().find(|p| p.codec_name == "SlowDeflate").unwrap();

        assert!(snappy.is_pareto_optimal);
        assert!(zstd1.is_pareto_optimal);
        assert!(zstd3.is_pareto_optimal);
        assert!(lzma9.is_pareto_optimal);

        assert!(!slow.is_pareto_optimal);
        assert!(slow.pareto_rank > 1);
        assert!(!slow.is_on_convex_hull);
    }

    #[test]
    fn test_codec_raw_ffi_monotone_chain() {
        let mut raw_points = [
            TTZipParetoCodecPointRaw::new("FastLZ", 0.50, 3000.0, 8.0),
            TTZipParetoCodecPointRaw::new("MidCodec", 0.60, 1000.0, 16.0),
            TTZipParetoCodecPointRaw::new("UltraCodec", 0.85, 20.0, 256.0),
        ];

        compute_codec_pareto_frontier_raw(&mut raw_points);

        for pt in &raw_points {
            assert!(pt.is_pareto_optimal);
            assert_eq!(pt.pareto_rank, 1);
        }

        let fast = raw_points.iter().find(|p| p.name_as_str() == "FastLZ").unwrap();
        let ultra = raw_points.iter().find(|p| p.name_as_str() == "UltraCodec").unwrap();
        assert!(fast.is_on_convex_hull);
        assert!(ultra.is_on_convex_hull);
    }
}
