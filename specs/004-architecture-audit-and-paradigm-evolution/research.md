# Technical Research & Architectural Decisions: Full Paradigm Evolution

- **Feature ID**: `004-architecture-audit-and-paradigm-evolution`
- **Target Subsystems**: `ttzip-engine` (Rust Core), `CTTZipBridge` (C-ABI Layer), `TTZipCore` (Swift 6 SDK), `TTZipApp` (SwiftUI / AppKit Presentation Layer), `CI/CD Quality Gates`
- **Status**: `COMPLETE`

---

## 1. Research Topic 1: Bounded-Memory Solid 7z Decompression ($O(1)$ RAM Sliding Ring Buffer)

### Technical Analysis
In `sevenz/decoder/payload.rs:66`, `let mut unpack_buf = vec![0u8; expected_unpack_size]` allocates an in-memory vector sized to the total uncompressed solid block. For 20GB+ archives or zip bombs, this triggers allocator failure, memory blowup, or kernel swap death.

### Architectural Decision
- **Decision**: Implement a **Streaming Sliding Ring Buffer Decoder** with direct file descriptor dispatch (`pwrite`).
- **Rationale**: Solid block decompression requires LZMA2 dictionary continuity across file boundaries, but does NOT require keeping decompressed data in heap memory once written to disk. A 4MB sliding buffer provides sufficient window size while bounding process RSS to $\le 64\text{MB}$.
- **Alternatives Considered**:
  - *Per-file separate decompression*: Impossible in solid archives because LZMA2 dictionary references preceding files.
  - *Mmap temporary scratch file*: Amplifies disk write I/O by $2\times$ (scratch write + final copy).

---

## 2. Research Topic 2: Work-Stealing Parallel Extraction DAG with Persistent Engine Thread Pool

### Technical Analysis
`archive/unified/extract.rs` currently falls back to `libarchive`'s single-threaded sequential read loop (`archive_read_next_header` + `archive_read_data`), underutilizing multi-core Apple Silicon hardware. Additionally, `zip/reader.rs` repeatedly creates and destroys `rayon::ThreadPoolBuilder::new().build()` on each invocation.

### Architectural Decision
- **Decision**: Implement a **Work-Stealing Parallel Extraction DAG** orchestrated over a **Global Persistent Topology-Aware Thread Pool** (`EngineThreadPool`).
- **Rationale**: For non-solid ZIP and multi-file archives, all file streams have known offsets in `ArchiveSource`. An initial catalog scan ($<10\text{ms}$) generates an extraction DAG. Worker threads parallelly issue `apfs_preallocate` and decompress file chunks directly via positional `pread` and `pwrite`. The global thread pool matches Apple Silicon P-core/E-core asymmetric counts.
- **Alternatives Considered**:
  - *Per-operation thread pool*: Rejected due to measurable pthread creation latency (~2–5ms per extraction).
  - *Grand Central Dispatch (libdispatch) C-FFI*: Rejected because Rayon provides superior work-stealing load balancing across deep CPU core pipelines.

---

## 3. Research Topic 3: Flat Arena-Allocated VFS Index with Packed String Interning

### Technical Analysis
`fs/vfs/tree.rs:81-99` performs `curr.children.iter().position(...)` per path segment, degrading to $O(N^2)$ quadratic complexity on large flat directories. `VfsNode` recursively allocates separate `String` instances and `Vec<VfsNode>` children, creating $>300,000$ heap allocations for 100,000 files.

### Architectural Decision
- **Decision**: Replace `VfsTree` / `VfsNode` with a **Flat Arena Struct-of-Arrays (SoA)** index (`VfsArena`) backed by a contiguous `string_arena: Vec<u8>` string interning pool and 32-bit `NodeId` references (`first_child_id`, `next_sibling_id`).
- **Rationale**: Arena allocation guarantees contiguous memory locality, hardware prefetching, and zero heap allocations during traversal. Build-time `FxHashMap<(NodeId, &str), NodeId>` resolves directory paths in strict $O(N)$ time. ARM64 NEON vector instructions can directly scan `string_arena` in parallel for $<1\text{ms}$ fuzzy searches.
- **Alternatives Considered**:
  - *Trie / Radix Tree*: High pointer overhead and poor cache line packing compared to flat arena arrays.
  - *SQLite In-Memory*: Unnecessary SQL query parser overhead and foreign-language runtime complexity.

---

## 4. Research Topic 4: Zero-Copy PackedStringArray C-ABI & Single Source of Truth VFS Adapter

### Technical Analysis
`RustVfsSession.swift:29` calls `entries.map { strdup($0.path) }` followed by `free`, producing $2N$ heap allocator calls across FFI. Simultaneously, Swift and Rust both duplicate full directory tree data structures (`ArchiveTreeNode` / `ArchiveTreeStore` vs `VfsTree`).

### Architectural Decision
- **Decision**: Introduce `TTZipPackedEntryArray` (single contiguous UTF-8 buffer + offset/length arrays) and make Rust `VfsArena` the **Single Source of Truth (SSOT)**.
- **Rationale**: Swift passes metadata in a single contiguous memory block in $O(1)$ FFI calls. The Swift presentation layer (`NSOutlineView` / SwiftUI) queries Rust on-demand via a windowed slice C-ABI (`ttzip_rust_vfs_get_children(dir_id, offset, limit)`), completely eliminating Swift-side tree duplication.
- **Alternatives Considered**:
  - *JSON serialization*: High CPU decoding penalty and string allocations.
  - *FlatBuffers / Protocol Buffers*: Adds heavy external code-generation dependencies.

---

## 5. Research Topic 5: Swift 6 `@Observable` State & Centralized Ephemeral Resource Broker

### Technical Analysis
`AppViewState` acts as a monolithic God-object with 35+ forwarding computed properties and redundant `@MainActor.run` hops. Five uncoordinated caching classes (`ExplorerLRUCache`, `EphemeralPreviewCacheManager`, `ImageIOThumbnailCache`, `ImageMetadataCache`, `PreviewLRUCacheManager`) operate independently without system memory pressure integration.

### Architectural Decision
- **Decision**: Refactor state into independent Swift 6 `@Observable` sub-state models (`NavigationState`, `TaskExecutionState`, etc.) and centralize caching in an actor-isolated `EphemeralResourceBroker` hooked into `DISPATCH_SOURCE_TYPE_MEMORYPRESSURE`.
- **Rationale**: `@Observable` performs fine-grained observation tracking, invalidating only the views that read mutated properties. `EphemeralResourceBroker` automatically purges thumbnails and preview caches upon receiving Darwin kernel memory pressure notifications, preventing app jetsam kills.
- **Alternatives Considered**:
  - *Per-cache memory pressure observers*: Duplicates notification setup across 5 services and leads to uncoordinated eviction priorities.
