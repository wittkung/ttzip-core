// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Pure Safe Rust `bsdiff` differential generator.
//!
//! Generates ternary control stream triplets `(diff_len, extra_len, seek_offset)`,
//! additive diff streams, and literal extra streams with memory-bounded suffix matching.

use crate::system::delta::types::DeltaResult;
use serde::{Deserialize, Serialize};

/// Control triplet instruction for binary differential patch generation and application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BsDiffControl {
    /// Number of bytes to read from the additive diff stream.
    pub diff_len: usize,
    /// Number of literal bytes to read from the extra stream.
    pub extra_len: usize,
    /// Signed seek displacement in old data relative to the end of the previous diff block.
    pub seek_offset: i64,
}

impl BsDiffControl {
    /// Creates a new control triplet.
    #[inline]
    pub const fn new(diff_len: usize, extra_len: usize, seek_offset: i64) -> Self {
        Self {
            diff_len,
            extra_len,
            seek_offset,
        }
    }
}

/// Raw decompressed bsdiff stream components.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BsDiffPatch {
    /// Sequence of control triplet operations.
    pub controls: Vec<BsDiffControl>,
    /// Additive byte differences: `(new_byte - old_byte) mod 256`.
    pub diff_data: Vec<u8>,
    /// Literal byte insertions.
    pub extra_data: Vec<u8>,
}

impl BsDiffPatch {
    /// Creates a new empty `BsDiffPatch`.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a triplet along with corresponding diff and extra slices.
    pub fn push_triplet(
        &mut self,
        ctrl: BsDiffControl,
        diff_slice: &[u8],
        extra_slice: &[u8],
    ) {
        self.controls.push(ctrl);
        self.diff_data.extend_from_slice(diff_slice);
        self.extra_data.extend_from_slice(extra_slice);
    }
}

#[derive(Clone, Copy, Default)]
struct BsdiffWindowState {
    scan: usize,
    pos: usize,
    len: usize,
    lastscan: usize,
    lastpos: usize,
}

/// Pure Safe Rust high-throughput bsdiff generator.
pub struct TTZipBsDiff;

impl TTZipBsDiff {
    /// Generates a binary delta patch between `old_data` and `new_data`.
    pub fn diff(old_data: &[u8], new_data: &[u8]) -> DeltaResult<BsDiffPatch> {
        let mut patch = BsDiffPatch::new();
        if new_data.is_empty() {
            return Ok(patch);
        }
        if old_data.is_empty() {
            patch.push_triplet(BsDiffControl::new(0, new_data.len(), 0), &[], new_data);
            return Ok(patch);
        }

        let sa = build_suffix_array(old_data);
        let mut win = BsdiffWindowState::default();

        while win.scan < new_data.len() {
            let oldscore = advance_scan_window(&sa, old_data, new_data, &mut win);

            if win.len != oldscore || win.scan >= new_data.len() {
                let mut lenf = compute_forward_boundary(
                    old_data,
                    new_data,
                    win.scan,
                    win.lastscan,
                    win.lastpos,
                );
                let mut lenb = if win.scan < new_data.len() {
                    compute_backward_boundary(old_data, new_data, win.scan, win.pos)
                } else {
                    0
                };

                if win.lastscan + lenf > win.scan.saturating_sub(lenb) {
                    (lenf, lenb) = resolve_boundary_overlap(old_data, new_data, &win, lenf, lenb);
                }

                emit_patch_triplet(&mut patch, old_data, new_data, &win, lenf, lenb);
                win.lastscan = win.scan.saturating_sub(lenb);
                win.lastpos = win.pos.saturating_sub(lenb);
            }
        }

        if win.lastscan < new_data.len() {
            let trailing_extra = &new_data[win.lastscan..];
            patch.push_triplet(
                BsDiffControl::new(0, trailing_extra.len(), 0),
                &[],
                trailing_extra,
            );
        }

        Ok(patch)
    }
}

