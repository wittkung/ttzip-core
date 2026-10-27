# Implementation Plan: TTZip 现代化终端交互式 TUI 与独立 CLI 引擎 (Feature 170)

**Feature ID**: `170-rust-interactive-tui-engine`  
**Created**: 2026-08-21  
**Status**: Planning Phase  
**Artifact**: Architecture & Implementation Plan

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **组件目标**:
  - 创建 Rust Workspace 独立二进制成员 `rust/ttzip-tui`；
  - 依赖 `ttzip-glue` 核心库；
  - 基于 `ratatui` + `crossterm` 构建 TUI；
  - 基于 `clap` 构建统一命令行分发；
  - 编写 `scripts/build_tui.sh` 输出单文件 Universal 二进制 `bin/ttzip`。

### 1.2 Constitution Check
- [x] **I. 流式第一性**: 预览仅拉取 64KB 切片，常驻内存严控 $\le 16\text{MB}$；
- [x] **II. 纵深防御**: TUI 解压操作 100% 走 `SafeExtractEngine` 路径防御；
- [x] **III. 确定性确界**: 按键 Esc/q 触发原子取消令牌，无挂起线程；
- [x] **IV. 真实预言机**: 提供完整的自动化单测与无头模式差分回归。

---

## 2. Phase 0: Research Items Index

- - R001 [SUBAGENT:research] 《终端 UI 框架与事件循环选型》：设计 ratatui + crossterm + crossbeam-channel 双线程渲染架构。
- - R002 [SUBAGENT:research] 《归档 VFS 目录树与即时模糊搜索算法》：构建 VfsTree 与 SkimMatcherV2 毫秒级打分高亮引擎。
- - R003 [SUBAGENT:research] 《终端免解压 QuickLook 预览实现》：设计 64KB 流式语法高亮与格式化 Hex Dump 引擎。
- - R004 [SUBAGENT:research] 《统一 CLI 命令行调度与 Universal Standalone 二进制打包》：设计 clap 统一分发与 lipo 单文件打包。

---

## 3. Phase 1: Design Artifacts Index

- **数据模型**: [`specs/170-rust-interactive-tui-engine/data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/170-rust-interactive-tui-engine/data-model.md)
- **强类型契约**:
  - [SUBAGENT:research] `contracts/tui_event_contract.json`
  - [SUBAGENT:research] `contracts/tui_vfs_tree_contract.json`
- **快速验证指南**: [`specs/170-rust-interactive-tui-engine/quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/170-rust-interactive-tui-engine/quickstart.md)

---

## 4. Component Changes

### 4.1 新建组件
- `rust/ttzip-tui/Cargo.toml`
- `rust/ttzip-tui/src/main.rs`: CLI 入口与 Clap 解析
- `rust/ttzip-tui/src/app.rs`: 核心状态机与键盘事件映射
- `rust/ttzip-tui/src/event.rs`: 终端事件轮询与后台通道集成
- `rust/ttzip-tui/src/vfs.rs`: 归档层级树与模糊搜索匹配
- `rust/ttzip-tui/src/preview.rs`: 文本语法高亮与 Hex 格式化
- `rust/ttzip-tui/src/ui/mod.rs`: 根布局组合
- `rust/ttzip-tui/src/ui/explorer.rs`: 目录树渲染组件
- `rust/ttzip-tui/src/ui/search.rs`: 模糊搜索弹窗
- `rust/ttzip-tui/src/ui/progress.rs`: 实时吞吐仪表板
- `rust/ttzip-tui/src/ui/theme.rs`: macOS 风格主题色调
- `scripts/build_tui.sh`: Universal 二进制构建脚本

### 4.2 修改组件
- `rust/Cargo.toml`: 将 `ttzip-tui` 加入 workspace 成员。
