# Technical Execution Plan: TTZip Systemic Architecture & Engineering Governance

- **Feature ID**: `002-systemic-architecture-and-governance`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `READY`
- **Specification**: [spec.md](./spec.md)
- **Research Decisions**: [research.md](./research.md)
- **Data Model**: [data-model.md](./data-model.md)
- **Contracts**: [contracts/](./contracts/)
- **Quickstart**: [quickstart.md](./quickstart.md)

---

## 1. Executive Summary & Architecture Blueprint

This plan establishes systemic engineering governance across TTZip to prevent future regressions, memory safety bugs, and concurrency degradation across the Swift-Rust boundary.

```mermaid
graph TD
    subgraph "Swift 6 Concurrency Layer"
        TaskExec[TaskExecutionHandle] -->|Arc retain/release| TokenBridge[Cancellation Bridge]
        BufferAdapter[CUnsafeBufferAdapter] -->|Packed String Buffer| ContigArray[TTZipPackedStringArray]
        Lz4Cache[VFSLz4CachePool] -->|2048-entry LRU Bound| CacheMap[rawSizeCache]
    end

    subgraph "C-ABI Contract Boundary (lint-contracts.sh)"
        TokenBridge --> TokenHandle[ttzip_rust_cancellation_token_*]
        ContigArray --> FfiString[ttzip_rust_free_string]
        ErrorEnvelope[TTZipErrorInfo *out_error] --> FfiError[set_last_error]
    end

    subgraph "Rust Microkernel Layer (ttzip-engine)"
        SafeExtract[validate_no_intermediate_symlinks] --> POSIX[VFS Ancestor Realpath]
        TreeBuilder[VfsTreeBuilder O(N)] --> VfsTree[Hierarchical VfsNode]
        ZeroAlloc[cmp_case_insensitive] --> VfsSearch[fuzzy_search_tree]
        TwoPhase[plan_evictions] --> ShardLock[RwLock<LruShard>]
        ShardLock -.->|Outside Lock| DiskIO[fs::write]
    end
```

---

## 2. Technical Touchpoints & Implementation Scope

### Component 1: Cross-Language FFI & Lifecycle Subsystem (`US1`)
- `core/rust/ttzip-engine/src/types.rs`: Structured `TTZipErrorInfo` C-ABI definition and `resolve_thread_budget`.
- `core/Sources/CTTZipBridge/include/ttzip_rust_glue.h`: Header alignment for `TTZipErrorInfo`, `TTZipPackedStringArray`, `ttzip_rust_cancellation_token_retain`.
- `core/rust/ttzip-engine/src/ffi/runtime_ffi/cancellation_ffi.rs`: Atomic reference counting on `Arc<CancellationToken>`.
- `core/Sources/TTZipCore/Concurrency/TaskExecutionHandle.swift`: Explicit retain on dispatch and release on deinit.
- `core/Sources/TTZipCore/Bridge/CUnsafeBufferAdapter.swift`: Contiguous packed buffer allocation.

### Component 2: Defensive Systems & Path Traversal Barrier (`US2`)
- `core/rust/ttzip-engine/src/fs/safe_extract.rs`: `validate_no_intermediate_symlinks` ancestor recursion check.
- `core/rust/ttzip-engine/src/archive/tar/writer.rs`: `truncate_to_char_boundary` for PAX/ustar header strings.
- `core/rust/ttzip-engine/src/crypto/sha1/winzip.rs`: Constant-time byte comparisons (`subtle::ConstantTimeEq`).
- `core/rust/ttzip-engine/src/crypto/rs_fec/record_format.rs`: Dynamic slice scaling for Reed-Solomon recovery records.

### Component 3: Lock-Free Concurrency & Zero-Allocation Hotpaths (`US3`)
- `core/rust/ttzip-engine/src/vfs/cache_pool.rs`: Two-phase eviction (`plan_evictions`) with lock-free disk writes.
- `core/rust/ttzip-engine/src/fs/vfs/node.rs`: `cmp_case_insensitive` Unicode iterator comparator.
- `core/rust/ttzip-engine/src/fs/vfs/search.rs`: `starts_with_ignore_case` zero-allocation prefix matching.
- `core/rust/ttzip-engine/src/fs/vfs/tree.rs`: `VfsTreeBuilder` hash pre-indexing.
- `core/rust/ttzip-engine/src/archive/unified/create.rs`: Single-pass stream splitting via `archive_write_open2`.

### Component 4: Multi-Matrix CI & Statistical Regression Gate (`US4`)
- `core/scripts/ab_performance_audit.py`: Automated multi-round git worktree A/B benchmark suite.
- `core/rust/Cargo.toml`: Proptest, Criterion, and multi-architecture target definitions.
- `.specify/scripts/bash/lint-contracts.sh`: Schema validator for C-ABI data structures.
- `.specify/scripts/bash/lint-tasks.sh`: Sequential dependency and task validator.
