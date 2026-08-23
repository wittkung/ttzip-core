# Implementation Plan: 185-total-rust-microkernel-migration-and-c-swift-pruning

## Technical Context
- **Objective**: Purge all C source trees from `Sources/CTTZipBridge/`, enhance Rust multi-core password recovery, and prune redundant Swift container parsing logic.

---

## Constitution Check
- [x] **Safe Rust Microkernel**: 100% of codecs, containers, crypto, and recovery backed by Cargo and Rust.
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **SRP & LOC Budget**: 100% of files maintained strictly under $< 350\text{ LOC}$.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《彻底移除 SPM 中 C 语言源码树》: Completed.
- R002 [SUBAGENT:research] 《Safe Rust 多核并行密码恢复与字典攻击引擎》: Completed.

---

## Phase 1: Component Change List

### 1. C Source Tree Purge & Header Unification
- **`Sources/CTTZipBridge/`**: Delete `zopfli/`, `fast-lzma2/`, `lzfse/`, `snappy/`, `CTTZipBridge.c`, `CTTZipBridge_Archive.c`.
- **`Sources/CTTZipBridge/include/CTTZipBridge.h`**: Clean, unified header including `ttzip_rust_glue.h`.
- **`Package.swift`**: Remove any obsolete C compile flags.

### 2. Rust Password Recovery Engine Enhancement
- **`rust/ttzip-glue/src/crypto/password_recovery.rs`**: Rayon-accelerated chunked dictionary and brute-force password recovery for ZipCrypto and WinZip AES.
- **`rust/ttzip-glue/src/ffi/crypto_ffi/password_recovery.rs`**: Export C-ABI functions.

### 3. Swift Redundancy Pruning
- **`Sources/TTZipCore/PasswordRecoveryEngine.swift`**: Direct delegation to Rust multi-core password recovery C-ABI.
- **`Sources/TTZipCore/Split/SplitVolumeEngine.swift`**: Direct delegation to Rust split engine.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` on all Rust crates.
2. `./scripts/build_rust.sh --release && ./scripts/build_tui.sh`.
3. `swift test` ensuring all 897+ tests pass with 0 failures and 0 warnings.
4. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
