# Phase 0 Research: TTZip 现代化终端交互式 TUI 与独立 CLI 引擎 (Feature 170)

**Feature ID**: `170-rust-interactive-tui-engine`  
**Created**: 2026-08-21  
**Status**: Completed  
**Artifact**: Phase 0 Technical Research & Architecture Invariants

---

## 1. 调研项与决策矩阵

### R001: 终端 UI 框架与事件循环选型 (`ratatui` + `crossterm`)

- **Decision (选定方案)**:
  采用 **`ratatui = "0.28"` + `crossterm = "0.28"` + `crossbeam-channel` 双线程架构**：
  1. **UI 渲染与事件主线程**: 监听键盘/鼠标事件并执行 60 FPS 即时模式布局渲染；
  2. **后台工作线程**: 负责解压、压缩、预览流拉取，并通过无锁通道推送进度帧，主线程永不阻塞。
- **Rationale (选择理由)**:
  `ratatui` 是目前工业级 CLI 工具（如 `gitui`, `bottom`, `lazygit` 的 Rust 对标）事实上的黄金标准。双线程事件分发杜绝了在大文件解压期间终端 UI 假死问题。
- **Alternatives Considered (被否决方案)**:
  - *方案 A（单线程阻塞循环）*：被否决。后台执行压缩解压时将直接导致按键无响应与画面冻结。
  - *方案 B（基于 C `ncurses` 封装）*：被否决。C API 存在全局静态状态、跨平台 Windows/macOS 支持脆弱，易发内存破坏。
- **Source (查阅依据)**:
  - [Ratatui Documentation & Examples](https://ratatui.rs/)
  - `rust/ttzip-glue/src/runtime/cancellation.rs`

---

### R002: 归档 VFS 目录树与即时模糊搜索算法 (`fuzzy-matcher` / `nucleo`)

- **Decision (选定方案)**:
  采用 **内存前缀树 `VfsTree` + `fuzzy_matcher::skim::SkimMatcherV2`**：
  1. 在初次 Inspect 归档条目时，按 `/` 分隔符构建扁平索引数组与层级树状节点；
  2. 用户在 `/` 搜索模式下输入时，`SkimMatcherV2` 并行对全部条目路径进行打分并提取字符高亮索引区间。
- **Rationale (选择理由)**:
  `SkimMatcherV2` 单核匹配 50,000 条路径仅需约 $3\text{ms}$，能够实现随打随搜（Instant Search-as-you-type）的丝滑交互体验。
- **Alternatives Considered (被否决方案)**:
  - *方案 A（简单子串包含 `contains`）*：被否决。无法处理缩写或错位搜索（如 `crc32c` 无法搜出 `CTTZipCRC32Neon.c`）。
- **Source (查阅依据)**:
  - [fuzzy-matcher crate documentation](https://docs.rs/fuzzy-matcher/)
  - `rust/ttzip-glue/src/zip/mod.rs`

---

### R003: 终端免解压 QuickLook 预览实现 (In-Terminal Stream Preview)

- **Decision (选定方案)**:
  采用 **流式内存切片解压 + `syntect` 语法探测 + 格式化 Hex Dump 引擎**：
  1. 文本文件（`.rs`, `.swift`, `.c`, `.json`, `.md` 等）：仅解压前 64KB，根据后缀识别语言并转换为 ANSI 24-bit TrueColor 转义字符；
  2. 二进制文件：以标准 16 字节对齐 Hex 格式输出（Offset | Hex Bytes | ASCII Sidebar）；
  3. 超过 64KB 部分显示 `[Truncated: File is larger than preview limit]`，保护内存常驻。
- **Rationale (选择理由)**:
  用户绝大部分预览诉求只是快速核对文件头或配置片段，流式截断预览避免了将整个几百 MB 视频或日志全量加载到内存中。
- **Alternatives Considered (被否决方案)**:
  - *方案 A（解压到临时文件后调用外部 `bat` 或 `less`）*：被否决。产生不必要的磁盘 I/O，且临时文件存在残留风险。
- **Source (查阅依据)**:
  - `rust/ttzip-glue/src/archive/stream_adapter.rs`
  - `rust/ttzip-glue/src/sevenz/decoder.rs`

---

### R004: 统一 CLI 命令行调度与 Universal Standalone 二进制打包 (Clap & Packaging)

- **Decision (选定方案)**:
  采用 **`clap = { version = "4.5", features = ["derive"] }` + `scripts/build_tui.sh` (lipo universal)**：
  - 支持无头命令（`ttzip x file.zip -o ./out`、`ttzip c out.zip file1 file2`）与交互 TUI（`ttzip file.zip` 或纯命令 `ttzip`）；
  - 脚本自动调用 `cargo build --release` 编译 `aarch64` 与 `x86_64` 并使用 `lipo` 合并为单个 Mach-O 二进制 `bin/ttzip`。
- **Rationale (选择理由)**:
  将 CLI 与 TUI 统一为单一入口，用户无需记忆两个不同命令，且纯静态链接保证在任意 macOS 14.0+ 机器上开箱即用。
- **Alternatives Considered (被否决方案)**:
  - *方案 A（拆分为 `ttzip-cli` 与 `ttzip-tui` 两个独立二进制）*：被否决。增加了分发与版本同步的心智负担。
- **Source (查阅依据)**:
  - [Clap Documentation](https://docs.rs/clap/)
  - `scripts/build_rust.sh`
