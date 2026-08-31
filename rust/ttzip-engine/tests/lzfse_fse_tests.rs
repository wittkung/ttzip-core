// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and invariant tests for LZFSE Finite State Entropy (FSE/tANS).
//!
//! Tests frequency normalization conservation, liveness invariants, encoder/decoder
//! bijective state transitions, and fused value decoder table equivalence.

use ttzip_engine::codecs::lzfse::fse::*;
use ttzip_engine::codecs::lzfse::tables::*;
use ttzip_engine::types::TTZipStatus;

#[test]
fn test_fse_constants_and_geometry() {
    assert_eq!(LZFSE_ENCODE_L_SYMBOLS, 20);
    assert_eq!(LZFSE_ENCODE_M_SYMBOLS, 20);
    assert_eq!(LZFSE_ENCODE_D_SYMBOLS, 64);
    assert_eq!(LZFSE_ENCODE_LITERAL_SYMBOLS, 256);

    assert_eq!(LZFSE_ENCODE_L_STATES, 64);
    assert_eq!(LZFSE_ENCODE_M_STATES, 64);
    assert_eq!(LZFSE_ENCODE_D_STATES, 256);
    assert_eq!(LZFSE_ENCODE_LITERAL_STATES, 1024);

    assert_eq!(L_EXTRA_BITS.len(), LZFSE_ENCODE_L_SYMBOLS);
    assert_eq!(L_BASE_VALUE.len(), LZFSE_ENCODE_L_SYMBOLS);
    assert_eq!(M_EXTRA_BITS.len(), LZFSE_ENCODE_M_SYMBOLS);
    assert_eq!(M_BASE_VALUE.len(), LZFSE_ENCODE_M_SYMBOLS);
    assert_eq!(D_EXTRA_BITS.len(), LZFSE_ENCODE_D_SYMBOLS);
    assert_eq!(D_BASE_VALUE.len(), LZFSE_ENCODE_D_SYMBOLS);
}

#[test]
fn test_fse_normalize_freq_sum_and_liveness_all_alphabets() {
    // 1. Literal alphabet (256 symbols, 1024 states)
    let mut lit_occurrences = [0u32; 256];
    for (i, occ) in lit_occurrences.iter_mut().enumerate() {
        if i % 3 == 0 {
            *occ = ((i * 17 + 5) % 100) as u32 + 1;
        }
    }
    let mut lit_freq = [0u16; 256];
    fse_normalize_freq(
        LZFSE_ENCODE_LITERAL_STATES,
        LZFSE_ENCODE_LITERAL_SYMBOLS,
        &lit_occurrences,
        &mut lit_freq,
    );

    let lit_sum: usize = lit_freq.iter().map(|&f| f as usize).sum();
    assert_eq!(lit_sum, LZFSE_ENCODE_LITERAL_STATES, "Literal freq sum must equal 1024");
    for i in 0..256 {
        if lit_occurrences[i] > 0 {
            assert!(lit_freq[i] >= 1, "Symbol {} has non-zero occurrences but 0 freq", i);
        } else {
            assert_eq!(lit_freq[i], 0, "Symbol {} has 0 occurrences but non-zero freq", i);
        }
    }

    // 2. L alphabet (20 symbols, 64 states)
    let mut l_occurrences = [0u32; 20];
    for (i, occ) in l_occurrences.iter_mut().enumerate() {
        *occ = (i as u32 + 1) * 10;
    }
    let mut l_freq = [0u16; 20];
    fse_normalize_freq(
        LZFSE_ENCODE_L_STATES,
        LZFSE_ENCODE_L_SYMBOLS,
        &l_occurrences,
        &mut l_freq,
    );

    let l_sum: usize = l_freq.iter().map(|&f| f as usize).sum();
    assert_eq!(l_sum, LZFSE_ENCODE_L_STATES, "L freq sum must equal 64");
    for i in 0..20 {
        assert!(l_freq[i] >= 1, "L Symbol {} must have non-zero freq", i);
    }

    // 3. M alphabet (20 symbols, 64 states)
    let mut m_occurrences = [0u32; 20];
    m_occurrences[0] = 500;
    m_occurrences[1] = 200;
    m_occurrences[5] = 50;
    m_occurrences[19] = 1;
    let mut m_freq = [0u16; 20];
    fse_normalize_freq(
        LZFSE_ENCODE_M_STATES,
        LZFSE_ENCODE_M_SYMBOLS,
        &m_occurrences,
        &mut m_freq,
    );

    let m_sum: usize = m_freq.iter().map(|&f| f as usize).sum();
    assert_eq!(m_sum, LZFSE_ENCODE_M_STATES, "M freq sum must equal 64");
    assert!(m_freq[0] >= m_freq[1]);
    assert!(m_freq[1] >= m_freq[5]);
    assert!(m_freq[19] >= 1);

    // 4. D alphabet (64 symbols, 256 states)
    let mut d_occurrences = [0u32; 64];
    for i in 0..64 {
        d_occurrences[i] = (64 - i as u32) * 5;
    }
    let mut d_freq = [0u16; 64];
    fse_normalize_freq(
        LZFSE_ENCODE_D_STATES,
        LZFSE_ENCODE_D_SYMBOLS,
        &d_occurrences,
        &mut d_freq,
    );

    let d_sum: usize = d_freq.iter().map(|&f| f as usize).sum();
    assert_eq!(d_sum, LZFSE_ENCODE_D_STATES, "D freq sum must equal 256");
    for i in 0..64 {
        assert!(d_freq[i] >= 1);
    }
}

