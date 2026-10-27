# Tasks: 097-7z-zero-config-optimization-audit

**Input**: Design documents from `specs/097-7z-zero-config-optimization-audit/`  
**Prerequisites**: [plan.md](file:///Users/kevintung/Documents/dev/TTZip/specs/097-7z-zero-config-optimization-audit/plan.md), [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/097-7z-zero-config-optimization-audit/spec.md), [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/097-7z-zero-config-optimization-audit/research.md), [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/097-7z-zero-config-optimization-audit/data-model.md), [contracts/](file:///Users/kevintung/Documents/dev/TTZip/specs/097-7z-zero-config-optimization-audit/contracts/)

---

## Phase 1: Setup & Environment Validation

**Purpose**: 验证构建与 7z 测试基线

- [x] T001 [P] Validate current Swift 6.0 build and C static library linking in `Package.swift`
- [x] T002 [P] Verify baseline unit tests for 7z engine and Fast-LZMA2 in `Tests/TTZipTests/FastLZMA2Tests.swift`

---

## Phase 2: Foundational (7z Engine Invariants & C Bridge Alignment)

**Purpose**: 确保 C 静态库桥接与 Swift 核心调度层契约 100% 对齐

- [x] T003 [P] Verify dynamic entropy sampling threshold ($H > 7.90$) and Store fallback in `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`
- [x] T004 [P] Verify ARM64 NEON SHA-256 KDF and AES-256-CBC 512KB parallel decryption in `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c` and `Sources/CTTZipBridge/ttzip_7z_crypto_neon.c`
- [x] T005 [P] Verify two-level `mkdir_p` stack directory cache in `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c`

---

## Phase 3: User Story 1 - 零配置全自动极速 7z 压缩与自适应分流 (Priority: P1) 🎯 MVP

**Goal**: 确保 7z 压缩在零用户参数干预下自适应达成最佳吞吐（L1 $\ge 3,200\text{ MB/s}$，高熵自动 Store 降级）

- [x] T006 [P] [US1] Verify zero-configuration CPU topology auto-detection in `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`
- [x] T007 [P] [US1] Verify Fast-LZMA2 / SWAR / Radix match finder multi-tier binding in `Sources/CTTZipBridge/ttzip_fl2_bridge.c`
- [x] T008 [US1] Ensure `ArchiveWriter+Dispatch.swift` routes 7z compression directly to `SevenZipParallelWriter` in `Sources/TTZipCore/ArchiveWriter+Dispatch.swift`
- [x] T009 [US1] Run end-to-end 7z compression validation tests in `Tests/TTZipTests/FastLZMA2Tests.swift`

**Checkpoint**: 7z 零配置自适应并行压缩通过完整测试，达到历史最优吞吐。

---

## Phase 4: User Story 2 - 原生硬件加速 7z 并行解压与加密直通 (Priority: P2)

**Goal**: 确保 7z 原生多块并行解压与硬件向量化解密在全部场景下 100% 稳定运行

- [x] T010 [P] [US2] Verify parallel multi-block payload decoder (LZMA2 / Zstd / Deflate / Store) in `Sources/CTTZipBridge/ttzip_7z_block_decoder.c`
- [x] T011 [P] [US2] Verify temporal parallelism for KDF derivation in `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`
- [x] T012 [US2] Ensure `ArchiveExtractor+Dispatch.swift` routes 7z extraction to native in-process decoder with libarchive fallback in `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift`
- [x] T013 [US2] Run 7z encrypted archive and multi-block decompression regression tests in `Tests/TTZipTests/TouchIDAndHeaderEncryptionTests.swift`

**Checkpoint**: 7z 原生并行解压与硬件向量化解密全量测试通过。

---

## Phase 5: User Story 3 - 归档极速检视与多卷穿透零摩擦 (Priority: P3)

**Goal**: 在 `ArchiveReader` 中接入 7z 零拷贝原生 Header 数据库解析快速通道（$< 2\text{ms}$ 响应）

- [x] T014 [P] [US3] Bridge `NativeSevenZipEngine.inspectSevenZip` with `ttzip_native_inspect_archive` in `Sources/TTZipCore/SevenZip/NativeSevenZipEngine.swift`
- [x] T015 [US3] Add 7z zero-copy fast-path inspection with graceful encrypted fallback in `Sources/TTZipCore/ArchiveReader.swift`
- [x] T016 [US3] Add unit tests for 7z zero-copy inspection speed and encryption tier verification in `Tests/TTZipTests/ArchiveEncryptionIntrospectionTests.swift`

**Checkpoint**: 7z 目录检视在 $< 2\text{ms}$ 极速响应且完全兼容加密归档。

---

## Phase 6: Polish & Full Matrix Hardening

**Purpose**: 全格式回归测试与门禁验证

- [x] T017 [P] Execute full format support suite in `Tests/TTZipTests/FormatSupportTests.swift`
- [x] T018 Execute full quickstart verification scenarios in `specs/097-7z-zero-config-optimization-audit/quickstart.md`
