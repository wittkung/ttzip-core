# Implementation Plan: 176-sink-snappy-brotli-split-entropy-recovery-to-rust

## Technical Context
- **Target Architecture**: Self-sufficient Safe Rust core (`rust/ttzip-glue` and `rust/ttzip-tui`) + Ultra-thin Swift layer (`Sources/TTZipApp`, minimal C-ABI bridge).
- **Core Components Sinking**:
  1. **Snappy Framing & Brotli Codecs**: Pure Rust `snap = "1.1"` (correct Castagnoli CRC-32C) and `brotli = "7.0"` (0 Apple framework dependency) streaming engines.
  2. **Multi-Volume Split & Virtual Reader**: Streaming `SplitVolumeWriter` (`std::io::Write`) and continuous `VirtualMultiVolumeReader` (`std::io::Read + Seek`).
  3. **SIMD Shannon Entropy & Codec Selector**: 4-way unrolled 256-bucket histogram with NEON/AVX2 reduction (<70µs probe) and 3-stage cascaded recommendation.
  4. **VFS O(1) Lock-Free LZ4 LRU Cache Pool**: Index-based Arena doubly-linked list with `hashbrown::HashMap` and 16-way sharded locks.
  5. **In-Memory Multi-Core Password Recovery & SIMD Salvage**: Rayon parallel in-memory verification (>150,000 keys/sec) and SIMD corrupted TOC reconstruction.

---

## Constitution Check
- [x] **Principle 1: Safe Rust First**: C++ snappy and Apple `Compression.framework` are replaced with pure Safe Rust implementations.
- [x] **Principle 2: Zero Polling & Zero Disk Leakage**: In-memory password recovery eliminates all temporary disk file creation.
- [x] **Principle 3: Zero OOM & Bounded Buffering**: Streaming codecs enforce bounded 4MB double-buffering.
- [x] **Principle 4: Zero Breaking Changes**: All existing Swift public APIs retain backward compatibility through high-level C-ABI glue, ensuring 100% test pass rate across 863+ tests and 7/7 local CI stages.

---

## Phase 0: Research Items Index
- R001 [SUBAGENT:research] 《纯 Rust Snappy Framing 与 Brotli 跨平台流式实现方案》: Completed.
- R002 [SUBAGENT:research] 《多分卷切分与连续虚拟重组流设计方案》: Completed.
- R003 [SUBAGENT:research] 《SIMD 向量化香农熵与智能 Codec 决策方案》: Completed.
- R004 [SUBAGENT:research] 《VFS O(1) 无锁双向链表 LZ4 缓存池设计方案》: Completed.
- R005 [SUBAGENT:research] 《纯内存 Rayon 密码恢复与归档结构自愈方案》: Completed.

---

## Phase 1: Architecture Artifacts & Component Change List

### 1. `rust/ttzip-glue/src/` Modules
- **`src/codecs/snappy/`**: `block.rs`, `frame.rs`, `pipe.rs`, `mod.rs`.
- **`src/codecs/brotli/`**: `block.rs`, `stream.rs`, `pipe.rs`, `mod.rs`.
- **`src/archive/split/`**: `writer.rs`, `reader.rs`, `mod.rs`.
- **`src/analytics/`**: `entropy.rs`, `codec_selector.rs`, `mod.rs`.
- **`src/vfs/`**: `cache_pool.rs`, `mod.rs`.
- **`src/crypto/recovery/`**: `probe.rs`, `worker.rs`, `mod.rs`.
- **`src/archive/repair/`**: `zip.rs`, `tar.rs`, `mod.rs`.

### 2. C-ABI FFI Updates
- `src/ffi/codecs_ffi/snappy.rs` & `brotli.rs`: Export Snappy Frame & Brotli stream C-ABIs.
- `src/ffi/archive_ffi/split.rs` & `repair.rs`: Export Split & Repair C-ABIs.
- `src/ffi/analytics_ffi.rs`: Export SIMD Entropy & Codec selector C-ABIs.
- `src/ffi/crypto_ffi/recovery.rs`: Export in-memory password recovery C-ABIs.
- `src/ffi/vfs_ffi.rs`: Export VFS Cache Pool C-ABIs.
- Update `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.

### 3. Swift Thinning
- `Sources/TTZipCore/Snappy/SnappyFramingStream.swift` & `SnappyBlockEngine.swift`: Forward to Rust.
- `Sources/TTZipCore/Brotli/NativeBrotliEngine.swift`: Delete `import Compression`, forward to Rust.
- `Sources/TTZipCore/Split/MultiVolumeStreamSink.swift`: Forward to Rust split stream.
- `Sources/TTZipCore/Services/ArchiveEntropyEvaluator.swift` & `SmartCodecSelector.swift`: Forward to Rust.
- `Sources/TTZipCore/VFS/VFSLz4CachePool.swift`: Forward to Rust cache pool.
- `Sources/TTZipCore/PasswordRecoveryEngine.swift` & `Strategies/PasswordRecoveryStrategyProtocol.swift`: Forward to Rust in-memory recovery.
- `Sources/TTZipCore/ArchiveRepairEngine.swift` & `Strategies/ArchiveRepairStrategyProtocol.swift`: Forward to Rust repair.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` across all unit, property, and integration tests.
2. `./scripts/build_rust.sh --release` and `./scripts/build_tui.sh`.
3. `swift test` across all 863+ tests ensuring 100% green.
4. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
