// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::time::Instant;
use ttzip_types::builder::VfsSnapshotBuilder;
use ttzip_types::node::TTWorkspaceEntryNode;
use ttzip_types::unified_node::{UnifiedEntryNode, UnifiedVfsTable, NO_NODE, ROOT_PARENT_ID, NODE_FLAG_DIRECTORY};

#[test]
fn test_1m_node_synthetic_tree_generation_and_indexing() {
    let count = 1_000_000;
    let mut table = UnifiedVfsTable::with_capacity(count, count * 16);

    // 1. Build a synthetic tree with 1,000,000 nodes
    let mut root_node = UnifiedEntryNode::empty();
    root_node.flags = NODE_FLAG_DIRECTORY;
    root_node.parent_id = ROOT_PARENT_ID;
    root_node.first_child_id = 1;
    table.root_id = table.add_entry(root_node, "root");

    // Add children under root
    for i in 1..count {
        let is_last = i == count - 1;
        let mut child_node = UnifiedEntryNode::empty();
        child_node.uncompressed_size = 1024;
        child_node.compressed_size = 512;
        child_node.crc32 = 0x12345678;
        child_node.parent_id = 0;
        child_node.first_child_id = NO_NODE;
        child_node.next_sibling_id = if is_last { NO_NODE } else { (i + 1) as u32 };
        child_node.mtime_epoch_secs = 1700000000;
        table.add_entry(child_node, &format!("file_{:07}.dat", i));
    }

    assert_eq!(table.len(), count);

    // 2. Measure indexation and CSR conversion time
    let start = Instant::now();
    let (csr_nodes, arena) = VfsSnapshotBuilder::build(&table);
    let elapsed = start.elapsed();

    println!(
        "Indexed {} nodes to CSR in {:.2?} ({} MB arena)",
        csr_nodes.len(),
        elapsed,
        arena.len() / (1024 * 1024)
    );

    // 3. Verify correctness and memory invariants
    assert!(csr_nodes.len() >= count);

    assert_eq!(std::mem::size_of::<TTWorkspaceEntryNode>(), 64);
    let total_node_bytes = csr_nodes.len() * std::mem::size_of::<TTWorkspaceEntryNode>();
    println!(
        "Total Flat CSR node memory footprint: {:.2} MB",
        total_node_bytes as f64 / (1024.0 * 1024.0)
    );

    // The synthetic root is at index 0. Our actual "root" node is its first child.
    let synthetic_root = &csr_nodes[0];
    let actual_root = &csr_nodes[synthetic_root.first_child_id as usize];
    let start_idx = actual_root.first_child_id as usize;
    let end_idx = start_idx + actual_root.child_count as usize;
    let children = &csr_nodes[start_idx..end_idx];

    let arena_bytes = arena.into_bytes();
    let search_start = Instant::now();
    let found = ttzip_types::node::find_child_by_name(children, &arena_bytes, b"file_0999999.dat");
    let search_elapsed = search_start.elapsed();

    if found.is_none() {
        println!("Could not find file_0999999.dat");
        println!("Children count: {}", children.len());
        if let Some(last) = children.last() {
            let offset = last.name_offset as usize;
            let len = last.name_len as usize;
            let name = String::from_utf8_lossy(&arena_bytes[offset..offset+len]);
            println!("Last child name: {}", name);
        }
        if let Some(first) = children.first() {
            let offset = first.name_offset as usize;
            let len = first.name_len as usize;
            let name = String::from_utf8_lossy(&arena_bytes[offset..offset+len]);
            println!("First child name: {}", name);
        }
    }

    assert!(found.is_some(), "Should find file_0999999.dat");
    println!("Search latency for last item: {:.2?}", search_elapsed);
    assert!(search_elapsed.as_millis() <= 8, "Search latency should be <= 8.33ms");

    // Also assert load time (distinguishing debug build unoptimized loops from opt mode)
    #[cfg(not(debug_assertions))]
    assert!(elapsed.as_millis() <= 600, "Load time should be <= 600ms in opt mode (took {}ms)", elapsed.as_millis());
    #[cfg(debug_assertions)]
    assert!(elapsed.as_millis() <= 2000, "Load time should be <= 2000ms in debug mode (took {}ms)", elapsed.as_millis());
}
