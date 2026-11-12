// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Systematic Cauchy Reed-Solomon Erasure Coding and Matrix Inversion over $\text{GF}(2^8)$.

use super::gf8::{gf_inv, gf_mul, gf8_mul_acc_slice};
use rayon::prelude::*;
use ttzip_types::TTZipStatus;

const PARALLEL_PARITY_THRESHOLD_BYTES: usize = 65536;

/// Constructs an $M \times K$ Cauchy generator matrix.
///
/// Element $C_{i, j} = \frac{1}{i \oplus (M + j)}$.
pub fn create_cauchy_matrix(rows_m: usize, cols_k: usize) -> Vec<u8> {
    let mut matrix = vec![0u8; rows_m * cols_k];
    for i in 0..rows_m {
        for j in 0..cols_k {
            let diff = (i as u8) ^ ((rows_m + j) as u8);
            matrix[i * cols_k + j] = gf_inv(diff);
        }
    }
    matrix
}

fn eliminate_pivot_column(aug: &mut [u8], n: usize, col: usize) -> Result<(), TTZipStatus> {
    let width = n * 2;
    let pivot_row = (col..n)
        .find(|&r| aug[r * width + col] != 0)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;

    if pivot_row != col {
        for k in 0..width {
            aug.swap(col * width + k, pivot_row * width + k);
        }
    }

    let pivot_inv = gf_inv(aug[col * width + col]);
    for k in 0..width {
        aug[col * width + k] = gf_mul(aug[col * width + k], pivot_inv);
    }

    for r in 0..n {
        let factor = aug[r * width + col];
        if r == col || factor == 0 {
            continue;
        }
        for k in 0..width {
            aug[r * width + k] ^= gf_mul(aug[col * width + k], factor);
        }
    }
    Ok(())
}

/// Inverts an $N \times N$ matrix over $\text{GF}(2^8)$ using Gaussian elimination.
pub fn invert_matrix(matrix: &[u8], n: usize) -> Result<Vec<u8>, TTZipStatus> {
    if matrix.len() != n * n || n == 0 {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let width = n * 2;
    let mut aug = vec![0u8; n * width];
    for i in 0..n {
        aug[i * width..i * width + n].copy_from_slice(&matrix[i * n..(i + 1) * n]);
        aug[i * width + n + i] = 1;
    }

    for col in 0..n {
        eliminate_pivot_column(&mut aug, n, col)?;
    }

    let mut inv = vec![0u8; n * n];
    for i in 0..n {
        inv[i * n..(i + 1) * n].copy_from_slice(&aug[i * width + n..i * width + width]);
    }
    Ok(inv)
}

/// High-performance Systematic Cauchy Reed-Solomon Codec.
#[derive(Debug, Clone)]
pub struct ReedSolomonEngine {
    pub k: usize,
    pub m: usize,
    pub cauchy_matrix: Vec<u8>,
}

impl ReedSolomonEngine {
    /// Creates a new Reed-Solomon engine for $K$ data shards and $M$ parity shards.
    pub fn new(k: usize, m: usize) -> Result<Self, TTZipStatus> {
        if k == 0 || m == 0 || k + m > 256 {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let cauchy_matrix = create_cauchy_matrix(m, k);
        Ok(Self {
            k,
            m,
            cauchy_matrix,
        })
    }

    /// Encodes $K$ data shards into $M$ parity shards in-place, parallelizing across parity
    /// rows via Rayon when `m * shard_len >= 65536`.
    pub fn encode_in_place(
        &self,
        data_shards: &[&[u8]],
        parity_shards: &mut [&mut [u8]],
    ) -> Result<(), TTZipStatus> {
        if data_shards.len() != self.k || parity_shards.len() != self.m {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let shard_len = data_shards[0].len();
        if shard_len == 0
            || data_shards.iter().any(|s| s.len() != shard_len)
            || parity_shards.iter().any(|s| s.len() != shard_len)
        {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        if self.m * shard_len >= PARALLEL_PARITY_THRESHOLD_BYTES {
            parity_shards
                .par_iter_mut()
                .enumerate()
                .for_each(|(p, p_slice)| {
                    p_slice.fill(0);
                    let row = &self.cauchy_matrix[p * self.k..(p + 1) * self.k];
                    for (d, d_slice) in data_shards.iter().enumerate() {
                        let coeff = row[d];
                        if coeff != 0 {
                            gf8_mul_acc_slice(coeff, d_slice, p_slice);
                        }
                    }
                });
        } else {
            for (p, p_slice) in parity_shards.iter_mut().enumerate() {
                p_slice.fill(0);
                let row = &self.cauchy_matrix[p * self.k..(p + 1) * self.k];
                for (d, d_slice) in data_shards.iter().enumerate() {
                    let coeff = row[d];
                    if coeff != 0 {
                        gf8_mul_acc_slice(coeff, d_slice, p_slice);
                    }
                }
            }
        }
        Ok(())
    }

    /// Encodes $K$ data shards into $M$ parity shards.
    #[inline]
    pub fn encode(
        &self,
        data_shards: &[&[u8]],
        parity_shards: &mut [&mut [u8]],
    ) -> Result<(), TTZipStatus> {
        self.encode_in_place(data_shards, parity_shards)
    }

    fn build_decode_submatrix(&self, available_indices: &[usize]) -> Result<Vec<u8>, TTZipStatus> {
        let k = self.k;
        let mut submatrix = vec![0u8; k * k];
        for (r, &idx) in available_indices.iter().take(k).enumerate() {
            if idx < k {
                submatrix[r * k + idx] = 1;
            } else {
                let parity_row = idx - k;
                if parity_row >= self.m {
                    return Err(TTZipStatus::ErrInvalidParam);
                }
                submatrix[r * k..(r + 1) * k]
                    .copy_from_slice(&self.cauchy_matrix[parity_row * k..(parity_row + 1) * k]);
            }
        }
        Ok(submatrix)
    }

    /// Reconstructs missing data shards from any $K$ available shards (data or parity).
    pub fn decode(
        &self,
        available_shards: &[&[u8]],
        available_indices: &[usize],
        missing_indices: &[usize],
        reconstructed: &mut [&mut [u8]],
    ) -> Result<(), TTZipStatus> {
        if available_shards.len() < self.k
            || available_indices.len() != available_shards.len()
            || missing_indices.len() != reconstructed.len()
            || missing_indices.is_empty()
        {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let block_size = available_shards[0].len();
        if block_size == 0 {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let submatrix = self.build_decode_submatrix(available_indices)?;
        let inv_matrix = invert_matrix(&submatrix, self.k)?;

        for (m_idx, &missing_col) in missing_indices.iter().enumerate() {
            if missing_col >= self.k {
                continue;
            }
            let dst = &mut reconstructed[m_idx];
            if dst.len() != block_size {
                return Err(TTZipStatus::ErrInvalidParam);
            }
            dst.fill(0);

            for (r, shard) in available_shards.iter().take(self.k).enumerate() {
                let coeff = inv_matrix[missing_col * self.k + r];
                if coeff != 0 {
                    gf8_mul_acc_slice(coeff, shard, dst);
                }
            }
        }

        Ok(())
    }
}
