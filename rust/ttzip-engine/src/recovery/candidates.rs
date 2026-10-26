// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Candidate source for the engine's brute-force password search.

/// A lazy source of brute-force password candidates.
///
/// The candidates are produced in the same order the crypto crate's fused brute
/// force uses: every length in the requested range, and within a length every
/// combination of the charset in lexicographic order. The engine keeps its own
/// source because the search orchestration needs the candidates as a stream it can
/// batch, confirm and resume, which a fused "find the first hit" helper cannot
/// provide.
pub struct BruteForceCandidates {
    /// The characters the candidates are built from.
    chars: Vec<char>,
    /// The shortest candidate length.
    min_len: usize,
    /// The longest candidate length.
    max_len: usize,
    /// The length of the candidates currently produced.
    len: usize,
    /// The mixed-radix counter over the current length.
    index: Vec<usize>,
    /// Whether the first batch has been produced yet.
    initialised: bool,
    /// Whether the source ran out of candidates.
    exhausted: bool,
}

impl BruteForceCandidates {
    /// Creates a source for the given charset and length range.
    ///
    /// Returns `None` when the charset is empty or the length range is empty.
    pub fn new(charset: &str, min_len: usize, max_len: usize) -> Option<Self> {
        let chars: Vec<char> = charset.chars().collect();
        if chars.is_empty() || min_len == 0 || max_len < min_len {
            return None;
        }
        Some(Self {
            chars,
            min_len,
            max_len,
            len: min_len,
            index: Vec::new(),
            initialised: false,
            exhausted: false,
        })
    }

    /// Produces the next candidate, or `None` when the source is exhausted.
    pub fn next_candidate(&mut self) -> Option<String> {
        if self.exhausted {
            return None;
        }
        if !self.initialised {
            self.initialised = true;
            self.len = self.min_len;
            self.index = vec![0; self.len];
        }

        let candidate: String = self.index.iter().map(|&i| self.chars[i]).collect();
        self.advance();
        Some(candidate)
    }

    /// Fills the buffer with the next batch of candidates.
    ///
    /// Returns `false` when the source is exhausted and the buffer was left empty.
    pub fn fill_batch(&mut self, buffer: &mut Vec<String>, batch_size: usize) -> bool {
        buffer.clear();
        if batch_size == 0 {
            return false;
        }
        while buffer.len() < batch_size {
            match self.next_candidate() {
                Some(candidate) => buffer.push(candidate),
                None => break,
            }
        }
        !buffer.is_empty()
    }

    /// Advances the mixed-radix counter, rolling over to the next length.
    fn advance(&mut self) {
        let mut overflow = true;
        for pos in (0..self.index.len()).rev() {
            self.index[pos] += 1;
            if self.index[pos] < self.chars.len() {
                overflow = false;
                break;
            }
            self.index[pos] = 0;
        }
        if !overflow {
            return;
        }
        self.len += 1;
        if self.len > self.max_len {
            self.exhausted = true;
        } else {
            self.index = vec![0; self.len];
        }
    }
}
