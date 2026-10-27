# Implementation Plan: TTZip 核心胶水层全面迁移 Rust 架构方案 (Feature 168)

**Feature ID**: `168-rust-bridge-glue-migration`  
**Created**: 2026-08-21  
**Status**: Planning Phase  
**Artifact**: Architecture & Implementation Plan

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **现有语言与运行时**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 (`Sources/CTTZipBridge`) + Vendor 静态库（`libarchive`, `libdeflate`, `zstd`, `fast-lzma2`, `snappy`, `lzfse` 等）。
- **目标架构**:
  - 建立 Rust Workspace `rust/`，包含核心静态库 crate `ttzip-glue`（或 `ttzip-rs`）；
  - 输出 C-ABI 静态库 `libttzip_glue.a`，通过 `scripts/build_rust.sh` 打包为 Apple Silicon (`aarch64-apple-darwin`) 与 Intel (`x86_64-apple-darwin`) Universal 二进制；
  - 自动生成 `Sources/CTTZipBridge/include/ttzip_rust_glue.h` 与 `module.modulemap`，由 `TTZipCore` 直接导入消费；
  - 渐进式废弃与移除 `Sources/CTTZipBridge/` 下的手写 C 胶水文件。
- **构建环境约束**: macOS 14.0+, Xcode 16.0+, Rust 1.80+, Cargo, `cbindgen`。

### 1.2 Constitution Check (四大系统工程铁律合规性检查)
- [x] **I. 流式第一性 (Stream-First)**:
  - 零内存假设：禁止全量缓冲大文件，采用 `Pin<Box<StreamState>>` + 64KB/128KB 微缓冲分块拉取；
  - 单任务常驻内存水位严控在 $\le 64\text{MB}$；
  - 零内核清零：热路径使用未初始化页缓冲与切片，消除 `Data(count:)` 导致的零页缺页中断。
- [x] **II. 纵深防御 (Invariant-First)**:
  - 路径防御：Rust 层执行严苛的规范化检查，免疫 ZipSlip、绝对路径与 `..` 穿越；
  - 权限与时间修复：两阶段安全写入，目录初建为 `0700`，完成后自底向上倒序回写权限与 mtime，采用 `O_NOFOLLOW` 验证防 TOCTOU 劫持；
  - 防溢出算术：所有缓冲与条目偏移使用 `checked_add` / `checked_mul`。
- [x] **III. 确定性确界 (Bounds-First)**:
  - 句柄生命周期：全部 C 句柄封装于实现了 `Drop` 的 Safe Rust 结构体，消除 UAF 与 Double-Free；
  - 敏感凭据物理擦除：密码与解密上下文在 `Drop` 时调用 `zeroize` / `memset_s`；
  - FFI 异常屏障：所有导出的 `extern "C"` 入口包裹 `std::panic::catch_unwind`，严禁 Panic 跨越 FFI 边界。
- [x] **IV. 真实预言机 (Oracle-First)**:
  - 差分测试（Differential Testing）：Rust 引擎生成的归档与系统原生 `/usr/bin/unzip`、`/usr/bin/tar` 双向交叉比对；
  - 性能门禁：`./scripts/benchmark_ab.sh` 5 轮交替采样验证，ZIP 压缩 $\ge 1500\text{ MB/s}$、解压 $\ge 4500\text{ MB/s}$、AES $\ge 1800\text{ MB/s}$。

---

## 2. Phase 0: Research Items Index (前置调研项)

- - R001 [SUBAGENT:research] 《现有 C 胶水层资产盘点与高危热路径审计》：全面扫描 `Sources/CTTZipBridge` 93 个 C 文件与 112 个头文件，识别 7 大功能模块、高危裸指针操作与宪法热路径。
- - R002 [SUBAGENT:research] 《macOS / Apple Silicon 与 Swift 6.0 下 Rust 静态库集成选型》：对比 `extern "C"` + `cbindgen`、UniFFI、`cxx` 与 Makefile 外部编排方案，确定 C-ABI + Universal 静态库 + SPM 消费的最佳路径。
- - R003 [SUBAGENT:research] 《Vendor C 静态库在 Rust 中的安全生命周期封装与流式适配》：确定 `sys` 低级绑定 + Safe RAII `Drop` 包装 + `Pin<Box<StreamState>>` 蹦床回调 + 异常屏障机制。
- - R004 [SUBAGENT:research] 《Apple Silicon ARM64 NEON 与 Crypto 扩展在 Rust 中的指令集映射与性能对齐》：调研 `core::arch::aarch64` 稳定版 Intrinsics，确定 PMULL 12 路 CRC32、UDOT Adler32、8 路交织 AES-256 与硬件 SHA-256 的极致实现。

---

## 3. Phase 1: Design Artifacts Index (设计工件索引)

