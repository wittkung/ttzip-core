# Tasks: TTZip 现代化终端交互式 TUI 与独立 CLI 引擎 (Feature 170)

**Feature ID**: `170-rust-interactive-tui-engine`  
**Created**: 2026-08-21  
**Status**: Ready for Implementation  
**Specification**: [`specs/170-rust-interactive-tui-engine/spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/170-rust-interactive-tui-engine/spec.md)  
**Plan**: [`specs/170-rust-interactive-tui-engine/plan.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/170-rust-interactive-tui-engine/plan.md)

---

## Phase 1: Crate 骨架与 Clap 命令行分发 (Crate Setup & Clap CLI)

- [x] T001 [P] 初始化 `rust/ttzip-tui/Cargo.toml` 并配置 workspace 依赖 in `rust/ttzip-tui/Cargo.toml`
- [x] T002 [P] 实现 `rust/ttzip-tui/src/main.rs` 中的 Clap 命令行参数解析与无头/交互路由 in `rust/ttzip-tui/src/main.rs`
- [x] T003 [P] 编写 Universal Standalone 二进制打包脚本 in `scripts/build_tui.sh`

---

## Phase 2: VFS 虚拟文件系统与即时模糊搜索 (US1, US3 - VFS & Search)

- [x] T004 [P] [US1] 实现归档层级树节点解析器与折叠/展开状态机 in `rust/ttzip-tui/src/vfs.rs`
- [x] T005 [P] [US3] 实现基于 `fuzzy-matcher` 的随打随搜打分与高亮区间提取器 in `rust/ttzip-tui/src/vfs.rs`

---

## Phase 3: 免解压流式预览引擎 (US2 - In-Terminal Preview)

- [x] T006 [P] [US2] 实现 64KB 文本流式语法高亮检测器 in `rust/ttzip-tui/src/preview.rs`
- [x] T007 [P] [US2] 实现 16 字节对齐格式化 Hex Dump 引擎 in `rust/ttzip-tui/src/preview.rs`

---

## Phase 4: UI 布局与主题渲染 (US1, US4 - Ratatui Layout & Dashboard)

- [x] T008 [P] [US1] 实现 Header、Footer 与 Theme 样式调色板 in `rust/ttzip-tui/src/ui/mod.rs`
- [x] T009 [P] [US1] 实现 Explorer 目录树与条目表格组件 in `rust/ttzip-tui/src/ui/explorer.rs`
- [x] T010 [P] [US3] 实现 Search 模糊搜索弹窗与高亮覆盖层 in `rust/ttzip-tui/src/ui/search.rs`
- [x] T011 [P] [US4] 实现 Progress 实时吞吐、进度条与 CPU 负载监控弹窗 in `rust/ttzip-tui/src/ui/progress.rs`

---

## Phase 5: 事件循环与状态机编排 (US1, US4 - Event Loop & App State)

- [x] T012 [US1] 实现终端键盘/鼠标事件轮询与后台通道集成 in `rust/ttzip-tui/src/event.rs`
- [x] T013 [US1] 实现 AppState 状态转移与原子取消集成 in `rust/ttzip-tui/src/app.rs`

---

## Phase 6: 全量编译验证与收敛 (Converge & Verification)

- [x] T014 运行 `cargo test --manifest-path rust/ttzip-tui/Cargo.toml` 验证 TUI 单元测试
- [x] T015 运行 `./scripts/build_tui.sh --release` 编译 Universal 二进制并验证无头/列表模式