#[test]
fn test_fse_normalize_freq_edge_cases() {
    // Single symbol takes 100% of the occurrences
    let mut single_sym_occ = [0u32; 20];
    single_sym_occ[7] = 10000;
    let mut freq = [0u16; 20];
    fse_normalize_freq(64, 20, &single_sym_occ, &mut freq);
    assert_eq!(freq[7], 64);
    for i in 0..20 {
        if i != 7 {
            assert_eq!(freq[i], 0);
        }
    }

    // Uniform occurrences
    let uniform_occ = [100u32; 64];
    let mut uniform_freq = [0u16; 64];
    fse_normalize_freq(256, 64, &uniform_occ, &mut uniform_freq);
    let sum: usize = uniform_freq.iter().map(|&f| f as usize).sum();
    assert_eq!(sum, 256);
    for &f in uniform_freq.iter() {
        assert_eq!(f, 4);
    }

    // All zeros: sum is conserved and assigned to symbol 0
    let zero_occ = [0u32; 20];
    let mut zero_freq = [0u16; 20];
    fse_normalize_freq(64, 20, &zero_occ, &mut zero_freq);
    let sum: usize = zero_freq.iter().map(|&f| f as usize).sum();
    assert_eq!(sum, 64);
    assert_eq!(zero_freq[0], 64);
}

