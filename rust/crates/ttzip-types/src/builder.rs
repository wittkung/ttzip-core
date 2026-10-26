// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use crate::arena::StringArena;
use crate::mach_vm::MachVmAllocation;
use crate::node::{TTWorkspaceAux, TTWorkspaceEntryNode};
use crate::snapshot::{
    CompactVector128, EmbeddingSectionHeader, TTWorkspaceSectionEntry, TTWorkspaceSnapshotHeader,
    TT_EMBEDDING_DIMENSION_128, TT_EMBEDDING_MAGIC, TT_EMBEDDING_QUANTIZATION_INT8,
    TT_EMBEDDING_VERSION_1, TT_FEATURE_FLAG_CSR, TT_FEATURE_FLAG_LEXSORT, TT_FEATURE_FLAG_NAME_MASK,
    TT_SECTION_FLAG_ALIGNED_64, TT_SECTION_FLAG_READ_ONLY, TT_SECTION_STRING_ARENA,
    TT_SECTION_TYPE_EMBEDDINGS, TT_SECTION_TYPE_NAME_MASKS, TT_SECTION_TYPE_STABLE_ID_INDICES,
    TT_SECTION_TYPE_STABLE_ID_KEYS, TT_SECTION_VFS_NODES,
};
use crate::status::TTZipStatus;
use crate::unified_node::UnifiedVfsTable;
use std::collections::VecDeque;
use std::sync::Arc;

/// Converts a legacy `UnifiedVfsTable` (which uses linked-list siblings)
/// into the highly-optimized `TTWorkspaceEntryNode` Flat CSR contiguous array.
pub struct VfsSnapshotBuilder;

impl VfsSnapshotBuilder {
    /// Performs an O(N) lock-free flattening of the AST into a CSR array.
    /// Returns the built CSR node vector and the populated StringArena.
    pub fn build(ast: &UnifiedVfsTable) -> (Vec<TTWorkspaceEntryNode>, StringArena<'_>) {
        let node_count = ast.nodes.len();
        let mut csr_nodes = Vec::with_capacity(node_count);
        let mut arena = StringArena::with_capacity(ast.strings.len() + 1024, node_count);

        if node_count == 0 {
            return (csr_nodes, arena);
        }

        csr_nodes.push(Self::create_synthetic_root(&mut arena));

        let mut queue = VecDeque::new();
        queue.push_back((0, ast.root_id));

        while let Some((parent_csr_idx, child_ast_idx)) = queue.pop_front() {
            Self::process_ast_siblings(
                ast,
                parent_csr_idx,
                child_ast_idx,
                &mut csr_nodes,
                &mut arena,
                &mut queue,
            );
        }

        (csr_nodes, arena)
    }

    /// Creates the synthetic root node (index 0) required by the CSR ABI.
    #[inline]
    fn create_synthetic_root(arena: &mut StringArena<'_>) -> TTWorkspaceEntryNode {
        TTWorkspaceEntryNode {
            parent_id: u32::MAX,
            first_child_id: u32::MAX,
            child_count: 0,
            name_offset: arena.intern(""),
            sort_prefix: 0,
            uncompressed_size: 0,
            mtime_epoch_s: 0,
            aux: TTWorkspaceAux { compressed_size: 0 },
            stable_id: 0,
            file_type: libc::S_IFDIR as u32,
            flags: 0,
            name_len: 0,
        }
    }

    /// Computes the 8-byte big-endian sort prefix for fast binary search.
    #[inline]
    fn compute_sort_prefix(name: &str) -> u64 {
        let bytes = name.as_bytes();
        let copy_len = bytes.len().min(8);
        if copy_len == 0 {
            return 0;
        }

        let mut buf = [0u8; 8];
        buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
        u64::from_be_bytes(buf)
    }

