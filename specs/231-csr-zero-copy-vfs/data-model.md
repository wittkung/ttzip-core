# Data Model & Memory Geometry: CSR Zero-Copy VFS (Feature 231)

**Feature Directory**: `products/ttzip/core/specs/231-csr-zero-copy-vfs`

---

## 1. Physical Memory Geometry

```
┌────────────────────────────────────────────────────────────────────────┐
│               Mach VM 16KB Page-Aligned Allocation                     │
├────────────────────────────────────────────────────────────────────────┤
│ 1. Header (64 bytes)                                                   │
│    - magic: 0x54545A5646533031 ("TTZVFS01")                            │
│    - node_count: u32                                                   │
│    - string_arena_size: u32                                            │
│    - root_node_id: u32                                                 │
│    - generation: u32                                                   │
│    - reserved / padding to 64 bytes                                    │
├────────────────────────────────────────────────────────────────────────┤
│ 2. Flat CSR Nodes Array (node_count * 64 bytes)                        │
│    - Node 0 (Synthetic Root, 64B)                                      │
│    - Node 1 (First Child, 64B)                                         │
│    ...                                                                 │
│    - Node N (Last Node, 64B)                                           │
├────────────────────────────────────────────────────────────────────────┤
│ 3. Contiguous String Arena (string_arena_size bytes)                   │
│    - Monotonically appended UTF-8 path segments                        │
│    - Substrings indexed by (name_offset, name_len)                     │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Core Entities

### 2.1 `TTWorkspaceEntryNode` (C-ABI / Rust / Swift, 64 Bytes)

| Offset (Bytes) | Field | Type | Description |
|---|---|---|---|
| `0..4` | `parent_id` | `u32` / `UInt32` | Parent node index (`0xFFFFFFFF` = synthetic root) |
| `4..8` | `first_child_id` | `u32` / `UInt32` | First direct child index in CSR contiguous slice (`0xFFFFFFFF` = None) |
| `8..12` | `child_count` | `u32` / `UInt32` | Direct child count in contiguous slice |
| `12..16` | `name_offset` | `u32` / `UInt32` | Byte offset into `StringArena` |
| `16..24` | `sort_prefix` | `u64` / `UInt64` | Big-endian first 8 bytes of filename for register comparisons |
| `24..32` | `uncompressed_size` | `u64` / `UInt64` | Uncompressed file size in bytes (0 for directories) |
| `32..40` | `mtime_epoch_s` | `i64` / `Int64` | Modification timestamp (Unix epoch seconds) |
| `40..48` | `aux` | `u64` / `UInt64` | Compressed size or device payload |
| `48..56` | `stable_id` | `u64` / `UInt64` | Stable cross-generation unique entry identifier |
| `56..60` | `file_type` | `u32` / `UInt32` | POSIX mode (`S_IFDIR`, `S_IFREG`, `S_IFLNK`) |
| `60..62` | `flags` | `u16` / `UInt16` | Bitmask: encrypted, hidden, read-only |
| `62..64` | `name_len` | `u16` / `UInt16` | Byte length of UTF-8 filename in `StringArena` |

### 2.2 `TTWorkspaceVfsView` (C-ABI Bridge Struct)

```c
typedef struct {
    const uint8_t *nodes;              // Pointer to contiguous TTWorkspaceEntryNode array
    const uint8_t *string_arena;       // Pointer to contiguous UTF-8 byte arena
    uint32_t node_count;               // Total number of nodes in table
    uint32_t string_arena_size;        // Total bytes in string arena
    uint32_t root_node_id;             // Root node index (typically 0)
    uint32_t generation;              // Snapshot generation counter
    uint32_t flags;                   // Capability flags
    uint32_t _reserved;               // Alignment padding
} TTWorkspaceVfsView;
```

### 2.3 `VfsNodeViewModel` (Swift UI Projection, On-Demand)

```swift
public struct VfsNodeViewModel: Identifiable, Sendable {
    public let id: UInt64              // stable_id
    public let name: String            // Decoded UTF-8 string (materialized only for visible rows)
    public let isDirectory: Bool       // Derived from file_type (S_IFDIR)
    public let uncompressedSize: UInt64
    public let childCount: UInt32
    public let modificationDate: Date
}
```

---

## 3. Snapshot Lifecycle State Machine

```mermaid
stateDiagram-v2
    [*] --> Unallocated
    Unallocated --> Allocated : mach_vm_allocate(16KB aligned)
    Allocated --> Populated : VfsSnapshotBuilder::build(&UnifiedVfsTable)
    Populated --> Leased : ttvfs_acquire_snapshot(out_view)
    Leased --> Leased : readChildren() / search() [Shared Read-Only]
    Leased --> Released : ttvfs_release_snapshot(lease) [refcount == 0]
    Released --> Deallocated : mach_vm_deallocate()
    Deallocated --> [*]
```

### State Descriptions
1. **Unallocated**: No physical memory allocated.
2. **Allocated**: Kernel reserves 16KB-aligned memory pages via `mach_vm_allocate`.
3. **Populated**: Central directory entries parsed into CSR flat array; `StringArena` filled; sort prefixes computed.
4. **Leased**: Swift UI acquires reference-counted lease (`TTVFSLease`). Multiple windows or views share immutable read-only access.
5. **Released**: All referencing UI components release lease; reference count drops to zero.
6. **Deallocated**: Memory returned directly to Darwin XNU kernel via `mach_vm_deallocate`. Zero memory retained.