fn advance_scan_window(
    sa: &[usize],
    old_data: &[u8],
    new_data: &[u8],
    win: &mut BsdiffWindowState,
) -> usize {
    let mut oldscore = 0usize;
    win.scan = (win.scan + win.len).min(new_data.len());
    let mut scsc = win.scan;

    while win.scan < new_data.len() {
        let match_result = search_longest_match(sa, old_data, &new_data[win.scan..]);
        win.pos = match_result.0;
        win.len = match_result.1;

        while scsc < win.scan + win.len && scsc < new_data.len() {
            if is_matching_at_offset(old_data, new_data, scsc, win.lastscan, win.lastpos) {
                oldscore += 1;
            }
            scsc += 1;
        }

        if (win.len == oldscore && win.len != 0) || win.len > oldscore + 8 {
            break;
        }

        if is_matching_at_offset(old_data, new_data, win.scan, win.lastscan, win.lastpos) {
            oldscore = oldscore.saturating_sub(1);
        }

        win.scan += 1;
    }

    win.scan = win.scan.min(new_data.len());
    oldscore
}

#[inline]
fn is_matching_at_offset(
    old_data: &[u8],
    new_data: &[u8],
    idx: usize,
    lastscan: usize,
    lastpos: usize,
) -> bool {
    if idx < lastscan || idx >= new_data.len() {
        return false;
    }
    let old_idx = lastpos + (idx - lastscan);
    old_idx < old_data.len() && old_data[old_idx] == new_data[idx]
}

fn compute_forward_boundary(
    old_data: &[u8],
    new_data: &[u8],
    scan: usize,
    lastscan: usize,
    lastpos: usize,
) -> usize {
    let mut s = 0i64;
    let mut sf = 0usize;
    let mut lenf = 0usize;
    let mut i = 0usize;

    while lastscan + i < scan && lastpos + i < old_data.len() && lastscan + i < new_data.len() {
        if old_data[lastpos + i] == new_data[lastscan + i] {
            s += 1;
        }
        i += 1;
        if s * 2 - (i as i64) > (sf as i64) * 2 - (lenf as i64) {
            sf = s as usize;
            lenf = i;
        }
    }
    lenf
}

fn compute_backward_boundary(old_data: &[u8], new_data: &[u8], scan: usize, pos: usize) -> usize {
    let mut sb = 0i64;
    let mut sc = 0usize;
    let mut lenb = 0usize;
    let mut j = 1usize;

    while scan >= j && pos >= j {
        if old_data[pos - j] == new_data[scan - j] {
            sb += 1;
        }
        if sb * 2 - (j as i64) > (sc as i64) * 2 - (lenb as i64) {
            sc = sb as usize;
            lenb = j;
        }
        j += 1;
    }
    lenb
}

fn resolve_boundary_overlap(
    old_data: &[u8],
    new_data: &[u8],
    win: &BsdiffWindowState,
    lenf: usize,
    lenb: usize,
) -> (usize, usize) {
    let overlap = (win.lastscan + lenf) - (win.scan - lenb);
    let mut s = 0i64;
    let mut ss = 0i64;
    let mut lens = 0usize;

    for k in 0..overlap {
        let f_dist = overlap - k;
        if win.lastscan + lenf > f_dist
            && win.lastpos + lenf > f_dist
            && new_data[win.lastscan + lenf - f_dist] == old_data[win.lastpos + lenf - f_dist]
        {
            s += 1;
        }
        let b_dist = lenb - k;
        if win.pos >= b_dist
            && win.scan >= b_dist
            && new_data[win.scan - b_dist] == old_data[win.pos - b_dist]
        {
            s -= 1;
        }
        if s > ss {
            ss = s;
            lens = k + 1;
        }
    }

    (lenf.saturating_sub(overlap - lens), lenb.saturating_sub(lens))
}