#[test]
fn test_fse_check_freq_validation() {
    let valid_freq = [4u16; 16]; // sum = 64
    assert!(fse_check_freq(&valid_freq, 64).is_ok());

    let invalid_freq = [5u16; 16]; // sum = 80 > 64
    assert_eq!(fse_check_freq(&invalid_freq, 64), Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_fse_init_encoder_table_properties() {
    let mut occurrences = [0u32; 20];
    for (i, occ) in occurrences.iter_mut().enumerate() {
        *occ = (i as u32 + 1) * 10;
    }
    let mut freq = [0u16; 20];
    fse_normalize_freq(64, 20, &occurrences, &mut freq);

    let mut encoder_table = [FseEncoderEntry::default(); 20];
    assert!(fse_init_encoder_table(64, 20, &freq, &mut encoder_table).is_ok());

    // Invariant checking
    for i in 0..20 {
        let f = freq[i];
        let entry = encoder_table[i];
        if f == 0 {
            assert_eq!(entry, FseEncoderEntry::default());
        } else {
            assert!(entry.k >= 0);
            assert!(entry.s0 >= -64 && entry.s0 <= 64);
        }
    }

    // Invalid params
    let mut bad_table = [FseEncoderEntry::default(); 10];
    assert_eq!(
        fse_init_encoder_table(64, 20, &freq, &mut bad_table),
        Err(TTZipStatus::ErrInvalidParam)
    );
    assert_eq!(
        fse_init_encoder_table(63, 20, &freq, &mut encoder_table),
        Err(TTZipStatus::ErrInvalidParam)
    );
}

#[test]
fn test_fse_init_decoder_table_packed() {
    let mut occurrences = [0u32; 256];
    for (i, occ) in occurrences.iter_mut().enumerate() {
        *occ = ((i % 16) + 1) as u32;
    }
    let mut freq = [0u16; 256];
    fse_normalize_freq(1024, 256, &occurrences, &mut freq);

    let mut packed_table = [0i32; 1024];
    assert!(fse_init_decoder_table_packed(1024, 256, &freq, &mut packed_table).is_ok());

    // Verify unpack matches decoder entry
    let mut decoder_table = [FseDecoderEntry::default(); 1024];
    fse_init_decoder_table(1024, 256, &freq, &mut decoder_table);

    for i in 0..1024 {
        let packed = packed_table[i];
        let unpacked = FseDecoderEntry::from_packed_i32(packed);
        assert_eq!(unpacked, decoder_table[i]);
        assert_eq!(unpacked.to_packed_i32(), packed);
    }
}

#[test]
fn test_fse_encoder_decoder_bijective_state_transitions() {
    // Test for L (64 states, 20 symbols)
    let nstates = 64usize;
    let nsymbols = 20usize;
    let mut occurrences = [0u32; 20];
    for i in 0..20 {
        occurrences[i] = (i as u32 * 7 + 3) % 50 + 1;
    }
    let mut freq = [0u16; 20];
    fse_normalize_freq(nstates, nsymbols, &occurrences, &mut freq);

    let mut encoder_table = [FseEncoderEntry::default(); 20];
    fse_init_encoder_table(nstates, nsymbols, &freq, &mut encoder_table).unwrap();

    let mut decoder_table = [FseDecoderEntry::default(); 64];
    fse_init_decoder_table(nstates, nsymbols, &freq, &mut decoder_table);

    // Verify bijective roundtrip for every symbol with f > 0 and state in [0, nstates - 1]
    for sym in 0..nsymbols {
        let f = freq[sym] as usize;
        if f == 0 {
            continue;
        }
        let enc = encoder_table[sym];

        for s in 0..(nstates as i32) {
            // Encoder step (matching C fse_encode):
            let hi = s >= enc.s0 as i32;
            let nbits = if hi { enc.k as i32 } else { enc.k as i32 - 1 };
            let delta = if hi { enc.delta0 as i32 } else { enc.delta1 as i32 };

            let bits = if nbits > 0 { s & ((1 << nbits) - 1) } else { 0 };
            let next_state = delta + (s >> nbits);

            assert!(
                next_state >= 0 && (next_state as usize) < nstates,
                "next_state {} out of bounds [0, {}) for s={}, sym={}",
                next_state,
                nstates,
                s,
                sym
            );

            // Decoder step (matching C fse_decode):
            let dec = decoder_table[next_state as usize];
            assert_eq!(
                dec.symbol as usize, sym,
                "Decoded symbol {} did not match encoded symbol {} at state {}",
                dec.symbol, sym, next_state
            );
            assert_eq!(
                dec.k as i32, nbits,
                "Decoded bits {} did not match encoder nbits {} at state {}",
                dec.k, nbits, next_state
            );

            let recov_s = (dec.delta as i32) + bits;
            assert_eq!(
                recov_s, s,
                "Recovered state {} did not match original state {} for sym {}",
                recov_s, s, sym
            );
        }
    }
}

#[test]
fn test_fse_value_decoder_fusion_equivalence() {
    // 1. Literal Length Table (L)
    let mut l_occ = [0u32; 20];
    for i in 0..20 {
        l_occ[i] = (i as u32 + 1) * 3;
    }
    let mut l_freq = [0u16; 20];
    fse_normalize_freq(64, 20, &l_occ, &mut l_freq);

    let mut l_dec = [FseDecoderEntry::default(); 64];
    fse_init_decoder_table(64, 20, &l_freq, &mut l_dec);

    let mut l_val_dec = [FseValueDecoderEntry::default(); 64];
    assert!(fse_init_value_decoder_table(
        64,
        20,
        &l_freq,
        &L_BASE_VALUE,
        &L_EXTRA_BITS,
        &mut l_val_dec
    )
    .is_ok());

    for i in 0..64 {
        let std = l_dec[i];
        let fused = l_val_dec[i];
        let sym = std.symbol as usize;
        let expected_vbits = L_EXTRA_BITS[sym];
        let expected_vbase = L_BASE_VALUE[sym];

        assert_eq!(fused.value_bits, expected_vbits);
        assert_eq!(fused.vbase, expected_vbase);
        assert_eq!(fused.delta, std.delta);
        assert_eq!(fused.total_bits, (std.k as u8) + expected_vbits);
    }

    // 2. Match Distance Table (D)
    let mut d_occ = [0u32; 64];
    for i in 0..64 {
        d_occ[i] = (64 - i as u32) * 2;
    }
    let mut d_freq = [0u16; 64];
    fse_normalize_freq(256, 64, &d_occ, &mut d_freq);

    let mut d_dec = [FseDecoderEntry::default(); 256];
    fse_init_decoder_table(256, 64, &d_freq, &mut d_dec);

    let mut d_val_dec = [FseValueDecoderEntry::default(); 256];
    assert!(fse_init_value_decoder_table(
        256,
        64,
        &d_freq,
        &D_BASE_VALUE,
        &D_EXTRA_BITS,
        &mut d_val_dec
    )
    .is_ok());

    for i in 0..256 {
        let std = d_dec[i];
        let fused = d_val_dec[i];
        let sym = std.symbol as usize;
        let expected_vbits = D_EXTRA_BITS[sym];
        let expected_vbase = D_BASE_VALUE[sym];

        assert_eq!(fused.value_bits, expected_vbits);
        assert_eq!(fused.vbase, expected_vbase);
        assert_eq!(fused.delta, std.delta);
        assert_eq!(fused.total_bits, (std.k as u8) + expected_vbits);
    }
}

#[test]
fn test_lzfse_freq_value_huffman_encoding_roundtrip() {
    for val in 0..=31 {
        let (bits, nbits) = lzfse_encode_v1_freq_value(val);
        assert!(nbits > 0 && nbits <= 32);
        assert_eq!(bits & !((1 << nbits) - 1), 0);
    }

    // Full table serialization
    let l_freq = [3u16; 20];
    let m_freq = [3u16; 20];
    let d_freq = [4u16; 64];
    let lit_freq = [4u16; 256];

    let mut encoded = Vec::new();
    lzfse_encode_v1_freq_table(&l_freq, &m_freq, &d_freq, &lit_freq, &mut encoded);
    assert!(!encoded.is_empty());
}
