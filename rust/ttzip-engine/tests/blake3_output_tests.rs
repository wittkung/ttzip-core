// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and conformance tests for BLAKE3 extensible output function (XOF),
//! `Output` root finalizer, and `OutputReader` random seeking cursor.

use std::io::{Read, Seek, SeekFrom};
use ttzip_engine::crypto::blake3::{blake3, Blake3, Output, OutputReader};

const TEST_KEY: &[u8; 32] = b"whats the Elvish word for friend";
const DERIVE_KEY_CONTEXT: &str = "BLAKE3 2019-12-27 16:29:52 test vectors context";

/// Generates deterministic input buffer with repeating 251-byte cycle (0, 1, ..., 250, 0, ...).
fn generate_deterministic_input(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    buf
}

// ============================================================================
// 1. Prefix Consistency Theorem Tests (Hash[0..32] == XOF[0..32])
// ============================================================================
#[test]
fn test_blake3_prefix_consistency_theorem_various_lengths() {
    let test_lengths = [
        0, 1, 2, 7, 31, 32, 63, 64, 65, 127, 128, 129,
        511, 512, 1023, 1024, 1025, 2048, 4096, 7777,
    ];

    for &len in &test_lengths {
        let input = generate_deterministic_input(len);

        // Standard 32-byte digest
        let default_hash = blake3(&input);

        // Output descriptor root hash
        let mut hasher = Blake3::new();
        hasher.update(&input);
        let output: Output = hasher.finalize_output();

        assert_eq!(
            output.root_hash(),
            default_hash,
            "output.root_hash() must equal blake3() for length {}",
            len
        );

        // Output block 0 first 32 bytes
        let block_0 = output.root_output_block(0);
        assert_eq!(
            &block_0[..32],
            &default_hash[..],
            "output.root_output_block(0)[..32] must equal blake3() for length {}",
            len
        );

        // OutputReader XOF stream first 32 bytes and extended bytes
        let mut reader = OutputReader::new(output);
        assert_eq!(reader.position(), 0);

        let mut xof_128 = [0u8; 128];
        reader.fill(&mut xof_128);
        assert_eq!(reader.position(), 128);

        assert_eq!(
            &xof_128[..32],
            &default_hash[..],
            "XOF[..32] must strictly equal Hash[..32] for length {}",
            len
        );
        assert_eq!(
            &xof_128[..64],
            &block_0[..],
            "XOF[..64] must strictly equal block_0 for length {}",
            len
        );
    }
}

// ============================================================================
// 2. OutputReader Multi-Granularity Streaming Consistency Tests
// ============================================================================
#[test]
fn test_output_reader_multi_granularity_streaming() {
    let input = generate_deterministic_input(3500);
    let mut hasher = Blake3::new();
    hasher.update(&input);
    let output = hasher.finalize_output();

    const TOTAL_XOF_BYTES: usize = 65536;
    let mut ground_truth = vec![0u8; TOTAL_XOF_BYTES];
    let mut ref_reader = OutputReader::new(output);
    ref_reader.fill(&mut ground_truth);
    assert_eq!(ref_reader.position(), TOTAL_XOF_BYTES as u64);

    let chunk_sizes = [1, 7, 16, 17, 32, 63, 64, 65, 128, 131, 256, 1024, 4096, 65536];

    for &step in &chunk_sizes {
        let mut test_reader = OutputReader::new(output);
        let mut collected = Vec::with_capacity(TOTAL_XOF_BYTES);
        let mut buf = vec![0u8; step];

        while collected.len() < TOTAL_XOF_BYTES {
            let want = step.min(TOTAL_XOF_BYTES - collected.len());
            test_reader.fill(&mut buf[..want]);
            collected.extend_from_slice(&buf[..want]);
            assert_eq!(test_reader.position(), collected.len() as u64);
        }

        assert_eq!(
            collected, ground_truth,
            "Streaming extraction with step size {} mismatch against ground truth",
            step
        );
    }
}

