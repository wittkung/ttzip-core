# Feature Specification: CSR Zero-Copy VFS & Million-Item Low-Memory Navigation (Feature 231)

**Feature Directory**: `products/ttzip/core/specs/231-csr-zero-copy-vfs`

**Created**: 2026-10-05

**Status**: Draft

**Pipeline Tier**: [Full SDD] (modifies VFS data plane, C-ABI shared memory boundary via Mach VM, and SwiftUI presentation layer)

**Input**: Opening archives with 1,000,000 entries currently consumes 760–866 MB RAM, takes 3.48 seconds to first render, and suffers 2,318 ms latency on keystroke filtering (Issue P3). The root cause is a triple memory copying tax: Rust `Vec<UniFfiEntryMetadata>` (172 MB) -> UniFFI `RustBuffer` serialization buffer (106 MB) -> Swift `[ArchiveEntry]` array and individual string allocations (287 MB). Feature 231 executes Phase C of ADR-017 / Phase 11: Rust constructs a contiguous Compressed Sparse Row (CSR) node array with 64-byte microarchitecture-aligned POD nodes (`TTWorkspaceEntryNode`) and a monotonically appended `StringArena`, passes the memory slice to Swift through Mach VM anonymous shared memory (`mach_make_memory_entry_64`), and Swift binds on-demand view models using `VfsSnapshotReader` and `VfsSnapshotContext`.

---

## User Scenarios & Testing

### User Story 1 - Instant Load of Million-Item Archives (Priority: P1) 🎯 MVP

A user opens an archive containing 1,000,000 files and directories (such as a Linux kernel tree, large game assets bundle, or synthetic benchmark corpus). The Rust microkernel parses the central directory directly into a contiguous CSR table (`UnifiedVfsTable` / `TTWorkspaceEntryNode`) and passes the memory slice to Swift over Mach shared memory. The UI displays the root level within 300 milliseconds, and total resident memory stays under 200 MB.

**Why this priority**: Opening massive archives without memory exhaustion or multi-second freezing is a core commercial differentiator for TTZip.

**Independent Test**: Generate a synthetic 1,000,000-entry ZIP archive using the benchmark harness; invoke `AppViewState.loadArchive(path:)`; assert time to first table display $\le 300\text{ ms}$ and process resident memory (RSS) delta $\le 150\text{ MB}$.

**Acceptance Scenarios**:

1. **Given** a 1,000,000-entry ZIP archive, **When** opened in TTZip, **Then** the root directory items render within 300 ms with 0 main thread beachballs.
2. **Given** an open 1,000,000-entry archive, **When** measuring process memory via Mach task info, **Then** total memory attributable to the VFS remains $\le 200\text{ MB}$.
3. **Given** the archive loading path, **When** inspecting memory traffic across the Rust/Swift boundary, **Then** zero copies of the full entry array occur across FFI.

---

### User Story 2 - Sub-Millisecond Hierarchical Paging & Zero-Allocation Virtual Scrolling (Priority: P1)

A user navigates into a folder containing 50,000 items within the million-item archive. Using the CSR indexing model, the first child index (`first_child_id`) and count (`child_count`) provide immediate access to the contiguous slice of child nodes without tree traversal. Swift's `VfsSnapshotReader` reads only the visible 30–50 rows on screen into `VfsNodeViewModel`.

**Why this priority**: Large folders must scroll smoothly at 120 FPS without allocation churn or garbage collection pauses.

**Independent Test**: Select a folder with 50,000 child items; measure `VfsSnapshotReader.readChildren(of:in:limit:)` latency; assert retrieval time for 50 items $\le 1\text{ ms}$.

**Acceptance Scenarios**:

1. **Given** a folder containing 50,000 items, **When** the user opens or scrolls through the folder, **Then** fetching the visible 50 items executes in $\le 1\text{ ms}$.
2. **Given** continuous rapid scrolling in the file list, **When** monitoring memory allocations with Instruments, **Then** transient heap allocations stay under 2 MB per second.
3. **Given** directory expansion, **When** child items are displayed, **Then** directories and files are sorted by sort prefix (`sort_prefix`) in $O(1)$ without on-the-fly string sorting.

