# Research & Architecture Decisions: CSR Zero-Copy VFS (Feature 231)

**Feature Directory**: `products/ttzip/core/specs/231-csr-zero-copy-vfs`

---

## R1: Memory Layout & Zero-Copy Topology (CSR vs Linked Trees)

### Problem
Monolithic linked trees or array serialization across FFI boundaries (e.g. `[UniFfiEntryMetadata]` or `[ArchiveEntry]`) cause massive memory duplication:
- Rust `Vec`: 172 MB
- UniFFI `RustBuffer`: 106 MB
- Swift `[ArchiveEntry]` + individual String objects: 287 MB
Total memory reaches 760–866 MB for 1,000,000 entries. Furthermore, individual heap allocations cause ARC/GC scanning overhead and L1/L2 cache misses.

### Decision
Adopt a Compressed Sparse Row (CSR) flat array architecture:
1. **Contiguous Node Array**: Nodes are stored in a contiguous array of `TTWorkspaceEntryNode`, each strictly 64 bytes in size and 64-byte aligned (`#[repr(C, align(64))]`).
2. **Contiguous Slices**: A parent directory node points to its children via `first_child_id: u32` and `child_count: u32`. All direct children occupy a contiguous slice `[first_child_id .. first_child_id + child_count]`.
3. **Monotonic String Arena**: Filenames are stored in a contiguous byte buffer (`StringArena`). Nodes store `name_offset: u32` and `name_len: u16`.
4. **Sort Prefix**: Nodes store an 8-byte big-endian prefix `sort_prefix: u64` derived from the first 8 bytes of the filename, enabling branchless register comparisons for sorting.

### Physical Footprint
- 1,000,000 nodes $\times$ 64 bytes = 64.0 MB
- 1,000,000 filenames (average 28 bytes) in `StringArena` = ~28.0 MB
- Total contiguous memory: ~92.0 MB (well below the 200 MB budget, down from 866 MB).

### Source
`products/ttzip/core/rust/crates/ttzip-types/src/node.rs`, `products/ttzip/core/rust/crates/ttzip-types/src/builder.rs`.

---

## R2: Mach VM Shared Memory & Address Space Governance

### Problem
Passing large memory buffers across process boundaries or between runtimes without copying requires operating system virtual memory primitives. Standard heap memory (`malloc`) has fragmentation overhead and cannot be safely remapped via Mach ports without copying.

### Decision
Use Darwin Mach VM primitives:
1. **Allocation**: Allocate contiguous memory via `mach_vm_allocate` rounded up to the hardware page size (`16384` bytes on Apple Silicon M-series).
2. **Zero-Copy In-Process Sharing**: When Rust and Swift reside in the same process space, `TTWorkspaceVfsView` directly references `nodes` and `string_arena` raw pointers without intermediate translation.
3. **Cross-Process Capability**: `mach_make_memory_entry_64` generates a `mach_port_t` handle (`MAP_MEM_VM_COPY` / `VM_PROT_READ`), allowing instant Copy-On-Write sharing across XPC or sub-processes if needed.
4. **RAII Lease**: Swift wraps the view in `VfsSnapshotContext`, holding a `TTVFSLease`. On `deinit`, `ttvfs_release_snapshot` decrements the reference count and deallocates the Mach VM pages via `mach_vm_deallocate`.

### Source
`products/ttzip/core/rust/crates/ttzip-types/src/mach_vm.rs`, Darwin XNU kernel `mach_vm.defs`.

---

## R3: Keystroke Search & SIMD In-Place Filtering

### Problem
Filtering 1,000,000 items on every keystroke by transforming strings to lowercase in Swift computed properties takes 2,318 ms, completely dropping UI frames (120 Hz ProMotion requires $\le 8.33\text{ ms}$).

### Decision
Execute search scanning directly in the Rust microkernel over the contiguous `StringArena`:
1. **8-Byte Prefix Fast Path**: For prefix searches $\le 8$ characters, compare `sort_prefix: u64` in CPU registers without touching `StringArena`.
2. **SIMD Substring Scan**: For general substring queries, scan the contiguous `StringArena` using SIMD/NEON vectorized byte matching.
3. **Index-Only Return**: Rust returns only matching 32-bit node indices (`&[u32]` / `ContiguousArray<UInt32>`).
4. **On-Demand View Binding**: Swift receives the matching indices array (for 10,000 matches, just 40 KB of memory) and materializes `VfsNodeViewModel` only for the 30–50 rows visible in the viewport.

### Source
`products/ttzip/core/rust/crates/ttzip-types/src/node.rs` lines 63–75 (`compute_sort_prefix`), `products/ttzip/apple/Sources/TTZipApp/Views/VfsSnapshotReader.swift`.

---

## R4: SwiftUI Viewport Decoupling & Virtualization

### Problem
SwiftUI `Table` or `List` attempting to bind to an array of 1,000,000 `@Observable` elements causes significant UI latency due to view identity tracking and layout computation.

### Decision
1. **Viewport-Bounded Slicing**: `ArchiveExplorerState` holds the active `VfsSnapshotContext` and the filtered index array (`filteredIndices: ContiguousArray<UInt32>?`).
2. **On-Demand Row Materialization**: SwiftUI table rows query `VfsSnapshotReader.readChildren` or index lookups with an explicit slice limit, allocating Strings solely for onscreen cells.
3. **Cooperative Cancellation**: Rapid typing cancels prior search Tasks cooperatively (`Task.checkCancellation()`), preventing background task stampedes.

### Source
`products/ttzip/apple/Sources/TTZipApp/ViewModels/AppSubStates.swift:334-430`, `products/ttzip/apple/Sources/TTZipApp/Views/VfsSnapshotReader.swift`.