// ============================================================================
// 3. OutputReader O(1) Seek and Arbitrary Offset Navigation Tests
// ============================================================================
#[test]
fn test_output_reader_seek_random_access() {
    let input = generate_deterministic_input(5000);
    let mut hasher = Blake3::new();
    hasher.update(&input);
    let output = hasher.finalize_output();

    const REFERENCE_LEN: usize = 8192;
    let mut ref_stream = vec![0u8; REFERENCE_LEN];
    let mut ref_reader = OutputReader::new(output);
    ref_reader.fill(&mut ref_stream);

    let mut reader = OutputReader::new(output);

    let seek_targets = [
        0u64, 1, 31, 32, 63, 64, 65, 100, 127, 128, 129,
        500, 1000, 1023, 1024, 1025, 4095, 4096, 4097, 8000,
    ];

    for &pos in &seek_targets {
        reader.seek(pos);
        assert_eq!(reader.position(), pos);

        let read_len = 48;
        if (pos as usize + read_len) <= REFERENCE_LEN {
            let mut read_buf = [0u8; 48];
            reader.fill(&mut read_buf);
            assert_eq!(reader.position(), pos + 48);
            assert_eq!(
                read_buf,
                ref_stream[pos as usize..pos as usize + 48],
                "Data mismatch after seeking to offset {}",
                pos
            );
        }
    }

    // Test large non-cached seek (1,000,000 bytes)
    let large_offset = 1_000_000u64;
    reader.seek(large_offset);
    assert_eq!(reader.position(), large_offset);
    let mut large_buf1 = [0u8; 64];
    reader.fill(&mut large_buf1);
    assert_eq!(reader.position(), large_offset + 64);

    // Re-seek and verify deterministic reproduction
    reader.seek(large_offset);
    let mut large_buf2 = [0u8; 64];
    reader.fill(&mut large_buf2);
    assert_eq!(large_buf1, large_buf2);

    // Backward and jumpy seeks
    let jumpy_pattern = [500, 20, 1500, 64, 0, 7000, 63, 128];
    for &pos in &jumpy_pattern {
        reader.seek(pos);
        let mut sample = [0u8; 16];
        reader.fill(&mut sample);
        assert_eq!(
            sample,
            ref_stream[pos as usize..pos as usize + 16],
            "Jumpy seek failed at offset {}",
            pos
        );
    }
}

// ============================================================================
// 4. std::io::Read and std::io::Seek Trait Conformance Tests
// ============================================================================
#[test]
fn test_output_reader_std_io_traits() {
    let input = b"BLAKE3 std::io trait conformance payload verification";
    let mut hasher = Blake3::new();
    hasher.update(input);
    let output = hasher.finalize_output();

    let mut reader = OutputReader::new(output);

    // Read trait
    let mut head = [0u8; 10];
    let bytes_read = reader.read(&mut head).expect("Read trait success");
    assert_eq!(bytes_read, 10);
    assert_eq!(reader.position(), 10);

    // SeekFrom::Start
    let pos = Seek::seek(&mut reader, SeekFrom::Start(100)).expect("SeekFrom::Start");
    assert_eq!(pos, 100);
    assert_eq!(reader.position(), 100);

    // SeekFrom::Current
    let pos2 = Seek::seek(&mut reader, SeekFrom::Current(25)).expect("SeekFrom::Current positive");
    assert_eq!(pos2, 125);
    assert_eq!(reader.position(), 125);

    let pos3 = Seek::seek(&mut reader, SeekFrom::Current(-50)).expect("SeekFrom::Current negative");
    assert_eq!(pos3, 75);
    assert_eq!(reader.position(), 75);

    // SeekFrom::End (should error)
    let end_res = Seek::seek(&mut reader, SeekFrom::End(0));
    assert!(end_res.is_err(), "SeekFrom::End must fail on infinite XOF stream");

    // Seek before start (negative result)
    let neg_res = Seek::seek(&mut reader, SeekFrom::Current(-100));
    assert!(neg_res.is_err(), "Seek before stream start must fail");
}

// ============================================================================
// 5. Official 131-Byte Golden Extended Output Vectors (Test Vectors JSON)
// ============================================================================
struct GoldenCase {
    input_len: usize,
    expected_hash_hex: &'static str,
    expected_keyed_hex: &'static str,
    expected_derive_key_hex: &'static str,
}

