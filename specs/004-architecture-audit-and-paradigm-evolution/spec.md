# Feature Specification: Full Architectural Audit, Defect Analysis, and Paradigm Evolution

- **Feature ID**: `004-architecture-audit-and-paradigm-evolution`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `DRAFT`
- **Created**: 2026-08-24
- **Target Subsystems**: `ttzip-engine` (Rust Core), `CTTZipBridge` (C-ABI Layer), `TTZipCore` (Swift 6 SDK), `TTZipApp` (SwiftUI / AppKit Presentation Layer), `CI/CD Quality Gates`

---

## 1. Executive Summary & Audit Findings

A comprehensive architectural audit across the entire TTZip codebase (Rust core engine, C-ABI bridge, Swift 6 SDK, and SwiftUI/AppKit application) identified structural bottlenecks, memory scalability risks, design pattern anti-patterns, and opportunities for fundamental paradigm shifts:

### Audit Finding 1: Solid Decompression & Parallel Compression Memory Scaling Flaws ($O(N)$ RAM Blowup)
- **Defect**: In `sevenz/decoder/payload.rs` (line 66), solid 7z block decompression allocates the entire uncompressed stream into heap memory (`vec![0u8; expected_unpack_size]`). Similarly, `zip/writer/streaming_parallel.rs` (line 322) reads entire files into memory via `fs::read(&item.abs_path)`.
- **Impact**: Multi-gigabyte archives or archive bombs immediately trigger multi-gigabyte heap allocations, causing process memory blowups, swap thrashing, or OOM crashes.
- **Paradigm Shift**: **Bounded-Memory Streaming Filter Engine ($O(1)$ RAM)**. Replace whole-file buffers with fixed-size streaming ring buffers (64KB/1MB windows) directly connected to file descriptors via `pwrite` and `pread`.

### Audit Finding 2: Sequential libarchive Fallback in Unified Extraction vs True Work-Stealing Parallel DAG
- **Defect**: In `archive/unified/extract.rs`, standard archive extractions route through sequential single-threaded `libarchive` (`archive_read_next_header` + `archive_read_data`) in 64KB synchronous chunks. Multi-core parallelism is only utilized in isolated zip reader paths.
- **Impact**: Modern Apple Silicon multi-core CPUs (M1/M2/M3/M4 with 8–24 cores) are underutilized during extraction of archives containing thousands of independent files.
- **Paradigm Shift**: **Parallel Work-Stealing Extraction DAG Engine**. Parse the central directory/catalog in $O(1)$ time, construct a lock-free extraction DAG, preallocate APFS extents in parallel, and dispatch independent file decompression tasks across an adaptive thread pool directly using positional I/O (`pwrite`).

### Audit Finding 3: VFS Quadratic Directory Lookups and Heavyweight Heap Trees
- **Defect**: In `fs/vfs/tree.rs`, `VfsTree::insert` performs `curr.children.iter().position(...)` (linear scan per path segment), causing $O(N^2)$ quadratic slowdowns when inserting tens of thousands of sibling files. Furthermore, `VfsNode` stores separate heap-allocated `String` instances (`name`, `full_path`), leading to cache miss storms.
- **Impact**: Building or searching VFS trees for 100,000+ files consumes excessive RAM and CPU cycles.
- **Paradigm Shift**: **Flat Arena-Allocated VFS Index with Packed String Interning**. Replace pointer-chasing tree structures with a contiguous struct-of-arrays (SoA) arena, string intern pool, and $O(1)$ hash-indexed directory sibling chains.

### Audit Finding 4: Cross-Language FFI String Marshalling & Dual-Tree Redundancy
- **Defect**: In `RustVfsSession.init`, Swift executes `entries.map { strdup($0.path) }` (N separate heap allocations), passes pointers across FFI, and Rust reallocates them into `String` (`path_str.to_string()`). Simultaneously, Swift maintains `ArchiveTreeStore` and `entryMap` in parallel with Rust's `VfsTree`.
- **Impact**: Redundant memory consumption (2x–3x duplication of file tree metadata across the language boundary) and microsecond latency spikes during archive load.
- **Paradigm Shift**: **Zero-Copy Packed String Array ABI & Unified VFS Paging Adapter**. The Rust engine maintains the single authoritative VFS tree; Swift accesses hierarchical views and windowed slices via lightweight C-ABI offsets without copying strings or duplicating trees.

### Audit Finding 5: Ephemeral Resource & Cache Fragmentation in Swift UI Layer
- **Defect**: Five independent, uncoordinated caching subsystems exist (`ExplorerLRUCache`, `EphemeralPreviewCacheManager`, `ImageIOThumbnailCache`, `ImageMetadataCache`, `PreviewLRUCacheManager`), each maintaining separate memory pools without unified eviction coordination under system memory pressure.
- **Impact**: Simultaneous previewing and browsing can exceed memory budgets under constrained macOS environments.
- **Paradigm Shift**: **Unified Memory & Ephemeral Resource Broker**. Centralize all caching behind an actor-isolated memory coordinator listening to Darwin `DISPATCH_SOURCE_TYPE_MEMORYPRESSURE` events.

