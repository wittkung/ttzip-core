// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use ttzip_types::node::TTWorkspaceEntryNode;

#[test]
fn test_node_layout_and_alignment() {
    // 1. Verify exact 64B POD size
    assert_eq!(
        std::mem::size_of::<TTWorkspaceEntryNode>(),
        64,
        "TTWorkspaceEntryNode MUST be exactly 64 bytes to fit in a cache line"
    );

    // 2. Verify alignment
    assert_eq!(
        std::mem::align_of::<TTWorkspaceEntryNode>(),
        64,
        "TTWorkspaceEntryNode MUST be 64-byte aligned to avoid cache line splits"
    );

    // 3. Verify it is Send and Sync
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TTWorkspaceEntryNode>();
}

#[test]
fn test_node_default_initialization() {
    let node = TTWorkspaceEntryNode::default();
    
    assert_eq!(node.sort_prefix, 0);
    assert_eq!(node.name_offset, 0);
    assert_eq!(node.name_len, 0);
    assert_eq!(node.flags, 0);
}
