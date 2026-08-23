# Implementation Plan: 172-purge-legacy-c-bridge

## Technical Context & Constitution Check
- **Language & Runtime**: C11 -> Swift 6.0 + Safe Rust 1.80+ FFI.
- **Concurrency Model**: Swift 6 Strict Concurrency (`@Sendable`, `Sendable`).
- **Target OS**: macOS 14.0+ (Apple Silicon ARM64 & x86_64).
- **Constitution Compliance**: Zero memory leaks, zero bare pointers in hot paths, 0 warnings, 100% test pass rate.

---

## Phase 0: Research & Grounded Inventory
- [x] R001 [SUBAGENT:research] 《C 胶水层全符号审计与清退分类》：遍历 93 个 `.c` 文件并分类为 3 个批次。
- [x] R002 [SUBAGENT:research] 《Swift 适配层与 Rust C-ABI 映射关系》：确定 `ttzip_rust_*` 对应函数。
- [x] R003 [SUBAGENT:research] 《工具函数 Swift 6 原生化方案》：确定 `String`, `UnsafeMutableRawPointer`, `ProcessInfo`, `ContinuousClock` 替换路径。

---

## Phase 1: 契约与设计工件
- `contracts/legacy_c_purge_contract.json`: 定义 C 桥接保留符号与 Rust/Swift 重定向契约。
- `data-model.md`: 记录 CTTZipBridge 导出符号与生命周期模型。
- `quickstart.md`: 记录编译验证与回归测试场景。

---

## Phase 2: Implementation Steps
1. **Batch 1 Purge**: 物理删除 44 个孤立废弃/实验性 C 文件及对应孤立 Swift 文件。
2. **Batch 2 Redirect & Purge**: 将 33 个密码/SIMD/编解码/归档 C 文件的 Swift 调用点重定向至 `ttzip_rust_*`，随后物理删除。
3. **Batch 3 Native & Convergence**: 将 16 个工具函数迁移至 Swift 原生，将其余必要符号收敛至单一 `CTTZipBridge.c`。
4. **Header Cleanup**: 清理 `include/` 冗余头文件，仅保留必要对外头文件与 `ttzip_rust_glue.h`。
5. **SPM & CI Verification**: 运行 `swift test` (859 项测试) 与 `./scripts/run_local_ci_gate.sh` (7 阶段门禁) 验证 0 Warnings 100% 绿色通过。