---

## 2. User Stories & Acceptance Scenarios

### User Story 1: Bounded-Memory Multi-Core Parallel Streaming Extraction (Priority: P1)
**As an** end-user extracting large solid 7z or multi-gigabyte ZIP archives on macOS,  
**I want** extraction to execute across all available CPU cores with strictly bounded memory consumption ($\le 64\text{MB}$ RSS),  
**So that** extraction completes in minimal time without system slowdowns, swap thrashing, or OOM crashes.

- **Scenario 1.1 (Parallel Extraction DAG)**: Extracting a 50,000-file ZIP archive distributes decompression work across all available CPU cores using work-stealing, achieving $>500\text{ MB/s}$ throughput on NVMe APFS.
- **Scenario 1.2 (7z Solid Streaming)**: Decompressing a 20GB solid 7z archive consumes $\le 64\text{MB}$ peak RSS throughout the entire extraction lifecycle.
- **Scenario 1.3 (APFS Parallel Preallocation)**: Disk file extents are preallocated in parallel via `fstore_t` before write dispatch, eliminating file fragmentation on Apple Silicon SSDs.

### User Story 2: Arena-Allocated Zero-Allocation VFS Engine (Priority: P1)
**As a** user browsing and searching an archive with 200,000 files in the GUI or CLI,  
**I want** tree construction to take $<50\text{ms}$ and fuzzy searches to execute with zero heap allocations in $<2\text{ms}$,  
**So that** the UI remains silky smooth at 120 FPS during interactive omnibar typing.

- **Scenario 2.1 (O(N) Arena Construction)**: Constructing the VFS tree for 200,000 entries completes in $<50\text{ms}$ with zero quadratic directory scans.
- **Scenario 2.2 (Zero-Copy FFI Marshalling)**: Swift passes archive metadata to Rust using a single contiguous `PackedStringArray` buffer, eliminating individual `strdup` and `free` invocations.
- **Scenario 2.3 (Zero Heap Search)**: Interactive fuzzy search queries execute directly against the arena index with 0 heap allocations, returning paginated `TTZipVfsMatchDto` records into a caller-provided buffer.

### User Story 3: Swift 6 Modernized Observation & Unified Resource Broker (Priority: P2)
**As a** macOS native application user,  
**I want** thumbnails, file previews, and archive navigation to respond instantaneously without UI micro-stutters or memory leaks,  
**So that** the desktop experience is seamless across M-series laptops and Mac Studio workstations.

- **Scenario 3.1 (Modern Observation Architecture)**: State models utilize Swift 6 `@Observable` macros with fine-grained view invalidation, eliminating redundant `@MainActor` hops and Combine publisher overhead.
- **Scenario 3.2 (Unified Memory Broker Coordination)**: Under system memory pressure warnings, the unified cache broker automatically evicts inactive preview files and thumbnail textures down to target thresholds within $\le 50\text{ms}$.
- **Scenario 3.3 (Zero-Copy Paging in UI)**: The `NSOutlineView` / SwiftUI browser loads directory children on-demand via Rust VFS slice handles without pre-materializing all archive entries in Swift memory.

### User Story 4: Defensive Hardening, Constant-Time Crypto & Anti-Bomb Protection (Priority: P2)
**As a** security-conscious enterprise user opening untrusted archives from the internet,  
**I want** TTZip to actively detect and neutralize archive bombs, path traversal attacks, and symlink race conditions,  
**So that** malicious archives can never compromise system security or exhaust disk/memory resources.

- **Scenario 4.1 (Real-Time Expansion Ratio Guard)**: Decompressing an archive bomb (e.g. 1000:1 expansion ratio) immediately aborts with `TTZIP_STATUS_ERR_SECURITY_VIOLATION` upon exceeding bounded limits.
- **Scenario 4.2 (Descriptor-Relative Safe I/O)**: File and directory creation utilizes descriptor-relative operations (`openat`, `mkdirat`, `symlinkat` with `O_NOFOLLOW`) to prevent symlink TOCTOU race conditions.
- **Scenario 4.3 (Constant-Time Verification & Zeroization)**: All AES key derivations, WinZip PVV checks, and PBKDF2 iterations run in constant time, with all sensitive key buffers zeroed via `zeroize` upon completion.

---

## 3. Functional Requirements (FR-01 to FR-20)

### Streaming Engine & Concurrency Architecture
- **FR-01**: The 7z decompression engine MUST decode solid blocks in bounded streaming chunks ($\le 4\text{MB}$ chunk size) directly into file writers, bounding peak RSS to $\le 64\text{MB}$.
- **FR-02**: The parallel ZIP writer (`streaming_parallel.rs`) MUST process large files via streaming chunked readers without reading entire files into heap memory (`fs::read`).
- **FR-03**: The unified archive extraction orchestrator MUST implement a **Work-Stealing Parallel DAG Extractor** for independent non-solid archive entries.
- **FR-04**: The engine MUST utilize a long-lived, adaptive Rayon thread pool matching Apple Silicon P-core/E-core topology rather than recreating thread pools per operation.
- **FR-05**: Progress reporting across all parallel pipelines MUST be rate-limited to $\le 60\text{Hz}$ using monotonic nanosecond clocks while guaranteeing atomic keyframes ($0\%$ and $100\%$).

