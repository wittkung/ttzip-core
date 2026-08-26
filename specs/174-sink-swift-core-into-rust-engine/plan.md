# Implementation Plan: 174-sink-swift-core-into-rust-engine

## Technical Context
- **Target Architecture**: Self-sufficient Safe Rust core (`rust/ttzip-glue` and `rust/ttzip-tui`) + Ultra-thin Swift layer (`Sources/TTZipApp`, minimal C-ABI bridge).
- **Core Components Sinking**:
  1. **Archive Containers**: ZIP (Zero-copy Central Directory, Zip64, LFH, Rayon parallel Deflate/Store), TAR (ustar, GNU LongName/LongLink, PAX Extended Headers, octal checksums), 7z seek tables.
  2. **Standards & Magic**: 16-format magic sniffer (Head, Tail, Sector 16, TarOffset), strict standards compliance checkers for PKWARE, POSIX, RFC 1952, RFC 8878, 7-Zip, ISO 9660, MS-WIM.
  3. **Cryptography & Self-Healing**: PKZIP 3-Key stream cipher, WinZip AES-CTR/CBC, 7z SHA-256 hardware KDF with `zeroize::ZeroizeOnDrop`, Cauchy GF(2^8) Reed-Solomon FEC (25+ GB/s NEON SIMD) and TTZR/TTRC self-healing recovery records.
  4. **Benchmarking & Differential Testing**: In-memory zero-I/O benchmarks, 7-Zip MIPS hardware scoring, Pareto frontier calculator, SplitMix64 mutation fuzzing, and system oracle differential testing.
  5. **Cross-Platform CLI**: Standalone `ttzip` binary in `rust/ttzip-tui` supporting all commands without Swift runtime dependencies.

---

## Constitution Check
- [x] **Principle 1: Safe Rust First**: All unsafe pointer operations and manual memory management in Swift are replaced by safe Rust with compiler-enforced borrowing.
- [x] **Principle 2: Zero Leak & Zeroize**: Cryptographic secrets use `zeroize::ZeroizeOnDrop` for deterministic memory erasure.
- [x] **Principle 3: Swift Thin UI Layer**: Swift code is strictly confined to SwiftUI views, ViewModels, and macOS native system integration (FinderSync, Keychain, QuickLook).
- [x] **Principle 4: Zero Breaking Changes & Zero Regression Gate**: All existing public Swift APIs retain full backward compatibility via high-level C-ABI glue, and 100% of 860+ tests and 7/7 local CI stages pass.

---

## Phase 0: Research Items Index
- R001 [SUBAGENT:research] 《ZIP 与 TAR 容器解析/打包纯 Rust 实现与零拷贝切片方案》: Completed.
- R002 [SUBAGENT:research] 《Standards Compliance 16 种格式嗅探与断言纯 Rust 下沉方案》: Completed.
- R003 [SUBAGENT:research] 《PKZIP、WinZip AES、7z KDF 与 Reed-Solomon FEC 纠错算子 Rust 迁移与 Zeroize 方案》: Completed.

---

## Phase 1: Architecture Artifacts & Component Change List

### 1. `rust/ttzip-glue/src/` Modules
- **`src/standards/`**: `mod.rs`, `anchors.rs`, `signatures.rs`, `registry.rs`, `sniffer.rs`, `extra_fields.rs`, `report.rs`, `ffi.rs`, `checkers/` (ZIP, TAR, 7z, GZ, ZSTD, BZ2, XZ, DiskImages, Modern).
- **`src/crypto/`**:
  - `zipcrypto.rs`: PKZIP 3-Key cipher (scalar ARM64 `__crc32b` + multi-stream SIMD + `ZeroizeOnDrop`).
  - `rs_fec/`: `mod.rs`, `gf8.rs` (Nibble SIMD `vqtbl1q_u8`), `cauchy.rs` (Gaussian elimination inversion), `recovery_record.rs` (TTZR/TTRC layout, corruption scan & repair).
- **`src/archive/tar/`**: `mod.rs`, `header.rs`, `scanner.rs`, `reader.rs`, `writer.rs` (full GNU LongName & PAX Extended Headers).
- **`src/zip/`**: `parser.rs` (Zero-copy CD & Zip64), `reader.rs` (Rayon parallel extraction + APFS preallocation), `writer/` (Store stream + parallel Deflate).
- **`src/bench/`**: `in_memory.rs`, `mips.rs`, `pareto.rs`.

### 2. `rust/ttzip-tui/` Modules
- `cli/handlers.rs`: Add native implementations for `verify`, `bench`, `test`, `fuzz`.

### 3. `Sources/CTTZipBridge/` & `Sources/TTZipCore/`
- Update `ttzip_rust_glue.h` with new standards, crypto, and container C-ABI symbols.
- Thin out Swift adapters to directly forward calls to `ttzip_rust_*`.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` on all Rust modules (unit tests, property tests, fuzz harnesses, differential oracles).
2. `./scripts/build_rust.sh --release` and `./scripts/build_tui.sh`.
3. `swift test` across all 860+ tests ensuring 100% green.
4. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
