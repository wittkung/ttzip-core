// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

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
            // Entire new_data is an extra literal insertion.
            patch.push_triplet(
                BsDiffControl::new(0, new_data.len(), 0),
                &[],
                new_data,
            );
            return Ok(patch);
        }

        let sa = build_suffix_array(old_data);

        let mut scan = 0usize;
        let mut pos = 0usize;
        let mut len = 0usize;
        let mut lastscan = 0usize;
        let mut lastpos = 0usize;

        while scan < new_data.len() {
            let mut oldscore = 0usize;
            scan += len;
            if scan > new_data.len() {
                scan = new_data.len();
            }
            let mut scsc = scan;

            while scan < new_data.len() {
                let match_result = search_longest_match(&sa, old_data, &new_data[scan..]);
                pos = match_result.0;
                len = match_result.1;

                while scsc < scan + len && scsc < new_data.len() {
                    if scsc >= lastscan {
                        let old_idx = lastpos + (scsc - lastscan);
                        if old_idx < old_data.len() {
                            if old_data[old_idx] == new_data[scsc] {
                                oldscore += 1;
                            }
                        }
                    }
                    scsc += 1;
                }

                if (len == oldscore && len != 0) || len > oldscore + 8 {
                    break;
                }

                if scan >= lastscan {
                    let old_idx = lastpos + (scan - lastscan);
                    if old_idx < old_data.len() && scan < new_data.len() {
                        if old_data[old_idx] == new_data[scan] {
                            oldscore = oldscore.saturating_sub(1);
                        }
                    }
                }

                scan += 1;
            }

            if scan > new_data.len() {
                scan = new_data.len();
            }

            if len != oldscore || scan >= new_data.len() {
                let mut s = 0i64;
                let mut sf = 0usize;
                let mut lenf = 0usize;

                // Find forward matching boundary from lastscan
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

                let mut lenb = 0usize;
                if scan < new_data.len() {
                    let mut sb = 0i64;
                    let mut sc = 0usize;
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
                }

                if lastscan + lenf > scan.saturating_sub(lenb) {
                    let overlap = (lastscan + lenf) - (scan - lenb);
                    let mut s = 0i64;
                    let mut ss = 0i64;
                    let mut lens = 0usize;

                    for k in 0..overlap {
                        if (lastscan + lenf > overlap - k)
                            && (lastpos + lenf > overlap - k)
                            && (new_data[lastscan + lenf - overlap + k]
                                == old_data[lastpos + lenf - overlap + k])
                        {
                            s += 1;
                        }
                        if (pos >= lenb - k)
                            && (scan >= lenb - k)
                            && (new_data[scan - lenb + k] == old_data[pos - lenb + k])
                        {
                            s -= 1;
                        }
                        if s > ss {
                            ss = s;
                            lens = k + 1;
                        }
                    }

                    lenf = lenf.saturating_sub(overlap - lens);
                    lenb = lenb.saturating_sub(lens);
                }

                let target_boundary = scan.saturating_sub(lenb);
                let extra_len = target_boundary.saturating_sub(lastscan + lenf);
                let seek_offset = (pos as i64 - lenb as i64) - (lastpos as i64 + lenf as i64);

                let mut diff_chunk = Vec::with_capacity(lenf);
                for idx in 0..lenf {
                    let old_byte = old_data[lastpos + idx];
                    let new_byte = new_data[lastscan + idx];
                    diff_chunk.push(new_byte.wrapping_sub(old_byte));
                }

                let extra_start = lastscan + lenf;
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

                lastscan = scan.saturating_sub(lenb);
                lastpos = pos.saturating_sub(lenb);
            }
        }

        if lastscan < new_data.len() {
            let trailing_extra = &new_data[lastscan..];
            patch.push_triplet(
                BsDiffControl::new(0, trailing_extra.len(), 0),
                &[],
                trailing_extra,
            );
        }

        Ok(patch)
    }
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