### VFS Engine & C-ABI Hardening
- **FR-06**: `VfsTree` MUST be restructured into an **Arena-Allocated Struct-of-Arrays (SoA)** index with hash-indexed directory chains, achieving $O(N)$ total construction time.
- **FR-07**: The C-ABI layer MUST provide `PackedStringArray` (`*const u8` data buffer + `*const u32` offsets) for batch string marshalling across FFI, eliminating per-path `strdup` calls.
- **FR-08**: `RustVfsSession` MUST provide a zero-copy paging API allowing Swift UI components to query directory children by range `[offset, count]` without materializing global entry arrays.
- **FR-09**: All fallible C-ABI endpoints MUST accept caller-allocated `*mut TTZipErrorInfo` structs for deterministic, thread-safe error reporting without TLS dependencies.
- **FR-10**: The C-ABI symbol manifest verification gate MUST enforce 100% bidirectional symbol and layout parity between headers, Swift bindings, and static library text symbols.

### Swift 6 Architecture & Resource Management
- **FR-11**: Domain state management MUST migrate to Swift 6 `@Observable` macro architecture, deprecating monolithic `ObservableObject` forwarding patterns.
- **FR-12**: All caching subsystems (`ImageIOThumbnailCache`, `PreviewLRUCacheManager`, `ExplorerLRUCache`) MUST be unified under a single `EphemeralResourceBroker` with centralized memory pressure handling.
- **FR-13**: Blocking native FFI calls MUST be isolated to dedicated compute executors outside the Swift 6 cooperative thread pool.
- **FR-14**: File preview extraction MUST use direct in-memory stream decoding with exact buffer sizing, eliminating fixed 32MB buffer over-allocations.

### Security, Cryptography & Defensive Invariants
- **FR-15**: The extraction pipeline MUST enforce a **Streaming Expansion Ratio Guard** that halts decompression if the uncompressed-to-compressed byte ratio exceeds $1000:1$ and total extracted size exceeds $100\text{MB}$.
- **FR-16**: File system operations during extraction MUST enforce descriptor-relative calls (`openat`, `mkdirat`, `fchmodat`) with `O_NOFOLLOW` to prevent symlink TOCTOU races.
- **FR-17**: All cryptographic routines (AES-GCM, WinZip AES, 7z KDF, Argon2) MUST execute in constant-time and enforce memory zeroization (`zeroize`) on drop.
- **FR-18**: In-place archive mutations MUST execute via transactional atomic copy-on-write APFS shadow files with automatic rollback on error or cancellation.

### CI/CD Quality Gates & Performance Governance
- **FR-19**: The CI pipeline MUST enforce clean execution under AddressSanitizer (ASan), ThreadSanitizer (TSan), and UndefinedBehaviorSanitizer (UBSan).
- **FR-20**: The release gate MUST execute automated statistical A/B benchmarking in isolated worktrees, failing builds that exhibit statistically significant regressions ($>3\%$ throughput delta, $p < 0.05$).

---

## 4. Success Criteria

- **SC-01 (Peak Memory Boundedness)**: Peak process RSS remains $\le 64\text{MB}$ during the extraction of solid 7z or ZIP archives of any size (tested up to 50GB).
- **SC-02 (Multi-Core Extraction Scalability)**: Parallel extraction achieves $\ge 3.5\times$ speedup on 8-core Apple Silicon systems compared to single-threaded extraction for archives containing $>1000$ files.
- **SC-03 (VFS Construction & Search Speed)**: Constructing the VFS tree for 100,000 entries takes $\le 30\text{ms}$; zero-allocation fuzzy search returns results in $\le 2\text{ms}$ with 0 heap allocations.
- **SC-04 (Zero TLS Leaks & 100% C-ABI Parity)**: 0 occurrences of TLS-backed error retrieval in the codebase; 100% of C-ABI symbols verified via automated CI symbol gates.
- **SC-05 (UI Smoothness & Zero Frame Drops)**: Main actor maintains steady 60/120 FPS without frame drops during heavy background compression and extraction workloads.
- **SC-06 (Automated Zero-Regression Release Gate)**: Automated A/B performance validation confirms zero throughput regressions across all supported compression codecs.

---

## 5. Non-Functional & Systemic Governance Constraints

- **Language Standards**: Rust 2021 Edition with zero unsafe UB; Swift 6 with `-strict-concurrency=complete`.
- **Target Platforms**: macOS 13.0+ (Ventura, Sonoma, Sequoia) on Apple Silicon (ARM64) and Intel (x86_64) Universal Binaries.
- **File Length Standard**: All source files MUST strictly adhere to the single-file size threshold of $\le 800$ LOC (target $\le 350$ LOC).
- **Tooling & Licensing**: GPL-3.0-or-later for application/frontend layer; BSD-3-Clause OR Apache-2.0 for core Rust engine and C-ABI bridge.