    /// Traverses the AST sibling linked list and sorts siblings lexicographically by name.
    fn collect_and_sort_siblings(ast: &UnifiedVfsTable, mut child_ast_idx: u32) -> Vec<u32> {
        let mut siblings = Vec::new();
        while child_ast_idx != crate::unified_node::NO_NODE {
            siblings.push(child_ast_idx);
            child_ast_idx = ast.nodes[child_ast_idx as usize].next_sibling_id;
        }

        siblings.sort_by_key(|&idx| {
            let node = &ast.nodes[idx as usize];
            ast.strings.get(node.path_offset, node.path_len).unwrap_or("")
        });

        siblings
    }

    /// Converts a single AST node into a contiguous CSR workspace entry node.
    fn convert_ast_node_to_csr<'a>(
        ast: &'a UnifiedVfsTable,
        ast_idx: u32,
        parent_csr_idx: u32,
        arena: &mut StringArena<'a>,
    ) -> TTWorkspaceEntryNode {
        let ast_node = &ast.nodes[ast_idx as usize];
        let name = ast.strings.get(ast_node.path_offset, ast_node.path_len).unwrap_or("");
        let name_offset = arena.intern(name);
        let name_len = name.len().min(u16::MAX as usize) as u16;
        let sort_prefix = Self::compute_sort_prefix(name);

        TTWorkspaceEntryNode {
            parent_id: parent_csr_idx,
            first_child_id: u32::MAX,
            child_count: 0,
            name_offset,
            sort_prefix,
            uncompressed_size: ast_node.uncompressed_size,
            mtime_epoch_s: 0,
            aux: TTWorkspaceAux { compressed_size: ast_node.compressed_size },
            stable_id: ast_idx as u64,
            file_type: ast_node.flags as u32,
            flags: 0,
            name_len,
        }
    }

    /// Collects, sorts, and appends the siblings of a parent to the CSR node buffer.
    fn process_ast_siblings<'a>(
        ast: &'a UnifiedVfsTable,
        parent_csr_idx: u32,
        child_ast_idx: u32,
        csr_nodes: &mut Vec<TTWorkspaceEntryNode>,
        arena: &mut StringArena<'a>,
        queue: &mut VecDeque<(u32, u32)>,
    ) {
        if child_ast_idx == crate::unified_node::NO_NODE {
            return;
        }

        let siblings = Self::collect_and_sort_siblings(ast, child_ast_idx);
        if siblings.is_empty() {
            return;
        }

        let first_child_csr_idx = csr_nodes.len() as u32;
        csr_nodes[parent_csr_idx as usize].first_child_id = first_child_csr_idx;
        csr_nodes[parent_csr_idx as usize].child_count = siblings.len() as u32;

        for &ast_idx in &siblings {
            let child_csr_idx = csr_nodes.len() as u32;
            let csr_node = Self::convert_ast_node_to_csr(ast, ast_idx, parent_csr_idx, arena);
            csr_nodes.push(csr_node);

            let ast_node = &ast.nodes[ast_idx as usize];
            if ast_node.first_child_id != crate::unified_node::NO_NODE {
                queue.push_back((child_csr_idx, ast_node.first_child_id));
            }
        }
    }

    /// Constructs a serialized in-memory snapshot with `TTWorkspaceSnapshotHeader` (128 bytes),
    /// `TT_SECTION_VFS_NODES`, `TT_SECTION_STRING_ARENA`, `TT_SECTION_TYPE_STABLE_ID_KEYS`,
    /// `TT_SECTION_TYPE_STABLE_ID_INDICES`, and `TT_SECTION_TYPE_NAME_MASKS`.
    /// Constructs a serialized in-memory snapshot, returning `Result<Arc<MachVmAllocation>, TTZipStatus>`.
    pub fn try_build_snapshot(ast: &UnifiedVfsTable) -> Result<Arc<MachVmAllocation>, TTZipStatus> {
        Self::try_build_snapshot_with_embeddings(ast, None, 0)
    }

    /// Constructs a serialized in-memory snapshot with optional vector embeddings section 9,
    /// returning `Result<Arc<MachVmAllocation>, TTZipStatus>`.
    pub fn try_build_snapshot_with_embeddings(
        ast: &UnifiedVfsTable,
        embeddings: Option<&[CompactVector128]>,
        model_fingerprint: u64,
    ) -> Result<Arc<MachVmAllocation>, TTZipStatus> {
        let (csr_nodes, arena) = Self::build(ast);
        let nodes_bytes = csr_nodes.len() * std::mem::size_of::<TTWorkspaceEntryNode>();
        let arena_slice = arena.into_bytes();

        let aux = compute_auxiliary_columns(&csr_nodes, arena_slice.as_slice());
        let k_bytes = aux.stable_keys.len() * std::mem::size_of::<u64>();
        let i_bytes = aux.stable_indices.len() * std::mem::size_of::<u32>();
        let m_bytes = aux.name_masks.len() * std::mem::size_of::<u64>();

        let emb_bytes = embeddings.map_or(0, |embs| {
            std::mem::size_of::<EmbeddingSectionHeader>()
                + std::mem::size_of_val(embs)
        });

        let off = calculate_offsets(
            nodes_bytes,
            arena_slice.len(),
            k_bytes,
            i_bytes,
            m_bytes,
            emb_bytes,
            embeddings.is_some(),
        );
        let mut alloc = MachVmAllocation::allocate(off.total)
            .map_err(|_| TTZipStatus::ErrOutOfMemory)?;

        let mut sections = Vec::with_capacity(if embeddings.is_some() { 6 } else { 5 });
        sections.push(make_section_entry(TT_SECTION_VFS_NODES, off.nodes, nodes_bytes, csr_nodes.len()));
        sections.push(make_section_entry(TT_SECTION_STRING_ARENA, off.arena, arena_slice.len(), arena_slice.len()));
        sections.push(make_section_entry(TT_SECTION_TYPE_STABLE_ID_KEYS, off.keys, k_bytes, aux.stable_keys.len()));
        sections.push(make_section_entry(TT_SECTION_TYPE_STABLE_ID_INDICES, off.indices, i_bytes, aux.stable_indices.len()));
        sections.push(make_section_entry(TT_SECTION_TYPE_NAME_MASKS, off.masks, m_bytes, aux.name_masks.len()));

        if let Some(embs) = embeddings {
            sections.push(make_section_entry(
                TT_SECTION_TYPE_EMBEDDINGS,
                off.embeddings,
                emb_bytes,
                embs.len(),
            ));
        }

        let header = TTWorkspaceSnapshotHeader {
            total_mapping_size: off.total as u64,
            section_count: sections.len() as u32,
            section_headers_offset: 128,
            feature_flags: TT_FEATURE_FLAG_CSR | TT_FEATURE_FLAG_LEXSORT | TT_FEATURE_FLAG_NAME_MASK,
            ..TTWorkspaceSnapshotHeader::default()
        };

        write_snapshot_buffers(
            alloc.as_mut_ptr(),
            &header,
            &sections,
            128,
            &csr_nodes,
            off.nodes,
            arena_slice.as_slice(),
            off.arena,
            &aux,
            off.keys,
            off.indices,
            off.masks,
            embeddings,
            off.embeddings,
            model_fingerprint,
        );

        Ok(Arc::new(alloc))
    }

    /// Constructs a serialized in-memory snapshot with `TTWorkspaceSnapshotHeader` (128 bytes),
    /// `TT_SECTION_VFS_NODES`, `TT_SECTION_STRING_ARENA`, `TT_SECTION_TYPE_STABLE_ID_KEYS`,
    /// `TT_SECTION_TYPE_STABLE_ID_INDICES`, and `TT_SECTION_TYPE_NAME_MASKS`.
    pub fn build_snapshot(ast: &UnifiedVfsTable) -> Arc<MachVmAllocation> {
        Self::build_snapshot_with_embeddings(ast, None, 0)
    }

    /// Constructs a serialized in-memory snapshot with optional vector embeddings section 9.
    pub fn build_snapshot_with_embeddings(
        ast: &UnifiedVfsTable,
        embeddings: Option<&[CompactVector128]>,
        model_fingerprint: u64,
    ) -> Arc<MachVmAllocation> {
        Self::try_build_snapshot_with_embeddings(ast, embeddings, model_fingerprint)
            .unwrap_or_else(|_| {
                MachVmAllocation::allocate(16384)
                    .map(Arc::new)
                    .expect("failed to allocate fallback Mach VM page")
            })
    }
}

