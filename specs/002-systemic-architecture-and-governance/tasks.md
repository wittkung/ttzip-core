# Tasks: TTZip Systemic Architecture & Engineering Governance

- **Feature ID**: `002-systemic-architecture-and-governance`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `COMPLETED`

---

## Phase 1: Setup

- [x] T001 Verify project build environment and Rust toolchain targets in `core/rust/Cargo.toml`
- [x] T002 [P] Verify Swift bridge dependency configurations in `core/Package.swift`
- [x] T003 [P] Verify C-ABI header module map in `core/Sources/CTTZipBridge/include/module.modulemap`

---

## Phase 2: Foundational

- [x] T004 Implement `resolve_thread_budget` auto-hardware parallelism helper in `core/rust/ttzip-engine/src/types.rs`
- [x] T005 [P] Implement `TTZipErrorInfo` C-ABI struct definition and error helper in `core/rust/ttzip-engine/src/types.rs`
- [x] T006 [P] Declare `TTZipErrorInfo` and `TTZipPackedStringArray` in `core/Sources/CTTZipBridge/include/ttzip_rust_glue.h`

---

## Phase 3: User Story 1 - Cross-Language FFI Contract & Lifecycle Governance

- [x] T007 [P] [US1] Wrap Filter AST in `ManuallyDrop` to prevent drop-order UAF in `core/rust/ttzip-engine/src/ffi/filter_ffi.rs`
- [x] T008 [P] [US1] Implement `Arc<CancellationToken>` atomic retain/release lifecycle in `core/rust/ttzip-engine/src/ffi/runtime_ffi/cancellation_ffi.rs`
- [x] T009 [P] [US1] Implement retain/release in `core/Sources/TTZipCore/Concurrency/TaskExecutionHandle.swift`
- [x] T010 [P] [US1] Pass active processor count as thread budget in `core/Sources/TTZipCore/ArchiveExtractor.swift`
- [x] T011 [P] [US1] Pass active processor count as thread budget in `core/Sources/TTZipCore/ArchiveWriter.swift`
- [x] T012 [P] [US1] Pass `format = 0` (Auto) for format detection in `core/Sources/TTZipCore/Services/InPlaceArchiveMutationEngine.swift`
- [x] T013 [P] [US1] Implement zero-fragment contiguous string array bridge in `core/Sources/TTZipCore/Bridge/CUnsafeBufferAdapter.swift`
- [x] T014 [P] [US1] Instrument `set_last_error` on core extraction and compression failure paths in `core/rust/ttzip-engine/src/ffi/archive_ffi/unified.rs`

---

## Phase 4: User Story 2 - Defensive Systems Architecture & Anti-Traversal Barrier

- [x] T015 [P] [US2] Implement UTF-8 character boundary truncation in `core/rust/ttzip-engine/src/archive/tar/writer.rs`
- [x] T016 [P] [US2] Implement `K_ENCODED_HEADER` decompression and recursive metadata parsing in `core/rust/ttzip-engine/src/sevenz/header/metadata.rs`
- [x] T017 [P] [US2] Store pack info total sizes in `core/rust/ttzip-engine/src/sevenz/header/stream.rs`
- [x] T018 [P] [US2] Implement `validate_no_intermediate_symlinks` directory traversal validator in `core/rust/ttzip-engine/src/fs/safe_extract.rs`
- [x] T019 [US2] Enforce intermediate symlink checks and safe relative targets in `core/rust/ttzip-engine/src/archive/unified/extract.rs`
- [x] T020 [P] [US2] Enforce intermediate symlink checks in `core/rust/ttzip-engine/src/ffi/archive_ffi/extract.rs`
- [x] T021 [P] [US2] Implement constant-time MAC and PVV verification in `core/rust/ttzip-engine/src/crypto/sha1/winzip.rs`

---

## Phase 5: User Story 3 - Lock-Free Concurrency & Zero-Allocation Hotpaths

- [x] T022 [US3] Implement direct streamed split archive writing via `archive_write_open2` in `core/rust/ttzip-engine/src/archive/unified/create.rs`
- [x] T023 [P] [US3] Implement $O(N)$ hash pre-indexed `VfsTreeBuilder` in `core/rust/ttzip-engine/src/fs/vfs/tree.rs`
- [x] T024 [P] [US3] Implement zero-allocation case-insensitive string comparison in `core/rust/ttzip-engine/src/fs/vfs/node.rs`
- [x] T025 [P] [US3] Implement zero-allocation case-insensitive prefix matching in `core/rust/ttzip-engine/src/fs/vfs/search.rs`
- [x] T026 [P] [US3] Implement two-phase lock splitting for disk spill and decompression in `core/rust/ttzip-engine/src/vfs/cache_pool.rs`
- [x] T027 [P] [US3] Implement bounded LRU eviction (2048 entries) for `rawSizeCache` in `core/Sources/TTZipCore/VFS/VFSLz4CachePool.swift`
- [x] T028 [US3] Replace `fs::read` fallback with memory-mapped `open_archive_source` in `core/rust/ttzip-engine/src/archive/unified/extract.rs`

---

## Phase 6: User Story 4 - Multi-Matrix Testing, Property Fuzzing & Performance Gate

- [x] T029 [P] [US4] Implement Dynamic Slice Scaling in `core/rust/ttzip-engine/src/crypto/rs_fec/record_format.rs`
- [x] T030 [P] [US4] Implement 2-level prefix keyspace partitioning and atomic attempt batching in `core/rust/ttzip-engine/src/crypto/password_recovery.rs`
- [x] T031 [P] [US4] Implement strict output buffer length guard in `core/rust/ttzip-engine/src/ffi/crypto_ffi/password_recovery.rs`
- [x] T032 [P] [US4] Implement `inv_mix_columns_block` and non-AArch64 inverse round keys in `core/rust/ttzip-engine/src/crypto/aes256/mod.rs`
- [x] T033 [P] [US4] Fix continuous CBC state chaining by removing `decryptor.clone()` in `core/rust/ttzip-engine/src/crypto/aes256/cbc.rs`
- [x] T034 [P] [US4] Pre-decode solid folders once for in-place entry slicing in `core/rust/ttzip-engine/src/archive/in_place_edit.rs`

---

## Phase 7: Polish & Cross-Cutting Integration Verification

- [x] T035 Execute full Rust engine regression and integration test suite via `cargo test`
- [x] T036 Execute Swift core framework test suite via `swift test`
- [x] T037 Validate JSON Schema contracts using `.specify/scripts/bash/lint-contracts.sh`
