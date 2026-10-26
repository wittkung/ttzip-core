// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ttzip_types::builder::VfsSnapshotBuilder;
use ttzip_types::node::find_child_by_name;
use ttzip_types::unified_node::{
    UnifiedEntryNode, UnifiedVfsTable, NODE_FLAG_DIRECTORY, NO_NODE, ROOT_PARENT_ID,
};

fn generate_million_node_table() -> UnifiedVfsTable {
    let count = 1_000_000;
    let mut table = UnifiedVfsTable::with_capacity(count, count * 16);

    let mut root_node = UnifiedEntryNode::empty();
    root_node.flags = NODE_FLAG_DIRECTORY;
    root_node.parent_id = ROOT_PARENT_ID;
    root_node.first_child_id = 1;
    table.root_id = table.add_entry(root_node, "root");

    for i in 1..count {
        let is_last = i == count - 1;
        let mut child_node = UnifiedEntryNode::empty();
        child_node.uncompressed_size = 1024;
        child_node.compressed_size = 512;
        child_node.crc32 = 0x12345678;
        child_node.parent_id = table.root_id;
        child_node.first_child_id = NO_NODE;
        child_node.next_sibling_id = if is_last { NO_NODE } else { (i + 1) as u32 };
        child_node.mtime_epoch_secs = 1700000000;
        table.add_entry(child_node, &format!("file_{:07}.dat", i));
    }

    table
}

fn bench_load(c: &mut Criterion) {
    let table = generate_million_node_table();

    let mut group = c.benchmark_group("vfs_million_items_load");
    group.bench_function("build_csr", |b| {
        b.iter(|| {
            let (csr_nodes, arena) = VfsSnapshotBuilder::build(&table);
            black_box((csr_nodes, arena));
        })
    });
    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let table = generate_million_node_table();
    let (csr_nodes, arena) = VfsSnapshotBuilder::build(&table);
    
    // In CSR, synthetic root is at index 0. The actual root is its child.
    let synthetic_root = &csr_nodes[0];
    let actual_root = &csr_nodes[synthetic_root.first_child_id as usize];
    let start = actual_root.first_child_id as usize;
    let end = start + actual_root.child_count as usize;
    let children = &csr_nodes[start..end];

    let arena_bytes = arena.into_bytes();

    let mut group = c.benchmark_group("vfs_million_items_search");
    group.bench_function("best_case", |b| {
        b.iter(|| {
            let res = find_child_by_name(children, &arena_bytes, b"file_0000001.dat");
            black_box(res);
        })
    });

    group.bench_function("worst_case", |b| {
        b.iter(|| {
            let res = find_child_by_name(children, &arena_bytes, b"file_0999999.dat");
            black_box(res);
        })
    });

    group.bench_function("non_existent", |b| {
        b.iter(|| {
            let res = find_child_by_name(children, &arena_bytes, b"file_not_found.dat");
            black_box(res);
        })
    });
    group.finish();
}

criterion_group!(benches, bench_load, bench_search);
criterion_main!(benches);