---

### User Story 3 - High-Throughput Keystroke Search & Filtered Slice Indexing (Priority: P1)

A user types query terms into the Omnibar or search field to filter the 1,000,000 items. Instead of allocating 1,000,000 Swift strings or evaluating lowercase transformations in SwiftUI computed properties, the search operation evaluates matching nodes in the microkernel and returns a compact index slice (`ContiguousArray<UInt32>`). Swift renders only the visible subset of matched indices.

**Why this priority**: Keystroke search latency must stay below 8.33 ms to avoid dropping frames on 120 Hz ProMotion displays.

**Independent Test**: Type a 5-character search term in an archive with 1,000,000 items; measure elapsed time until filtered table view updates; assert total latency $\le 8.33\text{ ms}$.

**Acceptance Scenarios**:

1. **Given** an active 1,000,000-entry archive, **When** the user types each character of a search query, **Then** the UI updates matching rows within 8.33 ms.
2. **Given** repeated keystrokes in quick succession, **When** new queries are issued, **Then** previous in-flight searches are cancelled cooperatively without thread starvation.
3. **Given** empty search query restoration, **When** the search query is cleared, **Then** the view instantly restores the unfiltered root slice in $O(1)$ without re-indexing.

---

### User Story 4 - Multi-Window Independent VFS Leases & Safe Snapshot Lifecycle (Priority: P2)

A user opens multiple archive windows simultaneously or switches between archives in tabs. Each window holds an independent `TTVFSLease` reference to the shared memory snapshot. When a window is closed, its `VfsSnapshotContext` releases the lease, deallocating the underlying Mach VM memory mapping and releasing memory back to 0 MB.

**Why this priority**: Memory must be reclaimed immediately upon closing archives without Mach port leaks or dangling memory mappings.

**Independent Test**: Open 3 distinct archives concurrently; close 2 of them; assert memory drops proportionally and `mach_port_deallocate` removes the Mach memory entry ports.

**Acceptance Scenarios**:

1. **Given** multiple archive windows open, **When** one window is closed, **Then** its corresponding Mach memory entry is released without affecting other open windows.
2. **Given** an archive window closing, **When** `VfsSnapshotContext.deinit` runs, **Then** `ttvfs_release_snapshot` executes and drops the lease count.
3. **Given** 100 consecutive archive open/close cycles, **When** auditing Mach port counts via `mach_port_names`, **Then** port count returns to baseline with 0 leaked ports.

---

## Functional Requirements

### 1. VFS Data Plane & CSR Representation
- **REQ-VFS-001**: Microarchitecture Node Alignment. `TTWorkspaceEntryNode` MUST be exactly 64 bytes in size and 64-byte aligned (`#[repr(C, align(64))]`), matching the Apple Silicon L1 cache line.
- **REQ-VFS-002**: Contiguous Sibling Slices. In the flattened CSR node array, all direct children of any directory node MUST be stored as a contiguous index range `[first_child_id .. first_child_id + child_count]`.
- **REQ-VFS-003**: Sort Prefix Acceleration. Each node MUST populate `sort_prefix` with the leading 8 bytes of the filename in big-endian integer format (`u64::from_be_bytes`), enabling branchless register comparisons for sorting and binary searching.
- **REQ-VFS-004**: String Arena Isolation. All filenames MUST be stored in a contiguous monotonic byte buffer (`StringArena`), indexed by `name_offset: u32` and `name_len: u16`, eliminating individual heap allocations for filenames.