struct SectionOffsets {
    nodes: usize,
    arena: usize,
    keys: usize,
    indices: usize,
    masks: usize,
    embeddings: usize,
    total: usize,
}

fn calculate_offsets(
    nodes_bytes: usize,
    arena_len: usize,
    keys_len: usize,
    indices_len: usize,
    masks_len: usize,
    embeddings_bytes: usize,
    has_embeddings: bool,
) -> SectionOffsets {
    let section_count = if has_embeddings { 6 } else { 5 };
    let sections_size = section_count * std::mem::size_of::<TTWorkspaceSectionEntry>();
    let nodes = (128 + sections_size + 63) & !63;
    let arena = (nodes + nodes_bytes + 63) & !63;
    let keys = (arena + arena_len + 63) & !63;
    let indices = (keys + keys_len + 63) & !63;
    let masks = (indices + indices_len + 63) & !63;
    let (embeddings, total) = if has_embeddings {
        let emb = (masks + masks_len + 63) & !63;
        let tot = (emb + embeddings_bytes + 16383) & !16383;
        (emb, tot)
    } else {
        let tot = (masks + masks_len + 16383) & !16383;
        (0, tot)
    };
    SectionOffsets { nodes, arena, keys, indices, masks, embeddings, total }
}

#[inline(always)]
fn make_section_entry(
    section_type: u32,
    offset: usize,
    size_bytes: usize,
    entry_count: usize,
) -> TTWorkspaceSectionEntry {
    TTWorkspaceSectionEntry {
        section_type,
        flags: TT_SECTION_FLAG_ALIGNED_64 | TT_SECTION_FLAG_READ_ONLY,
        offset: offset as u64,
        size_bytes: size_bytes as u64,
        entry_count: entry_count as u64,
    }
}

