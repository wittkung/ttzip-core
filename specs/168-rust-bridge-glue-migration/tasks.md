# Tasks: TTZip 核心胶水层全面迁移 Rust 架构方案 (Feature 168)

**Feature ID**: `168-rust-bridge-glue-migration`  
**Created**: 2026-08-21  
**Status**: Ready for Implementation  
**Specification**: [`specs/168-rust-bridge-glue-migration/spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/168-rust-bridge-glue-migration/spec.md)  
**Plan**: [`specs/168-rust-bridge-glue-migration/plan.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/168-rust-bridge-glue-migration/plan.md)

---

## Phase 1: 基础设施与构建流水线 (Foundational Setup & Build Pipeline)

- [x] T001 [P] 初始化 Rust Workspace 与 `ttzip-glue` crate 骨架 in `rust/Cargo.toml`
- [x] T002 [P] 编写 `rust/ttzip-glue/build.rs` 静态链接 `Vendor/libTTZipVendor.a` 及系统框架 in `rust/ttzip-glue/build.rs`
- [x] T003 [P] 编写 Universal 静态库交叉编译与 `cbindgen` 头文件生成脚本 in `scripts/build_rust.sh`
- [x] T004 更新 `Package.swift` 与 `module.modulemap` 引入 `ttzip_rust_glue.h` in `Package.swift`

---

## Phase 2: 硬件加速与密码算子迁移 (US2 - Hardware Acceleration & Crypto Parity)

- [x] T005 [P] [US2] 实现 Apple Silicon ARM64 12 路 PMULL 向量多项式折叠 CRC32 算子 in `rust/ttzip-glue/src/crypto/crc32.rs`
- [x] T006 [P] [US2] 实现 Apple Silicon ARM64 UDOT 点积与 5552 延迟求模 Adler32 算子 in `rust/ttzip-glue/src/crypto/adler32.rs`
- [x] T007 [P] [US2] 实现 Apple Silicon 8 路交织流水线 AES-256-CBC / CTR 硬件解密核 in `rust/ttzip-glue/src/crypto/aes256.rs`
- [x] T008 [P] [US2] 实现 Apple Silicon 硬件 SHA-256 KDF 密钥派生算子 in `rust/ttzip-glue/src/crypto/sha256.rs`
- [x] T009 [US2] 导出 C-ABI 密码与校验接口并在 `tests/` 中编写向量基准单测 in `rust/ttzip-glue/src/ffi/crypto_ffi.rs`

---

## Phase 3: 单格式编解码器安全封装 (US1 & US2 - Safe Codec Wrappers)

- [x] T010 [P] [US1] 实现 `libdeflate` 压缩与解压器的 Safe RAII 封装 in `rust/ttzip-glue/src/codecs/deflate.rs`
- [x] T011 [P] [US1] 实现 `zstd` 多线程上下文句柄与流式 API 的 Safe RAII 封装 in `rust/ttzip-glue/src/codecs/zstd.rs`
- [x] T012 [P] [US1] 实现 `fast-lzma2` 多线程流式上下文的 Safe RAII 封装 in `rust/ttzip-glue/src/codecs/lzma2.rs`
- [x] T013 [P] [US1] 实现 `snappy`, `lz4`, `lzfse` 极速块压缩的安全封装 in `rust/ttzip-glue/src/codecs/fast_blocks.rs`
- [x] T014 [P] [US1] 实现 Mozilla `uchardet` 字符编码探测安全封装 in `rust/ttzip-glue/src/codecs/chardet.rs`

---

## Phase 4: 流式管道、文件系统与安全防御 (US1, US3 - Safe FS & Streaming)

- [x] T015 [P] [US1] 实现 `libarchive` 自定义流读取/写入回调与 `std::io` 转换器 in `rust/ttzip-glue/src/archive/stream_adapter.rs`
- [x] T016 [P] [US1] 实现 POSIX `O_NOFOLLOW`、两阶段目录权限与 mtime 倒序回写引擎 in `rust/ttzip-glue/src/fs/safe_extract.rs`
- [x] T017 [P] [US1] 实现 APFS 16KB 页对齐分配与 Extent 连续物理空间预分配 in `rust/ttzip-glue/src/fs/apfs.rs`
- [x] T018 [P] [US3] 实现原子取消令牌（`AtomicBool`）与跨线程优雅释放通道 in `rust/ttzip-glue/src/runtime/cancellation.rs`
- [x] T019 [P] [US4] 实现统一日志路由器将 `tracing` / `log` 转发至 Swift `TTLogger` in `rust/ttzip-glue/src/runtime/logging.rs`

---

## Phase 5: 容器格式解析与并行执行引擎 (US1, US2, US4 - Archive Engines & FFI)

- [x] T020 [P] [US2] 实现多核并行 ZIP 压缩与 Central Directory 零拷贝解析引擎 in `rust/ttzip-glue/src/zip/mod.rs`
- [x] T021 [P] [US2] 实现 7z Header 零拷贝解析器与 Solid 固实流式解码引擎 in `rust/ttzip-glue/src/sevenz/mod.rs`
- [x] T022 [US4] 封装 FFI 统一入口并注入 `catch_unwind` 异常屏障 in `rust/ttzip-glue/src/ffi/mod.rs`
- [x] T023 [US4] 重构 Swift `TTZipCore` 桥接层以调用新 C-ABI 接口 in `Sources/TTZipCore/Bridge/ArchiveEngineBridge.swift`

---

## Phase 6: 质量审查、基准测试与收敛验证 (Converge & Verification)

- [x] T024 运行 Rust 单元测试与 ASan / Miri 内存安全验证 in `rust/ttzip-glue/`
- [x] T025 运行 Swift 全量 525+ 单元测试与差分预言机校验 (`swift test`)
- [x] T026 执行 `./scripts/benchmark_ab.sh` 5 轮采样评测，确保 ZIP 压缩/解压与 AES 吞吐无倒退
- [x] T027 清理与下线 `Sources/CTTZipBridge/` 中已完全迁移的冗余 C 文件