const GOLDEN_CASES: &[GoldenCase] = &[
    GoldenCase {
        input_len: 0,
        expected_hash_hex: "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262e00f03e7b69af26b7faaf09fcd333050338ddfe085b8cc869ca98b206c08243a26f5487789e8f660afe6c99ef9e0c52b92e7393024a80459cf91f476f9ffdbda7001c22e159b402631f277ca96f2defdf1078282314e763699a31c5363165421cce14d",
        expected_keyed_hex: "92b2b75604ed3c761f9d6f62392c8a9227ad0ea3f09573e783f1498a4ed60d26b18171a2f22a4b94822c701f107153dba24918c4bae4d2945c20ece13387627d3b73cbf97b797d5e59948c7ef788f54372df45e45e4293c7dc18c1d41144a9758be58960856be1eabbe22c2653190de560ca3b2ac4aa692a9210694254c371e851bc8f",
        expected_derive_key_hex: "2cc39783c223154fea8dfb7c1b1660f2ac2dcbd1c1de8277b0b0dd39b7e50d7d905630c8be290dfcf3e6842f13bddd573c098c3f17361f1f206b8cad9d088aa4a3f746752c6b0ce6a83b0da81d59649257cdf8eb3e9f7d4998e41021fac119deefb896224ac99f860011f73609e6e0e4540f93b273e56547dfd3aa1a035ba6689d89a0",
    },
    GoldenCase {
        input_len: 1,
        expected_hash_hex: "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213c3a6cb8bf623e20cdb535f8d1a5ffb86342d9c0b64aca3bce1d31f60adfa137b358ad4d79f97b47c3d5e79f179df87a3b9776ef8325f8329886ba42f07fb138bb502f4081cbcec3195c5871e6c23e2cc97d3c69a613eba131e5f1351f3f1da786545e5",
        expected_keyed_hex: "6d7878dfff2f485635d39013278ae14f1454b8c0a3a2d34bc1ab38228a80c95b6568c0490609413006fbd428eb3fd14e7756d90f73a4725fad147f7bf70fd61c4e0cf7074885e92b0e3f125978b4154986d4fb202a3f331a3fb6cf349a3a70e49990f98fe4289761c8602c4e6ab1138d31d3b62218078b2f3ba9a88e1d08d0dd4cea11",
        expected_derive_key_hex: "b3e2e340a117a499c6cf2398a19ee0d29cca2bb7404c73063382693bf66cb06c5827b91bf889b6b97c5477f535361caefca0b5d8c4746441c57617111933158950670f9aa8a05d791daae10ac683cbef8faf897c84e6114a59d2173c3f417023a35d6983f2c7dfa57e7fc559ad751dbfb9ffab39c2ef8c4aafebc9ae973a64f0c76551",
    },
    GoldenCase {
        input_len: 2,
        expected_hash_hex: "7b7015bb92cf0b318037702a6cdd81dee41224f734684c2c122cd6359cb1ee63d8386b22e2ddc05836b7c1bb693d92af006deb5ffbc4c70fb44d0195d0c6f252faac61659ef86523aa16517f87cb5f1340e723756ab65efb2f91964e14391de2a432263a6faf1d146937b35a33621c12d00be8223a7f1919cec0acd12097ff3ab00ab1",
        expected_keyed_hex: "5392ddae0e0a69d5f40160462cbd9bd889375082ff224ac9c758802b7a6fd20a9ffbf7efd13e989a6c246f96d3a96b9d279f2c4e63fb0bdff633957acf50ee1a5f658be144bab0f6f16500dee4aa5967fc2c586d85a04caddec90fffb7633f46a60786024353b9e5cebe277fcd9514217fee2267dcda8f7b31697b7c54fab6a939bf8f",
        expected_derive_key_hex: "1f166565a7df0098ee65922d7fea425fb18b9943f19d6161e2d17939356168e6daa59cae19892b2d54f6fc9f475d26031fd1c22ae0a3e8ef7bdb23f452a15e0027629d2e867b1bb1e6ab21c71297377750826c404dfccc2406bd57a83775f89e0b075e59a7732326715ef912078e213944f490ad68037557518b79c0086de6d6f6cdd2",
    },
    GoldenCase {
        input_len: 3,
        expected_hash_hex: "e1be4d7a8ab5560aa4199eea339849ba8e293d55ca0a81006726d184519e647f5b49b82f805a538c68915c1ae8035c900fd1d4b13902920fd05e1450822f36de9454b7e9996de4900c8e723512883f93f4345f8a58bfe64ee38d3ad71ab027765d25cdd0e448328a8e7a683b9a6af8b0af94fa09010d9186890b096a08471e4230a134",
        expected_keyed_hex: "39e67b76b5a007d4921969779fe666da67b5213b096084ab674742f0d5ec62b9b9142d0fab08e1b161efdbb28d18afc64d8f72160c958e53a950cdecf91c1a1bbab1a9c0f01def762a77e2e8545d4dec241e98a89b6db2e9a5b070fc110caae2622690bd7b76c02ab60750a3ea75426a6bb8803c370ffe465f07fb57def95df772c39f",
        expected_derive_key_hex: "440aba35cb006b61fc17c0529255de438efc06a8c9ebf3f2ddac3b5a86705797f27e2e914574f4d87ec04c379e12789eccbfbc15892626042707802dbe4e97c3ff59dca80c1e54246b6d055154f7348a39b7d098b2b4824ebe90e104e763b2a447512132cede16243484a55a4e40a85790038bb0dcf762e8c053cabae41bbe22a5bff7",
    },
    GoldenCase {
        input_len: 4,
        expected_hash_hex: "f30f5ab28fe047904037f77b6da4fea1e27241c5d132638d8bedce9d40494f328f603ba4564453e06cdcee6cbe728a4519bbe6f0d41e8a14b5b225174a566dbfa61b56afb1e452dc08c804f8c3143c9e2cc4a31bb738bf8c1917b55830c6e65797211701dc0b98daa1faeaa6ee9e56ab606ce03a1a881e8f14e87a4acf4646272cfd12",
        expected_keyed_hex: "7671dde590c95d5ac9616651ff5aa0a27bee5913a348e053b8aa9108917fe070116c0acff3f0d1fa97ab38d813fd46506089118147d83393019b068a55d646251ecf81105f798d76a10ae413f3d925787d6216a7eb444e510fd56916f1d753a5544ecf0072134a146b2615b42f50c179f56b8fae0788008e3e27c67482349e249cb86a",
        expected_derive_key_hex: "f46085c8190d69022369ce1a18880e9b369c135eb93f3c63550d3e7630e91060fbd7d8f4258bec9da4e05044f88b91944f7cab317a2f0c18279629a3867fad0662c9ad4d42c6f27e5b124da17c8c4f3a94a025ba5d1b623686c6099d202a7317a82e3d95dae46a87de0555d727a5df55de44dab799a20dffe239594d6e99ed17950910",
    },
    GoldenCase {
        input_len: 64,
        expected_hash_hex: "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98fc9cc56cb831ffe33ea8e7e1d1df09b26efd2767670066aa82d023b1dfe8ab1b2b7fbb5b97592d46ffe3e05a6a9b592e2949c74160e4674301bc3f97e04903f8c6cf95b863174c33228924cdef7ae47559b10b294acd660666c4538833582b43f82d74",
        expected_keyed_hex: "ba8ced36f327700d213f120b1a207a3b8c04330528586f414d09f2f7d9ccb7e68244c26010afc3f762615bbac552a1ca909e67c83e2fd5478cf46b9e811efccc93f77a21b17a152ebaca1695733fdb086e23cd0eb48c41c034d52523fc21236e5d8c9255306e48d52ba40b4dac24256460d56573d1312319afcf3ed39d72d0bfc69acb",
        expected_derive_key_hex: "a5c4a7053fa86b64746d4bb688d06ad1f02a18fce9afd3e818fefaa7126bf73e9b9493a9befebe0bf0c9509fb3105cfa0e262cde141aa8e3f2c2f77890bb64a4cca96922a21ead111f6338ad5244f2c15c44cb595443ac2ac294231e31be4a4307d0a91e874d36fc9852aeb1265c09b6e0cda7c37ef686fbbcab97e8ff66718be048bb",
    },
    GoldenCase {
        input_len: 65,
        expected_hash_hex: "de1e5fa0be70df6d2be8fffd0e99ceaa8eb6e8c93a63f2d8d1c30ecb6b263dee0e16e0a4749d6811dd1d6d1265c29729b1b75a9ac346cf93f0e1d7296dfcfd4313b3a227faaaaf7757cc95b4e87a49be3b8a270a12020233509b1c3632b3485eef309d0abc4a4a696c9decc6e90454b53b000f456a3f10079072baaf7a981653221f2c",
        expected_keyed_hex: "c0a4edefa2d2accb9277c371ac12fcdbb52988a86edc54f0716e1591b4326e72d5e795f46a596b02d3d4bfb43abad1e5d19211152722ec1f20fef2cd413e3c22f2fc5da3d73041275be6ede3517b3b9f0fc67ade5956a672b8b75d96cb43294b9041497de92637ed3f2439225e683910cb3ae923374449ca788fb0f9bea92731bc26ad",
        expected_derive_key_hex: "51fd05c3c1cfbc8ed67d139ad76f5cf8236cd2acd26627a30c104dfd9d3ff8a82b02e8bd36d8498a75ad8c8e9b15eb386970283d6dd42c8ae7911cc592887fdbe26a0a5f0bf821cd92986c60b2502c9be3f98a9c133a7e8045ea867e0828c7252e739321f7c2d65daee4468eb4429efae469a42763f1f94977435d10dccae3e3dce88d",
    },
    GoldenCase {
        input_len: 1024,
        expected_hash_hex: "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af71cf8107265ecdaf8505b95d8fcec83a98a6a96ea5109d2c179c47a387ffbb404756f6eeae7883b446b70ebb144527c2075ab8ab204c0086bb22b7c93d465efc57f8d917f0b385c6df265e77003b85102967486ed57db5c5ca170ba441427ed9afa684e",
        expected_keyed_hex: "75c46f6f3d9eb4f55ecaaee480db732e6c2105546f1e675003687c31719c7ba4a78bc838c72852d4f49c864acb7adafe2478e824afe51c8919d06168414c265f298a8094b1ad813a9b8614acabac321f24ce61c5a5346eb519520d38ecc43e89b5000236df0597243e4d2493fd626730e2ba17ac4d8824d09d1a4a8f57b8227778e2de",
        expected_derive_key_hex: "7356cd7720d5b66b6d0697eb3177d9f8d73a4a5c5e968896eb6a6896843027066c23b601d3ddfb391e90d5c8eccdef4ae2a264bce9e612ba15e2bc9d654af1481b2e75dbabe615974f1070bba84d56853265a34330b4766f8e75edd1f4a1650476c10802f22b64bd3919d246ba20a17558bc51c199efdec67e80a227251808d8ce5bad",
    },
    GoldenCase {
        input_len: 1025,
        expected_hash_hex: "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444f4c4a22b4b399155358a994e52bf255de60035742ec71bd08ac275a1b51cc6bfe332b0ef84b409108cda080e6269ed4b3e2c3f7d722aa4cdc98d16deb554e5627be8f955c98e1d5f9565a9194cad0c4285f93700062d9595adb992ae68ff12800ab67a",
        expected_keyed_hex: "357dc55de0c7e382c900fd6e320acc04146be01db6a8ce7210b7189bd664ea69362396b77fdc0d2634a552970843722066c3c15902ae5097e00ff53f1e116f1cd5352720113a837ab2452cafbde4d54085d9cf5d21ca613071551b25d52e69d6c81123872b6f19cd3bc1333edf0c52b94de23ba772cf82636cff4542540a7738d5b930",
        expected_derive_key_hex: "effaa245f065fbf82ac186839a249707c3bddf6d3fdda22d1b95a3c970379bcb5d31013a167509e9066273ab6e2123bc835b408b067d88f96addb550d96b6852dad38e320b9d940f86db74d398c770f462118b35d2724efa13da97194491d96dd37c3c09cbef665953f2ee85ec83d88b88d11547a6f911c8217cca46defa2751e7f3ad",
    },
];

