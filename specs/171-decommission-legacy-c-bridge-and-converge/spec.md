# Feature Specification: CTTZipBridge 遗留 C 代码库清理与架构收敛 (Feature 171)

**Feature ID**: `171-decommission-legacy-c-bridge-and-converge`  
**Created**: 2026-08-21  
**Status**: In Progress (Specification Phase)  
**Priority**: P0 (Codebase Simplification, Eliminating Redundant C Code, Build Speedup)

---

## 1. Executive Summary & Background

在 Feature 168 中，TTZip 已经在 `rust/ttzip-glue` 中用 Safe Rust 全面重构了全部底层算子与胶水管道（ARM64 NEON PMULL CRC32、UDOT Adler32、8-way AES-256、7z SHA-256 KDF、Codecs Safe Wrappers、流适配器、APFS 预分配、两阶段权限回写、ZIP/7z 原生引擎），并经过了 Feature 169 的 859 个 Swift 测试 + 85+ 个 Rust 工业级测试的全面验证。

然而，当前 `Sources/CTTZipBridge/` 目录下仍存留了 93 个历史 `.c` 文件及冗余子目录（`fast-lzma2/`, `lzfse/`, `native_inflate/`, `snappy/`, `zopfli/`）。这些 C 源文件存在以下问题：
1. **编译冗余与时间浪费**: SwiftPM 每次全量构建时仍需要编译这 93 个 C 源文件，严重拖慢了开发与测试循环；
2. **双重实现与维护负担**: 相同的算法与逻辑在 C 和 Rust 两端同时存在，容易造成认知混乱和版本偏差；
3. **内存安全风险隐患**: 遗留 C 源文件中包含裸 `malloc`/`free`、指针越界与并发竞态隐患，破坏了系统的内存安全保障。

本特性的目标是：**系统性清理并下线 `Sources/CTTZipBridge/` 中已被 Rust 完全替代的历史 C 源文件与冗余子目录，将 CTTZipBridge 升级为纯粹的轻量级 C-ABI 桥接模块（仅依赖 `ttzip_rust_glue.h` 与必要的基础声明），同时保证 Swift 6 的全部 859 个测试与本地 CI/CD 门禁 100% 绿色通过。**

---

## 2. User Scenarios

### User Scenario 1 (US1) - 极速 SwiftPM 增量与全量构建 (Blazing Fast SwiftPM Clean Build)
- **As a**: 核心开发者
- **I want to**: 执行 `swift build` 或 `swift test` 进行全新编译
- **So that**: 编译过程无需重复编译 93 个 C 源文件，SPM 构建耗时从数十秒降低至数秒内，极速反馈。

### User Scenario 2 (US2) - 零冗余代码库与清晰架构边界 (Single Source of Truth)
- **As a**: 架构师与代码审查者
- **I want to**: 代码库中每一个压缩算法、密码学算子、文件系统操作有且仅有一套 Safe Rust 实现
- **So that**: 彻底消除双重实现的技术债务，所有内存安全与硬件加速收益集中于 `ttzip-glue`。

### User Scenario 3 (US3) - 100% 接口兼容与零功能回退 (Zero Functional Regressions)
- **As a**: 终端用户与上层 GUI / CLI 调用方
- **I want to**: 底层 C 代码清理过程完全平滑
- **So that**: Swift 6 `TTZipCore`、`TTZipApp`、`ttzip-cli` 行为完全一致，859 项测试与 CI 门禁全绿通过。

---

## 3. Functional Requirements

### REQ-001: 遗留 C 源文件与子目录分类审计 (Legacy C Code Audit)
- 审计 `Sources/CTTZipBridge/` 下的全部 93 个 `.c` 文件：
  - 核心算子类（CRC32, Adler32, AES, SHA-256, BCJ）：确认 Rust 已 100% 承接并可安全删除；
  - 编解码类（deflate, zstd, fl2, lzfse, snappy, chardet）：确认已由 `Vendor/libTTZipVendor.a` + `ttzip-glue` 链接，删除内部副本；
  - 归档解析类（7z, zip, tar, dmg）：确认高层统一走 `ttzip_rust_inspect_archive` / `extract` / `create`，移除冗余 C 解析代码；
  - 辅助工具类（strnatcmp, magic sniff, mem budget）：在 Rust 或纯 Swift 模块中提供安全对等实现。

### REQ-002: 导出缺失的兼容 C-ABI 符号 (C-ABI Forwarding & Compatibility Shims)
- 在 `rust/ttzip-glue/src/ffi/` 中为 `TTZipCore` 中仍有直接引用的少量辅助 C 符号（如 `ttzip_strnatcasecmp`, `ttzip_magic_sniff_buffer`, `ttzip_core_aligned_alloc_16k` 等）补全 Safe Rust 导出；
- 更新 `Sources/CTTZipBridge/include/ttzip_rust_glue.h` 与 `ttzip_platform.h`。

### REQ-003: 移除冗余 C 源文件与更新 Package.swift (Decommission C Files & Target Config)
- 从 `Sources/CTTZipBridge/` 中安全删除已被 Rust 替代的 `.c` 文件和嵌套子目录（`fast-lzma2/`, `lzfse/`, `native_inflate/`, `snappy/`, `zopfli/`）；
- 在 `Sources/CTTZipBridge/` 中仅保留一个极简的桥接源文件（`CTTZipBridge.c`，仅用于触发 C Module 编译）与头文件；
- 清理 `Package.swift` 中 `CTTZipBridge` 的冗余 `.headerSearchPath` 配置。

### REQ-004: 全量测试验证与本地 CI 门禁核销 (Full Test & CI Gate Verification)
- 运行 `swift test` 确保 859 项 Swift 测试 100% 绿色通过；
- 运行 `./scripts/run_local_ci_gate.sh` 确保 7 阶段本地 CI 门禁 100% 通过。

---

## 4. Success Criteria

1. **代码库精简**: `Sources/CTTZipBridge/` 源文件数量从 93 个精简至 $\le 2$ 个，消除 15,000+ 行遗留 C 代码；
2. **构建性能提升**: `swift build --clean && swift build` 全量编译耗时下降 $\ge 50\%$；
3. **零功能回退**: Swift 6 `swift test` 全部 859 项测试 100% 通过（0 失败，0 告警）；
4. **CI 门禁全绿**: 运行 `./scripts/run_local_ci_gate.sh` 7 阶段全绿（耗时 $\le 10\text{s}$）。
