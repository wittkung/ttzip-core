# Implementation Plan: CSR Zero-Copy VFS & Million-Item Low-Memory Navigation (Feature 231)

**Feature Directory**: `products/ttzip/core/specs/231-csr-zero-copy-vfs`

**Classification**: `[Full SDD]`

**Status**: Ready for Tasks

---

## 1. Technical Context & Baseline

Opening archives with 1,000,000 entries currently consumes 760–866 MB RAM, takes 3.48 seconds to first render, and suffers 2,318 ms latency on keystroke filtering (Issue P3). The root cause is a triple memory copying tax:
1. Rust `Vec<UniFfiEntryMetadata>` (172 MB)
2. UniFFI `RustBuffer` serialization buffer (106 MB)
3. Swift `[ArchiveEntry]` array and individual string allocations (287 MB)

Feature 231 executes Phase C of ADR-017 / Phase 11:
- Rust constructs a contiguous Compressed Sparse Row (CSR) node array with 64-byte microarchitecture-aligned POD nodes (`TTWorkspaceEntryNode`) and a monotonically appended `StringArena`.
- Rust passes the memory slice to Swift through Mach VM anonymous shared memory (`mach_make_memory_entry_64`).
- Swift binds on-demand view models using `VfsSnapshotReader` and `VfsSnapshotContext`.
- Keystroke search filtering scans `StringArena` directly in Rust using SIMD and returns only a compact `ContiguousArray<UInt32>` of matching node indices.

---

## 2. Constitution & Architectural Invariants Check

| Principle / Gate | Status | Evidence / Verification |
|---|---|---|
| **P1: Minimalist Core & Zero Copy** | **PASS** | Monolithic `[ArchiveEntry]` arrays eliminated from browsing path. Swift borrows directly from Mach VM memory slice. |
| **P2: Microarchitecture Alignment** | **PASS** | `TTWorkspaceEntryNode` is strictly 64 bytes (`#[repr(C, align(64))]`), matching Apple Silicon L1 cache line with 0 alignment holes. |
| **P3: Single-File LOC Gate ($\le 800$ LOC)** | **PASS** | All target files audited. Sub-components factored into focused modules (`search.rs`, `builder.rs`, `reader.swift`). |
| **P4: Zero-Warning Hard Gate** | **PASS** | Clippy `-D warnings` and Swift 6 strict concurrency enforcement. |
| **P5: Contract Rigor Gate** | **PASS** | All JSON Schema contracts conform to Draft-07, zero bare objects, verified via `lint-contracts.sh`. |

---

## 3. Implementation Phases

### Phase 1: Rust Microkernel CSR Table Generation & Mach VM Export
- **Scope**: `crates/ttzip-types`, `crates/ttzip-archive`, `ttzip-engine`.
- **Key Tasks**:
  1. In `ttzip-archive`, ensure ZIP central directory, TAR headers, and 7z headers populate `UnifiedVfsTable` directly without intermediate metadata allocations.
  2. In `ttzip-types/src/builder.rs`, ensure `VfsSnapshotBuilder::build` outputs CSR flat node array in 16KB page-aligned `MachVmAllocation`.
  3. In `ttzip-engine`, export `ttzip_vfs_open_snapshot(archive_path, password)` returning `*mut TTVfsLease` and `TTWorkspaceVfsView`.

### Phase 2: Microkernel SIMD In-Place Search
- **Scope**: `crates/ttzip-types`, `ttzip-engine`.
- **Key Tasks**:
  1. Implement `vfs_search(lease, query, case_sensitive, max_results) -> Vec<u32>` in Rust:
     - Check 8-byte `sort_prefix: u64` in CPU register for fast prefix filtering.
     - Scan contiguous `StringArena` with SIMD/NEON vectorization for substring matching.
  2. Expose search C-ABI / UniFFI endpoint delivering matching node index slices directly into Swift.

### Phase 3: Swift FFI Bridge & Lease RAII Lifecycle
- **Scope**: `TTZipCore/ArchiveProtocols.swift`, `VfsSnapshotContext.swift`, `VfsSnapshotReader.swift`.
- **Key Tasks**:
  1. Wrap `TTVfsLease` in `VfsSnapshotContext: Sendable`, ensuring `deinit` automatically releases the Mach VM lease.
  2. Connect `VfsSnapshotReader.readChildren` and `readSlice` for $O(1)$ indexed child traversal.
  3. Implement Swift 6 non-escaping borrowing closures to prevent raw pointers escaping past snapshot lifetime.

### Phase 4: SwiftUI Presentation Layer Integration
- **Scope**: `TTZipApp/ViewModels/AppSubStates.swift`, `AppViewState+ArchiveOperations.swift`, `HomeExplorerContainerView.swift`.
- **Key Tasks**:
  1. Update `ArchiveExplorerState` to hold `VfsSnapshotContext` instead of monolithic `currentEntries: [ArchiveEntry]`.
  2. Rewire `loadArchive` to call `TTZipEngineFacade.shared.openVfsSnapshot(path:password:)`.
  3. Wire search input to the microkernel search pipeline with debounced cooperative Task cancellation.
  4. Ensure virtualized table view renders rows on-demand from the active snapshot context.

### Phase 5: Verification, Benchmarking & Gate Enforcement
- **Scope**: Benchmarks, CI tests, gate audits.
- **Key Tasks**:
  1. Implement `bench_vfs_million_items.rs` measuring load time ($\le 300\text{ ms}$), RSS memory ($\le 200\text{ MB}$), and search latency ($\le 8.33\text{ ms}$).
  2. Decommission legacy `inspectArchiveEntries` production call sites.
  3. Verify all Bazel and Cargo tests pass 100% green.
  4. Enforce LOC gate ($\le 800$ LOC) and static quality gates.

---

## 4. Risk Analysis & Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| **Raw Pointer Escape in Swift** | High (Crash / UAF) | Swift 6 `~Copyable, ~Escapable` design and RAII closure scopes (`withSnapshot`) prevent pointers escaping beyond lease lifetime. |
| **Mach Port Exhaustion on Rapid Cycles** | Medium (System Resource Leak) | Immediate `mach_port_deallocate` after mapping; RAII `MachPortGuard` ensures automatic cleanup on drop. |
| **Search Contention During Keystrokes** | Low (UI Lag) | Cooperative `Task.checkCancellation()` cancels prior search queries before spawning new ones; search runs in parallel thread pool. |