#[test]
fn test_blake3_131_byte_golden_vectors_all_modes() {
    for case in GOLDEN_CASES {
        let input = generate_deterministic_input(case.input_len);

        // 1. Default Hash Mode (131 bytes)
        let mut hasher = Blake3::new();
        hasher.update(&input);
        let mut reader = hasher.finalize_xof();
        let mut xof_out = [0u8; 131];
        reader.fill(&mut xof_out);
        assert_eq!(
            hex::encode(xof_out),
            case.expected_hash_hex,
            "131-byte standard XOF mismatch for input_len {}",
            case.input_len
        );

        // 2. Keyed Hash Mode (131 bytes)
        let mut keyed_hasher = Blake3::new_keyed(TEST_KEY);
        keyed_hasher.update(&input);
        let mut keyed_reader = keyed_hasher.finalize_xof();
        let mut keyed_out = [0u8; 131];
        keyed_reader.fill(&mut keyed_out);
        assert_eq!(
            hex::encode(keyed_out),
            case.expected_keyed_hex,
            "131-byte Keyed XOF mismatch for input_len {}",
            case.input_len
        );

        // 3. Derive Key Mode (131 bytes)
        let mut derive_hasher = Blake3::new_derive_key(DERIVE_KEY_CONTEXT);
        derive_hasher.update(&input);
        let mut derive_reader = derive_hasher.finalize_xof();
        let mut derive_out = [0u8; 131];
        derive_reader.fill(&mut derive_out);
        assert_eq!(
            hex::encode(derive_out),
            case.expected_derive_key_hex,
            "131-byte DeriveKey XOF mismatch for input_len {}",
            case.input_len
        );
    }
}
