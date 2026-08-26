# Implementation Plan: CTTZipBridge 遗留 C 代码库清理与架构收敛 (Feature 171)

**Feature ID**: `171-decommission-legacy-c-bridge-and-converge`  
**Created**: 2026-08-21  
**Status**: Planning Phase  
**Artifact**: Architecture & Implementation Plan

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **组件目标**:
  - `rust/ttzip-glue/src/ffi/compat_ffi.rs`: 补充 Swift 历史 Adapter 依赖的所有 C-ABI 兼容符号；
  - `Sources/CTTZipBridge/`: 删除 93 个 `.c` 文件及冗余子目录，仅保留 `CTTZipBridge.c`；
  - `Package.swift`: 清理 target `CTTZipBridge` 的 headerSearchPath；
  - 运行 `scripts/build_rust.sh` 更新 `ttzip_rust_glue.h` 与 `Vendor/libTTZipVendor.a`；
  - 运行 `swift test` 与 `./scripts/run_local_ci_gate.sh` 验证全绿。

### 1.2 Constitution Check
- [x] **I. 流式第一性**: 所有 C 符号直连 Rust 零拷贝与 $\le 64\text{MB}$ 流式状态机；
- [x] **II. 纵深防御**: 消除遗留 C 文件中的裸指针隐患，统一进入 Safe Rust 边界；
- [x] **III. 确定性确界**: 全部 FFI 导出均包裹 `catch_unwind` 异常屏障；
- [x] **IV. 真实预言机**: 859 个 Swift 单元测试与 7 阶段 CI 门禁全量验证。

---

## 2. Phase 0: Research Items Index

- - R001 [SUBAGENT:research] 《CTTZipBridge 现有 C 符号分类与 Rust 承接审计》：分类 93 个 C 源文件并梳理全部历史 C 符号清单。
- - R002 [SUBAGENT:research] 《Package.swift 目标精简与极简 CTTZipBridge.c 桥接设计》：设计 SPM 极简 C Target 与 Clang Modulemap 映射。

---

## 3. Phase 1: Design Artifacts Index

- **数据模型**: [`specs/171-decommission-legacy-c-bridge-and-converge/data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/171-decommission-legacy-c-bridge-and-converge/data-model.md)
- **强类型契约**:
  - [SUBAGENT:research] `contracts/compat_c_abi_contract.json`
- **快速验证指南**: [`specs/171-decommission-legacy-c-bridge-and-converge/quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/171-decommission-legacy-c-bridge-and-converge/quickstart.md)

---

## 4. Component Changes

### 4.1 新建/修改组件
- `rust/ttzip-glue/src/ffi/compat_ffi.rs`: 导出历史兼容 C 符号
- `rust/ttzip-glue/src/ffi/mod.rs`: 挂载 `compat_ffi`
- `Sources/CTTZipBridge/CTTZipBridge.c`: 极简 C 桥接源文件
- `Package.swift`: 移除冗余搜索路径

### 4.2 删除组件
- `Sources/CTTZipBridge/*.c` (除 `CTTZipBridge.c` 外的 92 个文件)
- `Sources/CTTZipBridge/fast-lzma2/`
- `Sources/CTTZipBridge/lzfse/`
- `Sources/CTTZipBridge/native_inflate/`
- `Sources/CTTZipBridge/snappy/`
- `Sources/CTTZipBridge/zopfli/`
