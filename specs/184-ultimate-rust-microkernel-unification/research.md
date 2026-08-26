# Phase 0 Research: 184-ultimate-rust-microkernel-unification

## Research Item R001: Rust Unified Archive Orchestration Architecture
- **Decision**: Implement `rust/ttzip-glue/src/archive/unified.rs` with unified format dispatch, stream pipeline allocation, thermal-aware core scaling, and progress callbacks across all 17 formats.
- **Rationale**: 
  - Centralizes format autodetection, container wrapping, and error handling in Safe Rust.
  - Eliminates duplicated glue code across CLI, TUI, and Swift.
- **Alternatives Considered**: 
  - *Keep separate format dispatchers in each front-end*: Incur high maintenance and subtle behavioral discrepancies.
- **Source**: 
  - `rust/ttzip-glue/src/ffi/archive_ffi/mod.rs`
  - `Sources/TTZipCore/ArchiveWriter+ZipDispatch.swift`
  - `Sources/TTZipCore/ArchiveWriter+TarSevenZipDispatch.swift`

---

## Research Item R002: Rust Unified VFS Tree & Search
- **Decision**: Implement `rust/ttzip-glue/src/fs/vfs.rs` supporting hierarchical node indexing, parent-child resolution, ASCII/Unicode tree formatting, and fuzzy filtering.
- **Rationale**: 
  - Replaces Swift OOP tree structures with zero-copy, cache-friendly Rust BTree/Arena nodes.
- **Alternatives Considered**: 
  - *Keep Swift `ArchiveComponentProtocol` hierarchy*: Allocates thousands of ARC objects for large archives.
- **Source**: 
  - `rust/ttzip-tui/src/vfs/mod.rs`
  - `Sources/TTZipCore/ArchiveComponentProtocol.swift`
