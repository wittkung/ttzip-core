# Tasks: Rust Code Quality & Modernization Audit

**Feature**: `213-rust-quality-clippy-modernization`  
**Status**: `COMPLETED`  

---

## Tasks

- [x] **Task 1: C-ABI Exports & C-String Modernization (`ttzip-glue/src/lib.rs`)**
  - [x] Replace `b"...\0".as_ptr() as *const c_char` with `c"...".as_ptr()`.
  - [x] Audit all C-ABI status string returns.

- [x] **Task 2: Cryptography & SIMD Modernization (`ttzip-glue/src/crypto/`)**
  - [x] Fix `crypto/rs_fec/tests.rs` module inception and identity ops.
  - [x] Fix `crypto/zipcrypto/tests.rs` module inception and needless range loop.
  - [x] Implement `Default` for `ZipCryptoBatch4`.
  - [x] Refactor `repair_archive_data` to accept slice `&mut [u8]`.

- [x] **Task 3: File System, VFS & Standards Modernization (`ttzip-glue/src/fs/`, `src/standards/`, `src/zip/`)**
  - [x] Fix `fs/scanner.rs` struct default initialization.
  - [x] Replace `vec![0u8; N]` in `standards/ffi.rs` and `standards/sniffer.rs` with stack arrays `[0u8; N]`.
  - [x] Fix `bool_assert_comparison` in sniffer tests.
  - [x] Fix `cloned_ref_to_slice_refs` in `zip/mod.rs` and `archive/unified/tests_lifecycle.rs`.
  - [x] Simplify `normalize_to_nfc` in `security/path_sanitizer.rs`.
  - [x] Implement `std::str::FromStr` for `EntryType` in `testing/differential.rs`.
  - [x] Fix byte char slices in `archive/tar/tests.rs`.

- [x] **Task 4: Runtime, Worker Pool & TUI Modernization (`ttzip-glue/src/runtime/`, `ttzip-tui/`)**
  - [x] Fix `ring_buffer/tests.rs` and `worker_pool/tests.rs` module inception.
  - [x] Use `sort_by_key` with `Reverse` in `ttzip-tui/src/vfs/search.rs` and `ttzip-glue/src/fs/vfs/search.rs`.
  - [x] Modernize braille plotter, modals, and repair runner in `ttzip-tui`.

- [x] **Task 5: Zero-Warning Clippy & Test Verification**
  - [x] Run `cargo clippy --all-targets --all-features -- -D warnings` (100% clean, 0 warnings).
  - [x] Run `./scripts/run_rust_tests.sh --unit --props --fuzz` (all tests pass).
  - [x] Run full 4-stage local CI gate (`./scripts/run_local_ci_gate.sh`).
  - [ ] Commit and push to `origin main`.