- **数据模型**: [`specs/168-rust-bridge-glue-migration/data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/168-rust-bridge-glue-migration/data-model.md)
- **强类型契约 (Contracts)**:
  - [SUBAGENT:research] `contracts/ttzip_glue_ffi_contract.json` (核心 FFI 交互契约)
  - [SUBAGENT:research] `contracts/ttzip_stream_contract.json` (流式微缓冲与分块契约)
  - [SUBAGENT:research] `contracts/ttzip_crypto_contract.json` (硬件加速密码与校验契约)
  - [SUBAGENT:research] `contracts/ttzip_progress_log_event.json` (进度、取消与日志事件契约)
- **快速验证指南**: [`specs/168-rust-bridge-glue-migration/quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/168-rust-bridge-glue-migration/quickstart.md)

---

## 4. Component Changes & Architecture Mapping (组件改动清单)

### 4.1 新建组件 (New Components)
- `rust/Cargo.toml` / `rust/ttzip-glue/Cargo.toml`: Rust 工作区与核心胶水 crate 配置。
- `rust/ttzip-glue/build.rs`: 链接 `Vendor/` 静态库与 Apple 框架。
- `rust/ttzip-glue/src/lib.rs`: 导出 C-ABI 符号与模块声明。
- `rust/ttzip-glue/src/archive/`: Safe libarchive 封装与流式解包。
- `rust/ttzip-glue/src/zip/`: 并行 ZIP 压缩、解压与 Central Directory 解析。
- `rust/ttzip-glue/src/sevenz/`: 7z Header 解析、Solid 固实流式解码与 LZMA2。
- `rust/ttzip-glue/src/crypto/`: Apple Silicon ARM64 NEON CRC32, Adler32, AES-256, SHA-256。
- `rust/ttzip-glue/src/codecs/`: `libdeflate`, `zstd`, `lz4`, `fast-lzma2`, `snappy`, `lzfse` 安全包装。
- `rust/ttzip-glue/src/fs/`: 安全两阶段权限倒序回写与 APFS Extent 预分配。
- `rust/ttzip-glue/src/ffi/`: 异常屏障、C 结构体对齐与裸指针切片适配。
- `scripts/build_rust.sh`: Universal 静态库编译、`cbindgen` 头文件生成与打包脚本。

### 4.2 修改组件 (Modified Components)
- `Package.swift`: 配置 `CTTZipBridge` 链接生成的 `libttzip_glue.a` 并暴露 `ttzip_rust_glue.h`。
- `Sources/CTTZipBridge/include/module.modulemap`: 引入 `ttzip_rust_glue.h`。
- `Sources/TTZipCore/Bridge/`: 重构 Swift 桥接层，用纯净的强类型接口替代 `CUnsafeBufferAdapter`。
- `Makefile`: 接入 `build_rust` 目标。

### 4.3 渐进式废弃与清理组件 (Deprecated & Deleted Components)
- `Sources/CTTZipBridge/*.c`: 随着各功能模块在 Rust 中迁移并通过验证，分阶段下线并移除对应的 C11 源码文件。

---

## 5. Phased Rollout Plan (分期推进计划)

```
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 1: 基础工程构建与核心工具库迁移                                    │
│ - Cargo 工作区搭建、build_rust.sh 编译链与 cbindgen 头文件导出            │
│ - Apple Silicon ARM64 PMULL CRC32 & UDOT Adler32 硬件加速迁移           │
│ - 错误屏障、日志中枢与取消令牌契约实现                                    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 2: 单格式编解码器安全封装与 ZIP 核心迁移                           │
│ - libdeflate, zstd, fast-lzma2, lz4, snappy, lzfse Safe 封装           │
│ - 并行 ZIP 压缩与解压流式管道迁移 (替换 CTTZipExtract & CTTZipBridge_Zip)│
│ - Swift 6 ~Copyable / Actor 适配与单元测试验证                          │
└───────────────────────────────────┬────────────────────────────────────┘
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 3: libarchive 归档引擎与 7z 固实流式解码迁移                      │
│ - libarchive 深度绑定与流式 Read/Write 回调适配                         │
│ - 7z Header 零拷贝解析器、Solid 固实流式解码与 AES-256 CBC 8-way NEON    │
│ - 安全文件系统落盘引擎 (两阶段权限倒序回写与 APFS 预分配)                  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 4: Swift 核心层瘦身、C 胶水层清理与全量性能回归门禁                 │
│ - 移除 CTTZipBridge 遗留冗余 C 代码，精简 Swift 桥接适配器               │
│ - 525+ Swift 单元测试 100% 通过与 ASan/Miri 0 泄漏确界验证              │
│ - ./scripts/benchmark_ab.sh 5 轮采样评测无回归合并                       │
└────────────────────────────────────────────────────────────────────────┘
```