### 2. Mach Shared Memory Transfer & C-ABI
- **REQ-FFI-001**: Zero-Copy Shared Memory Passing. The microkernel MUST allocate the CSR node table and string arena in Mach VM memory (`mach_vm_allocate`, 16KB page-aligned) and export the memory via `mach_make_memory_entry_64`.
- **REQ-FFI-002**: Direct In-Process Pointer Sharing. When running in the same process address space, `TTWorkspaceVfsView` MUST pass direct pointers (`nodes`, `string_arena`) to Swift without intermediary buffers.
- **REQ-FFI-003**: RAII Snapshot Lease Management. Access to `TTWorkspaceVfsView` MUST be governed by reference-counted leases (`ttvfs_lease_t`) managed by `VfsSnapshotContext`. Swift MUST NOT retain raw pointers past the lease lifetime.
- **REQ-FFI-004**: Elimination of Monolithic UniFFI Arrays. `inspect_archive_entries` returning `[UniFfiEntryMetadata]` MUST NOT be called in the production browsing path.

### 3. Swift Presentation Layer
- **REQ-UI-001**: Bounded View-Model Materialization. `VfsSnapshotReader` MUST materialize `VfsNodeViewModel` only for the requested visible slice (`limit` bounded by viewport capacity, typically $\le 50$ items).
- **REQ-UI-002**: Non-Copying Filtered View. Search filtering MUST generate index arrays (`ContiguousArray<UInt32>`); the table view MUST resolve items by indexing into the CSR snapshot on-demand.
- **REQ-UI-003**: MainActor Isolation. View models and state updates in `ArchiveExplorerState` MUST execute on `@MainActor`, while search evaluation and index scanning run in cooperative background tasks.

---

## Success Criteria

| ID | Metric | Target | Baseline (Legacy) |
|---|---|---|---|
| **SC-001** | 1,000,000-entry archive load time to first frame | $\le 300\text{ ms}$ | 3,480 ms |
| **SC-002** | 1,000,000-entry archive total VFS memory (RSS) | $\le 200\text{ MB}$ | 760–866 MB |
| **SC-003** | Keystroke search latency on 1,000,000 entries | $\le 8.33\text{ ms}$ | 2,318 ms |
| **SC-004** | Cross-FFI entry array copy volume | 0 bytes | 278 MB |
| **SC-005** | Memory leak after 100 open/close cycles | 0 bytes ($\le 2\text{ MB}$ noise) | Unbounded |

---

## Assumptions & Boundaries

1. **Single Truth Source**: `TTWorkspaceEntryNode` defined in `products/ttzip/core/rust/crates/ttzip-types/src/node.rs` and bridged in `products/ttzip/apple/Sources/TTZipApp/Views/VfsSnapshotReader.swift` is the sole canonical memory layout.
2. **Read-Only Inspection**: Phase C addresses archive inspection and browsing; write mutations (e.g. adding files to an existing archive) will build new snapshots rather than in-place mutation of the immutable CSR buffer.
3. **Platform Target**: macOS 14.0+ on Apple Silicon (ARM64) and x86_64; 16KB page alignment is required on Apple Silicon, fallback to standard page size on x86_64.

---

## Clarifications & Architectural Decisions (speckit-clarify)

### Decision 1: 100% Unified CSR Zero-Copy Ingestion (Resolved 2026-10-05)
- **Scope**: All archives, regardless of entry count (from 1 entry to 1,000,000 entries), strictly route through the CSR zero-copy VFS snapshot pipeline (`TTWorkspaceVfsView` / `VfsSnapshotReader`).
- **Rationale**: Eliminates dual-track maintenance, eradicates monolithic UniFFI `[UniFfiEntryMetadata]` array serialization across the FFI boundary, and guarantees consistent memory behavior.
- **Impact**: The legacy `inspectArchiveEntries` array-based inspection path in `ArchiveReader.swift` is decommissioned from production browsing.

### Decision 2: Rust Microkernel SIMD In-Place Search (Resolved 2026-10-05)
- **Scope**: Real-time keystroke search and filtering execute directly inside the Rust microkernel over the contiguous `StringArena`.
- **Rationale**: Scanning contiguous memory using SIMD in Rust avoids allocating 1,000,000 Swift String instances or building complex client-side indexes. It returns only a compact slice of matching 32-bit node IDs (`ContiguousArray<UInt32>`).
- **Impact**: Swift UI renders only the visible rows corresponding to the returned indices, guaranteeing keystroke filtering latency $\le 8.33\text{ ms}$ (120 FPS).

