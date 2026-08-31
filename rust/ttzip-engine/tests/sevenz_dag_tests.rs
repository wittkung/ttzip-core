// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for 7-Zip Coder BindPairs Directed Acyclic Graph (DAG)
//! and Kahn topological sorting engine.

use ttzip_engine::sevenz::dag::{build_and_sort, CoderGraph, SevenZError};
use ttzip_engine::types::TTZipStatus;

#[test]
fn test_dag_single_coder_passthrough() {
    // Single coder (e.g. Copy / LZMA2 / Store) with 1 input stream and 1 output stream
    let num_coders = 1;
    let bind_pairs = [];
    let stream_coder_map = [0];

    let graph = CoderGraph::build(num_coders, &bind_pairs, &stream_coder_map)
        .expect("Single coder graph build must succeed");

    assert_eq!(graph.topological_order(), &[0]);
    assert_eq!(graph.packed_streams(), &[0]);
    assert_eq!(graph.main_coder(), 0);
    assert_eq!(graph.unbound_out_streams(), &[0]);

    let sort_res = build_and_sort(num_coders, &bind_pairs, &stream_coder_map)
        .expect("Standalone build_and_sort must succeed");
    assert_eq!(sort_res, vec![0]);
}

#[test]
fn test_dag_single_coder_multi_stream() {
    // Single coder with 2 input streams and 1 output stream
    let num_coders = 1;
    let bind_pairs = [];
    let stream_coder_map = [0, 0];

    let graph = CoderGraph::build(num_coders, &bind_pairs, &stream_coder_map)
        .expect("Multi-stream single coder build must succeed");

    assert_eq!(graph.topological_order(), &[0]);
    assert_eq!(graph.packed_streams(), &[0, 1]);
    assert_eq!(graph.main_coder(), 0);
    assert_eq!(graph.nodes[0].in_streams, vec![0, 1]);
}

#[test]
fn test_dag_linear_three_stage_pipeline() {
    // Linear pipeline: LZMA2 (0) -> AES (1) -> Delta (2)
    // Coder 0 produces out 0 -> consumed by Coder 1 at in 1
    // Coder 1 produces out 1 -> consumed by Coder 2 at in 2
    // Coder 2 produces out 2 (final uncompressed output)
    let num_coders = 3;
    let in_to_coder = [0, 1, 2];
    let bind_pairs = [
        (1u64, 0u64), // In 1 (Coder 1) <- Out 0 (Coder 0)
        (2u64, 1u64), // In 2 (Coder 2) <- Out 1 (Coder 1)
    ];

    let graph = CoderGraph::build(num_coders, &bind_pairs, &in_to_coder)
        .expect("Linear 3-stage pipeline build must succeed");

    assert_eq!(graph.topological_order(), &[0, 1, 2]);
    assert_eq!(graph.packed_streams(), &[0]);
    assert_eq!(graph.main_coder(), 2);
    assert_eq!(graph.unbound_out_streams(), &[2]);

    assert_eq!(graph.nodes[0].in_degree, 0);
    assert_eq!(graph.nodes[0].out_degree, 1);
    assert_eq!(graph.nodes[0].dependents, vec![1]);

    assert_eq!(graph.nodes[1].in_degree, 1);
    assert_eq!(graph.nodes[1].out_degree, 1);
    assert_eq!(graph.nodes[1].dependencies, vec![0]);
    assert_eq!(graph.nodes[1].dependents, vec![2]);

    assert_eq!(graph.nodes[2].in_degree, 1);
    assert_eq!(graph.nodes[2].out_degree, 0);
    assert_eq!(graph.nodes[2].dependencies, vec![1]);
    assert!(graph.nodes[2].is_main_coder);

    let sort_res = build_and_sort(num_coders, &bind_pairs, &in_to_coder).unwrap();
    assert_eq!(sort_res, vec![0, 1, 2]);
}