struct AuxiliaryColumns {
    stable_keys: Vec<u64>,
    stable_indices: Vec<u32>,
    name_masks: Vec<u64>,
}

fn compute_auxiliary_columns(nodes: &[TTWorkspaceEntryNode], arena_bytes: &[u8]) -> AuxiliaryColumns {
    let mut stable_pairs: Vec<(u64, u32)> = nodes
        .iter()
        .enumerate()
        .map(|(idx, n)| (n.stable_id, idx as u32))
        .collect();
    stable_pairs.sort_unstable_by_key(|&(id, _)| id);
    let stable_keys: Vec<u64> = stable_pairs.iter().map(|&(id, _)| id).collect();
    let stable_indices: Vec<u32> = stable_pairs.iter().map(|&(_, idx)| idx).collect();

    let name_masks: Vec<u64> = nodes
        .iter()
        .map(|node| {
            let start = node.name_offset as usize;
            let end = start.saturating_add(node.name_len as usize);
            let name_bytes = arena_bytes.get(start..end).unwrap_or(&[]);
            crate::search::compute_name_mask(name_bytes)
        })
        .collect();

    AuxiliaryColumns {
        stable_keys,
        stable_indices,
        name_masks,
    }
}

fn write_snapshot_buffers(
    ptr: *mut u8,
    header: &TTWorkspaceSnapshotHeader,
    sections: &[TTWorkspaceSectionEntry],
    sections_offset: usize,
    nodes: &[TTWorkspaceEntryNode],
    nodes_offset: usize,
    arena: &[u8],
    arena_offset: usize,
    aux: &AuxiliaryColumns,
    keys_offset: usize,
    indices_offset: usize,
    masks_offset: usize,
    embeddings: Option<&[CompactVector128]>,
    embeddings_offset: usize,
    model_fingerprint: u64,
) {
    unsafe {
        std::ptr::write(ptr as *mut TTWorkspaceSnapshotHeader, *header);
        std::ptr::copy_nonoverlapping(
            sections.as_ptr(),
            ptr.add(sections_offset) as *mut TTWorkspaceSectionEntry,
            sections.len(),
        );
        if !nodes.is_empty() {
            std::ptr::copy_nonoverlapping(
                nodes.as_ptr() as *const u8,
                ptr.add(nodes_offset),
                std::mem::size_of_val(nodes),
            );
        }
        if !arena.is_empty() {
            std::ptr::copy_nonoverlapping(arena.as_ptr(), ptr.add(arena_offset), arena.len());
        }
        if !aux.stable_keys.is_empty() {
            std::ptr::copy_nonoverlapping(
                aux.stable_keys.as_ptr() as *const u8,
                ptr.add(keys_offset),
                std::mem::size_of_val(aux.stable_keys.as_slice()),
            );
        }
        if !aux.stable_indices.is_empty() {
            std::ptr::copy_nonoverlapping(
                aux.stable_indices.as_ptr() as *const u8,
                ptr.add(indices_offset),
                std::mem::size_of_val(aux.stable_indices.as_slice()),
            );
        }
        if !aux.name_masks.is_empty() {
            std::ptr::copy_nonoverlapping(
                aux.name_masks.as_ptr() as *const u8,
                ptr.add(masks_offset),
                std::mem::size_of_val(aux.name_masks.as_slice()),
            );
        }
        if let Some(embs) = embeddings {
            let emb_hdr = EmbeddingSectionHeader {
                magic: TT_EMBEDDING_MAGIC,
                version: TT_EMBEDDING_VERSION_1,
                entry_count: embs.len() as u32,
                dimension: TT_EMBEDDING_DIMENSION_128,
                quantization: TT_EMBEDDING_QUANTIZATION_INT8,
                _reserved1: 0,
                model_fingerprint,
                _reserved2: [0u8; 8],
            };
            let hdr_size = std::mem::size_of::<EmbeddingSectionHeader>();
            std::ptr::write(ptr.add(embeddings_offset) as *mut EmbeddingSectionHeader, emb_hdr);
            if !embs.is_empty() {
                std::ptr::copy_nonoverlapping(
                    embs.as_ptr() as *const u8,
                    ptr.add(embeddings_offset + hdr_size),
                    std::mem::size_of_val(embs),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified_node::{UnifiedEntryNode, NODE_FLAG_DIRECTORY, NO_NODE, ROOT_PARENT_ID};

    #[test]
    fn test_compute_sort_prefix() {
        assert_eq!(VfsSnapshotBuilder::compute_sort_prefix(""), 0);
        let prefix_a = VfsSnapshotBuilder::compute_sort_prefix("a");
        let prefix_b = VfsSnapshotBuilder::compute_sort_prefix("b");
        assert!(prefix_a < prefix_b);

        let prefix_exact = VfsSnapshotBuilder::compute_sort_prefix("12345678");
        assert_eq!(prefix_exact, u64::from_be_bytes(*b"12345678"));

        let prefix_long = VfsSnapshotBuilder::compute_sort_prefix("1234567890");
        assert_eq!(prefix_long, u64::from_be_bytes(*b"12345678"));
    }

    #[test]
    fn test_build_empty_ast() {
        let table = UnifiedVfsTable::with_capacity(0, 0);
        let (csr_nodes, arena) = VfsSnapshotBuilder::build(&table);
        assert!(csr_nodes.is_empty());
        assert!(arena.is_empty());
    }

    #[test]
    fn test_build_single_root_node() {
        let mut table = UnifiedVfsTable::with_capacity(1, 64);
        let mut node = UnifiedEntryNode::empty();
        node.parent_id = ROOT_PARENT_ID;
        node.flags = NODE_FLAG_DIRECTORY;
        table.root_id = table.add_entry(node, "root");

        let (csr_nodes, arena) = VfsSnapshotBuilder::build(&table);
        assert_eq!(csr_nodes.len(), 2);
        assert_eq!(csr_nodes[0].first_child_id, 1);
        assert_eq!(csr_nodes[0].child_count, 1);
        assert_eq!(csr_nodes[1].parent_id, 0);
        assert_eq!(csr_nodes[1].first_child_id, u32::MAX);
        assert_eq!(csr_nodes[1].child_count, 0);

        let bytes = arena.into_bytes();
        let name_start = csr_nodes[1].name_offset as usize;
        let name_len = csr_nodes[1].name_len as usize;
        let resolved = std::str::from_utf8(&bytes[name_start..name_start + name_len]).unwrap();
        assert_eq!(resolved, "root");
    }

    #[test]
    fn test_build_sorted_siblings_and_hierarchy() {
        let mut table = UnifiedVfsTable::with_capacity(4, 128);

        let mut root = UnifiedEntryNode::empty();
        root.parent_id = ROOT_PARENT_ID;
        root.flags = NODE_FLAG_DIRECTORY;
        root.first_child_id = 1;
        table.root_id = table.add_entry(root, "dir");

        let mut child_z = UnifiedEntryNode::empty();
        child_z.parent_id = 0;
        child_z.next_sibling_id = 2;
        table.add_entry(child_z, "zebra.txt");

        let mut child_a = UnifiedEntryNode::empty();
        child_a.parent_id = 0;
        child_a.next_sibling_id = 3;
        table.add_entry(child_a, "alpha.txt");

        let mut child_b = UnifiedEntryNode::empty();
        child_b.parent_id = 0;
        child_b.next_sibling_id = NO_NODE;
        table.add_entry(child_b, "beta.txt");

        let (csr_nodes, arena) = VfsSnapshotBuilder::build(&table);
        assert_eq!(csr_nodes.len(), 5);

        // Synthetic root -> dir
        assert_eq!(csr_nodes[0].child_count, 1);
        assert_eq!(csr_nodes[0].first_child_id, 1);

        // "dir" node at index 1 -> 3 children starting at index 2
        assert_eq!(csr_nodes[1].child_count, 3);
        assert_eq!(csr_nodes[1].first_child_id, 2);

        // Children should be sorted lexicographically: alpha.txt, beta.txt, zebra.txt
        let bytes = arena.into_bytes();
        let resolve_name = |node: &TTWorkspaceEntryNode| -> &str {
            let start = node.name_offset as usize;
            let len = node.name_len as usize;
            std::str::from_utf8(&bytes[start..start + len]).unwrap()
        };

        assert_eq!(resolve_name(&csr_nodes[2]), "alpha.txt");
        assert_eq!(resolve_name(&csr_nodes[3]), "beta.txt");
        assert_eq!(resolve_name(&csr_nodes[4]), "zebra.txt");

        // Sort prefixes must be monotonically non-decreasing
        assert!(csr_nodes[2].sort_prefix <= csr_nodes[3].sort_prefix);
        assert!(csr_nodes[3].sort_prefix <= csr_nodes[4].sort_prefix);
    }

    #[test]
    fn test_build_snapshot_sections_and_flags() {
        let mut table = UnifiedVfsTable::with_capacity(2, 64);
        let mut root = UnifiedEntryNode::empty();
        root.parent_id = ROOT_PARENT_ID;
        root.flags = NODE_FLAG_DIRECTORY;
        root.first_child_id = 1;
        table.root_id = table.add_entry(root, "root");

        let mut child = UnifiedEntryNode::empty();
        child.parent_id = 0;
        child.next_sibling_id = NO_NODE;
        table.add_entry(child, "test.txt");

        let alloc = VfsSnapshotBuilder::build_snapshot(&table);
        let ptr = alloc.as_ptr();
        let header = unsafe { &*(ptr as *const TTWorkspaceSnapshotHeader) };
        assert!(header.validate(alloc.len()).is_ok());

        let mapping = unsafe { std::slice::from_raw_parts(ptr, alloc.len()) };
        assert_eq!(header.section_count, 5);

        let nodes_sec = header.find_section(mapping, TT_SECTION_VFS_NODES).unwrap();
        assert_eq!(nodes_sec.flags, TT_SECTION_FLAG_ALIGNED_64 | TT_SECTION_FLAG_READ_ONLY);
        assert_eq!(nodes_sec.entry_count, 3);

        let arena_sec = header.find_section(mapping, TT_SECTION_STRING_ARENA).unwrap();
        assert_eq!(arena_sec.flags, TT_SECTION_FLAG_ALIGNED_64 | TT_SECTION_FLAG_READ_ONLY);

        let keys_sec = header.find_section(mapping, TT_SECTION_TYPE_STABLE_ID_KEYS).unwrap();
        assert_eq!(keys_sec.entry_count, 3);

        let indices_sec = header.find_section(mapping, TT_SECTION_TYPE_STABLE_ID_INDICES).unwrap();
        assert_eq!(indices_sec.entry_count, 3);

        let masks_sec = header.find_section(mapping, TT_SECTION_TYPE_NAME_MASKS).unwrap();
        assert_eq!(masks_sec.entry_count, 3);
    }

    #[test]
    fn test_build_snapshot_with_embeddings() {
        let mut table = UnifiedVfsTable::with_capacity(2, 64);
        let mut root = UnifiedEntryNode::empty();
        root.parent_id = ROOT_PARENT_ID;
        root.flags = NODE_FLAG_DIRECTORY;
        root.first_child_id = 1;
        table.root_id = table.add_entry(root, "root");

        let mut child = UnifiedEntryNode::empty();
        child.parent_id = 0;
        child.next_sibling_id = NO_NODE;
        table.add_entry(child, "file.rs");

        // 3 nodes in total: synthetic root, root, file.rs
        let mut v0 = CompactVector128::zero();
        v0[0] = 10;
        let mut v1 = CompactVector128::zero();
        v1[1] = 20;
        let mut v2 = CompactVector128::zero();
        v2[2] = 30;
        let vectors = [v0, v1, v2];

        let alloc = VfsSnapshotBuilder::build_snapshot_with_embeddings(&table, Some(&vectors), 0xCAFEBABE_DEADBEEF);
        let ptr = alloc.as_ptr();
        let header = unsafe { &*(ptr as *const TTWorkspaceSnapshotHeader) };
        assert!(header.validate(alloc.len()).is_ok());

        let mapping = unsafe { std::slice::from_raw_parts(ptr, alloc.len()) };
        assert_eq!(header.section_count, 6);

        let emb_tuple = header.find_embeddings(mapping);
        assert!(emb_tuple.is_some());
        let (emb_hdr, retrieved_vectors) = emb_tuple.unwrap();

        assert_eq!(emb_hdr.magic, TT_EMBEDDING_MAGIC);
        assert_eq!(emb_hdr.version, TT_EMBEDDING_VERSION_1);
        assert_eq!(emb_hdr.dimension, TT_EMBEDDING_DIMENSION_128);
        assert_eq!(emb_hdr.quantization, TT_EMBEDDING_QUANTIZATION_INT8);
        assert_eq!(emb_hdr.entry_count, 3);
        assert_eq!(emb_hdr.model_fingerprint, 0xCAFEBABE_DEADBEEF);

        assert_eq!(retrieved_vectors.len(), 3);
        assert_eq!(retrieved_vectors[0][0], 10);
        assert_eq!(retrieved_vectors[1][1], 20);
        assert_eq!(retrieved_vectors[2][2], 30);
    }

    #[test]
    fn test_try_build_snapshot() {
        let mut table = UnifiedVfsTable::with_capacity(2, 64);
        let mut root = UnifiedEntryNode::empty();
        root.parent_id = ROOT_PARENT_ID;
        root.flags = NODE_FLAG_DIRECTORY;
        root.first_child_id = 1;
        table.root_id = table.add_entry(root, "root");

        let mut child = UnifiedEntryNode::empty();
        child.parent_id = 0;
        child.next_sibling_id = NO_NODE;
        table.add_entry(child, "file.rs");

        let res = VfsSnapshotBuilder::try_build_snapshot(&table);
        assert!(res.is_ok());
        let alloc = res.unwrap();
        let ptr = alloc.as_ptr();
        let header = unsafe { &*(ptr as *const TTWorkspaceSnapshotHeader) };
        assert!(header.validate(alloc.len()).is_ok());
    }
}
