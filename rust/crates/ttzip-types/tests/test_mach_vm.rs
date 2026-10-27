// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use ttzip_types::ring_buffer::SnapshotRingBuffer;

#[test]
fn test_mach_vm_allocation_and_rcu_concurrency() {
    let mut ring_buffer = SnapshotRingBuffer::new();

    // 1. Acquire inactive buffer with 32KB requirement
    let buffer = ring_buffer
        .acquire_inactive_buffer(32768)
        .expect("Failed to allocate Mach VM inactive buffer");

    assert!(buffer.len() >= 32768);
    buffer[0] = 0xAA;
    buffer[32767] = 0xBB;

    // 2. Publish and export Mach memory entry port
    let (generation, port_guard) = ring_buffer
        .publish_and_export_port()
        .expect("Failed to publish inactive buffer and export port");

    assert_eq!(generation, 1);
    assert!(port_guard.raw_port() != 0);
}