#[test]
fn test_dag_bcj2_four_stream_composite_graph() {
    // 7z BCJ2 x86 filter composite DAG:
    // Coder 0: BCJ2 (inputs: 0, 1, 2, 3; output: 0) -> Main terminal coder
    // Coder 1: LZMA (input: 4; output: 1) -> binds to BCJ2 input 1 (Call)
    // Coder 2: LZMA (input: 5; output: 2) -> binds to BCJ2 input 2 (Jump)
    // Coder 3: Range / LZMA (input: 6; output: 3) -> binds to BCJ2 input 3 (Range)
    // Unbound input stream 0 (Main) is a direct pack stream from archive.
    let num_coders = 4;
    let in_to_coder = [0, 0, 0, 0, 1, 2, 3];
    let out_to_coder = [0, 1, 2, 3];
    let bind_pairs = [
        (1u64, 1u64), // In 1 (BCJ2) <- Out 1 (Coder 1)
        (2u64, 2u64), // In 2 (BCJ2) <- Out 2 (Coder 2)
        (3u64, 3u64), // In 3 (BCJ2) <- Out 3 (Coder 3)
    ];

    let graph = CoderGraph::build_with_maps(num_coders, &bind_pairs, &in_to_coder, &out_to_coder)
        .expect("BCJ2 4-stream composite DAG build must succeed");

    // Coders 1, 2, 3 have in_degree 0 and must precede Coder 0
    let order = graph.topological_order();
    assert_eq!(order.len(), 4);
    assert_eq!(order[3], 0); // Terminal coder BCJ2 is processed last
    assert!(order[..3].contains(&1));
    assert!(order[..3].contains(&2));
    assert!(order[..3].contains(&3));

    // Packed streams: In 0 (BCJ2 Main), In 4 (Coder 1), In 5 (Coder 2), In 6 (Coder 3)
    assert_eq!(graph.packed_streams(), &[0, 4, 5, 6]);
    assert_eq!(graph.main_coder(), 0);
    assert_eq!(graph.unbound_out_streams(), &[0]);
    assert!(graph.nodes[0].is_main_coder);

    // Verify node dependencies
    assert_eq!(graph.nodes[0].in_degree, 3);
    assert_eq!(graph.nodes[0].dependencies, vec![1, 2, 3]);
}

#[test]
fn test_dag_two_node_deadlock_cycle_interception() {
    // 2-node cycle: Coder 0 -> Coder 1 and Coder 1 -> Coder 0
    let num_coders = 2;
    let in_to_coder = [0, 1];
    let bind_pairs = [
        (0u64, 1u64), // In 0 (Coder 0) <- Out 1 (Coder 1)
        (1u64, 0u64), // In 1 (Coder 1) <- Out 0 (Coder 0)
    ];

    let res = CoderGraph::build(num_coders, &bind_pairs, &in_to_coder);
    assert_eq!(res.unwrap_err(), SevenZError::CyclicBindPairs);

    let sort_res = build_and_sort(num_coders, &bind_pairs, &in_to_coder);
    assert_eq!(sort_res.unwrap_err(), SevenZError::CyclicBindPairs);
}

#[test]
fn test_dag_three_node_triangular_cyclic_deadlock() {
    // 3-node cycle: Coder 0 -> Coder 1 -> Coder 2 -> Coder 0
    let num_coders = 3;
    let in_to_coder = [0, 1, 2];
    let bind_pairs = [
        (1u64, 0u64), // In 1 (Coder 1) <- Out 0 (Coder 0)
        (2u64, 1u64), // In 2 (Coder 2) <- Out 1 (Coder 1)
        (0u64, 2u64), // In 0 (Coder 0) <- Out 2 (Coder 2)
    ];

    let res = CoderGraph::build(num_coders, &bind_pairs, &in_to_coder);
    assert_eq!(res.unwrap_err(), SevenZError::CyclicBindPairs);
}

#[test]
fn test_dag_self_loop_rejection() {
    // Self-loop: Coder 0 binds its own output to its own input
    let num_coders = 2;
    let in_to_coder = [0, 1];
    let bind_pairs = [(0u64, 0u64)]; // In 0 (Coder 0) <- Out 0 (Coder 0)

    let res = CoderGraph::build(num_coders, &bind_pairs, &in_to_coder);
    assert_eq!(
        res.unwrap_err(),
        SevenZError::SelfLoop { coder_index: 0 }
    );
}

#[test]
fn test_dag_empty_folder_rejection() {
    let res = CoderGraph::build(0, &[], &[]);
    assert_eq!(res.unwrap_err(), SevenZError::EmptyFolder);
}

#[test]
fn test_dag_invalid_stream_index() {
    let num_coders = 2;
    let in_to_coder = [0, 1];
    // In stream index 99 is out of bounds (total in streams is 2)
    let bind_pairs = [(99u64, 0u64)];

    let res = CoderGraph::build(num_coders, &bind_pairs, &in_to_coder);
    assert_eq!(
        res.unwrap_err(),
        SevenZError::InvalidStreamIndex {
            stream_index: 99,
            max_streams: 2,
        }
    );
}

#[test]
fn test_dag_duplicate_stream_bind_rejection() {
    let num_coders = 3;
    let in_to_coder = [0, 1, 2];
    // In stream 1 bound twice
    let bind_pairs = [
        (1u64, 0u64),
        (1u64, 2u64),
    ];

    let res = CoderGraph::build(num_coders, &bind_pairs, &in_to_coder);
    assert_eq!(
        res.unwrap_err(),
        SevenZError::StreamAlreadyBound {
            stream_index: 1,
        }
    );
}

#[test]
fn test_sevenz_error_into_ttzip_status() {
    let err = SevenZError::CyclicBindPairs;
    let status: TTZipStatus = err.into();
    assert_eq!(status, TTZipStatus::ErrCorruptHeader);

    let err2 = SevenZError::SelfLoop { coder_index: 1 };
    let status2: TTZipStatus = err2.into();
    assert_eq!(status2, TTZipStatus::ErrCorruptHeader);
}