fn emit_patch_triplet(
    patch: &mut BsDiffPatch,
    old_data: &[u8],
    new_data: &[u8],
    win: &BsdiffWindowState,
    lenf: usize,
    lenb: usize,
) {
    let target_boundary = win.scan.saturating_sub(lenb);
    let extra_len = target_boundary.saturating_sub(win.lastscan + lenf);
    let seek_offset = (win.pos as i64 - lenb as i64) - (win.lastpos as i64 + lenf as i64);

    let mut diff_chunk = Vec::with_capacity(lenf);
    for idx in 0..lenf {
        let old_byte = old_data[win.lastpos + idx];
        let new_byte = new_data[win.lastscan + idx];
        diff_chunk.push(new_byte.wrapping_sub(old_byte));
    }

    let extra_start = win.lastscan + lenf;
    let extra_end = extra_start + extra_len;
    let extra_chunk = if extra_start <= new_data.len() && extra_end <= new_data.len() {
        &new_data[extra_start..extra_end]
    } else {
        &[]
    };

    patch.push_triplet(
        BsDiffControl::new(lenf, extra_len, seek_offset),
        &diff_chunk,
        extra_chunk,
    );
}

/// Constructs a suffix array for the given data slice using pure Safe Rust prefix doubling.
pub fn build_suffix_array(data: &[u8]) -> Vec<usize> {
    let n = data.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![0];
    }

    let mut sa: Vec<usize> = (0..n).collect();
    let mut rank: Vec<i32> = data.iter().map(|&b| b as i32).collect();
    let mut tmp_rank: Vec<i32> = vec![0; n];

    let mut k = 1usize;
    while k < n {
        sa.sort_unstable_by(|&a, &b| {
            let r_a1 = rank[a];
            let r_b1 = rank[b];
            if r_a1 != r_b1 {
                return r_a1.cmp(&r_b1);
            }
            let r_a2 = if a + k < n { rank[a + k] } else { -1 };
            let r_b2 = if b + k < n { rank[b + k] } else { -1 };
            r_a2.cmp(&r_b2)
        });

        tmp_rank[sa[0]] = 0;
        for i in 1..n {
            let prev = sa[i - 1];
            let curr = sa[i];
            let prev_k = if prev + k < n { rank[prev + k] } else { -1 };
            let curr_k = if curr + k < n { rank[curr + k] } else { -1 };

            let is_same = rank[prev] == rank[curr] && prev_k == curr_k;
            tmp_rank[curr] = tmp_rank[prev] + if is_same { 0 } else { 1 };
        }

        rank.copy_from_slice(&tmp_rank);
        if rank[sa[n - 1]] as usize == n - 1 {
            break;
        }
        k <<= 1;
    }

    sa
}

/// Binary searches suffix array for the longest common prefix match with the query.
/// Returns `(best_old_pos, best_match_len)`.
pub fn search_longest_match(
    sa: &[usize],
    old_data: &[u8],
    query: &[u8],
) -> (usize, usize) {
    if sa.is_empty() || query.is_empty() {
        return (0, 0);
    }

    let mut low = 0usize;
    let mut high = sa.len();
    let mut best_pos = sa[0];
    let mut best_len = 0usize;

    while low < high {
        let mid = low + (high - low) / 2;
        let suffix_idx = sa[mid];
        let suffix = &old_data[suffix_idx..];

        let match_len = common_prefix_len(suffix, query);
        if match_len > best_len {
            best_len = match_len;
            best_pos = suffix_idx;
        }

        if match_len == query.len() || suffix_idx + match_len >= old_data.len() {
            if suffix < query {
                low = mid + 1;
            } else {
                high = mid;
            }
        } else if suffix[match_len] < query[match_len] {
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    (best_pos, best_len)
}

#[inline(always)]
fn common_prefix_len(a: &[u8], b: &[u8]) -> usize {
    let max_len = a.len().min(b.len());
    let mut i = 0;
    while i < max_len && a[i] == b[i] {
        i += 1;
    }
    i
}
