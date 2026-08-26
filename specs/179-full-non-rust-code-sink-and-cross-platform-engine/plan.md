# Implementation Plan: 179-full-non-rust-code-sink-and-cross-platform-engine

## Technical Context
- **Target Architecture**: Safe Rust core engine (`rust/ttzip-glue`) + Thin Swift public facades (`Sources/TTZipCore`) + Standalone TUI/CLI (`rust/ttzip-tui` -> `bin/ttzip`).
- **Core Domains Sinking**:
  1. **Security & Path Sanitizer**: `rust/ttzip-glue/src/security/path_sanitizer.rs` (ZipSlip, Win32 devices, ADS, Unicode NFC).
  2. **CJK Charset & Transcoder**: `rust/ttzip-glue/src/charset/` (Mozilla bigram detector, `encoding_rs` zero-allocation transcoder).
  3. **Streaming RS-FEC & Recovery**: `rust/ttzip-glue/src/crypto/rs_fec/` (Streaming Cauchy RS, 32B raw binary SHA-256 fix).
  4. **Parallel FS Scanner & Preallocator**: `rust/ttzip-glue/src/fs/scanner.rs` (Rayon directory traversal, Inode cycle protection).
  5. **SIMD HexDiff & Fuzzing Harness**: `rust/ttzip-glue/src/testing/` (16B NEON/SSE2 vectorized difference and mutation fuzzer).
  6. **Platform Zeroize & Dynamic CPUID**: `rust/ttzip-glue/src/platform/` (Dead-Store safe key zeroization, P/E core and cache line sniffing).
  7. **Swift Facades & Pattern Thinning**: Thinning `PlatformPathSanitizer.swift`, `CharsetDetectionStrategyProtocol.swift`, `ReedSolomonFEC.swift`, `ArchiveRecoveryRecordEngine.swift`, `ZipDirectoryScanner.swift`, `FastHexDiffEngine.swift`, and `PlatformMemory.swift`.

---

## Constitution Check
- [x] **Principle 1: Safe Rust Core**: All algorithms, path safety, and codepages implemented in Safe Rust without Darwin/CoreFoundation lock-in.
- [x] **Principle 2: Zero Cloud Actions Quota**: 100% of compilation, verification, and regression tests run locally.
- [x] **Principle 3: Zero Memory Hazards**: Eliminates Swift pointer escape UAF, eliminates Dead-Store elimination on key memory, and fixes SHA-256 binary digest truncation.
- [x] **Principle 4: SRP LOC Budget**: All new files strictly kept under $< 350\sim 500\text{ LOC}$.

---

## Phase 0: Research Items Index
- R001 [SUBAGENT:research] 《路径安全防御、ZipSlip 字节级判定与 Windows 设备名/ADS 过滤方案》: Completed.
- R002 [SUBAGENT:research] 《双字节 Bigram 频度统计 CJK 字符集嗅探与 `encoding_rs` 跨平台转码方案》: Completed.
- R003 [SUBAGENT:research] 《流式 Cauchy RS-FEC、32B 二进制 SHA-256 校验与 Swift UAF 隐患消除方案》: Completed.
- R004 [SUBAGENT:research] 《多核并行目录遍历 (`fs::scanner`)、Inode 环路防护与盘块预分配方案》: Completed.

---

## Phase 1: Architecture Artifacts & Component Change List

### 1. `rust/ttzip-glue/` Modules
- **`src/security/path_sanitizer.rs`**: Single-pass traversal check, Win32 device filtering, ADS stripping, and NFC normalization.
- **`src/charset/`**: Mozilla bigram statistical tables, CSM state machines, and `encoding_rs` transcoder.
- **`src/crypto/rs_fec/recovery_record.rs`**: Streaming Cauchy accumulator, raw 32-byte binary SHA-256 root digest.
- **`src/fs/scanner.rs`**: Rayon parallel directory walker with 64-way sharded Inode tracker.
- **`src/testing/`**: Vectorized 16B hex diff formatter and SplitMix64 mutation fuzzer.
- **`src/platform/`**: `SecureBuffer` with compiler barrier `zeroize` and dynamic CPUID capability detector.
- **`src/ffi/`**: Export unified C-ABIs for path sanitizing, charset detection, directory scanning, and zeroizing.

### 2. `Sources/TTZipCore/` & `Sources/CTTZipBridge/`
- **`Sources/CTTZipBridge/include/ttzip_rust_glue.h`**: Add new C-ABI function prototypes.
- **`Sources/TTZipCore/Platform/PlatformPathSanitizer.swift`** & **`SecurityScanner.swift`**: Thin to invoke `ttzip_rust_sanitize_path`.
- **`Sources/TTZipCore/Strategies/CharsetDetectionStrategyProtocol.swift`** & **`CharsetDetector.swift`**: Thin to invoke `ttzip_rust_sanitize_filename`.
- **`Sources/TTZipCore/Security/ReedSolomonFEC.swift`** & **`ArchiveRecoveryRecordEngine.swift`**: Delegate to Rust streaming Cauchy engine, eliminating pointer escape.
- **`Sources/TTZipCore/Zip/ZipDirectoryScanner.swift`** & **`FolderStatsCalculator.swift`**: Delegate to Rust parallel scanner.
- **`Sources/TTZipCore/Testing/FastHexDiffEngine.swift`** & **`MalformedStreamFuzzEngine.swift`**: Delegate to Rust SIMD diff/fuzz.
- **`Sources/TTZipCore/Platform/PlatformMemory.swift`** & **`PlatformHardware.swift`**: Delegate to Rust zeroize and CPUID.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` on all Rust crates (`ttzip-glue`, `ttzip-tui`).
2. Build static library and standalone `bin/ttzip`.
3. `swift test` across all 872+ tests.
4. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
