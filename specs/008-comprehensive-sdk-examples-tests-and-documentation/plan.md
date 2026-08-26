# Implementation Plan: TTZip 全格式与高级设置深度接入示例、全套 SDK 测试用例与完整接入文档体系 (Feature 008)

- **Feature ID**: `008-comprehensive-sdk-examples-tests-and-documentation`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `PLANNED`
- **Created**: 2026-08-24

---

## 1. 技术上下文与架构全景 (Technical Context)

### 目标架构
1. **全 16 种格式矩阵**：ZIP (Deflate/Zstd/Bzip2/Store), 7Z (LZMA2/BCJ2), TAR, TAR.GZ, TAR.ZST, TAR.BZ2, TAR.XZ, GZ, ZST, BZ2, XZ, ISO, CPIO, AR。
2. **8 大高级配置选项**：
   - `threads`: 1–128 并发工作线程控制
   - `level`: 1–9 (Deflate/LZMA2), 1–22 (Zstandard)
   - `encryption`: AES-256 / ZipCrypto + 密码保护
   - `solid`: 固实压缩块控制 (7z)
   - `recovery_percentage`: Reed-Solomon RS-ECC 恢复记录 (5%–20%)
   - `filters`: Glob 包含/排除 (`--include`, `--exclude`) 与正则表达式
   - `progress_stream`: 响应式流式进度与优雅取消
   - `in_memory_vfs`: 内存缓冲区流与零拷贝 Span / MemorySegment 编解码
3. **分期规划**：
   - Phase 1: 编写体系化 SDK 开发者文档与高级设置实战秘籍 (`docs/sdk/`)
   - Phase 2: 编写并完善全 10 大语言生态的高级设置与多格式活体接入示例 (`examples/`)
   - Phase 3: 扩充各语言 SDK 自动化测试用例，覆盖 16 格式与高级参数断言
   - Phase 4: 运行全量本地 CI 门禁与 Out-Of-Tree 冒烟测试验证，确保 100% 绿色交付

---

## 2. 实施任务规划 (Phased Tasks)

### Phase 1: 体系化 SDK 开发者文档与高级设置秘籍 (T001 - T005)
- [ ] **T001**: 编写 `docs/sdk/README.md` (全语言 SDK 导航、16 格式支持矩阵与性能指标速查表)。
- [ ] **T002**: 编写 `docs/sdk/RUST_GUIDE.md`, `docs/sdk/SWIFT_GUIDE.md`, `docs/sdk/PYTHON_GUIDE.md`。
- [ ] **T003**: 编写 `docs/sdk/JVM_KOTLIN_GUIDE.md`, `docs/sdk/CPP_C_GUIDE.md`, `docs/sdk/GO_GUIDE.md`。
- [ ] **T004**: 编写 `docs/sdk/DART_FLUTTER_GUIDE.md`, `docs/sdk/DOTNET_GUIDE.md`, `docs/sdk/NODE_TYPESCRIPT_GUIDE.md`。
- [ ] **T005**: 编写 `docs/sdk/ADVANCED_SETTINGS_RECIPES.md` (加密、RS-ECC、VFS、流式进度、取消控制的多语言代码对照)。

### Phase 2: 10 大语言全格式与高级设置可运行示例工程 (`examples/`) (T006 - T015)
- [ ] **T006**: 在 `examples/rust/` 编写覆盖 16 格式与高级配置的 Rust 示例。
- [ ] **T007**: 在 `examples/swift/` 编写 Swift 6 Actor 与 AsyncStream 高级示例。
- [ ] **T008**: 在 `examples/python/` 扩充 Python 16 格式、Zstd Level 22、AES-256 加密与 ZipFile 平替高级示例。
- [ ] **T009**: 在 `examples/jvm/` 与 `examples/kotlin/` 编写 Java Panama FFM 与 Kotlin Flow 高级配置示例。
- [ ] **T010**: 在 `examples/cpp/` 扩充 C++20 `std::span` 零拷贝与 RAII 高级示例。
- [ ] **T011**: 在 `examples/c/` 扩充 C11 原生高级选项配置示例。
- [ ] **T012**: 在 `examples/go/` 扩充 Go `io/fs.FS` 虚拟文件系统与 `context.Context` 取消高级示例。
- [ ] **T013**: 在 `examples/dart/` 扩充 Dart / Flutter `Isolate` 与 `Stream<ArchiveProgress>` 响应式示例。
- [ ] **T014**: 在 `examples/dotnet/` 扩充 C# .NET 8 `ReadOnlySpan` 与 `IAsyncEnumerable` 异步流示例。
- [ ] **T015**: 在 `examples/node/` 扩充 Node.js / TypeScript Promise / Stream 示例。

### Phase 3: 全语言 SDK 原生测试套件 16 格式与高级设置扩充 (T016 - T021)
- [ ] **T016**: 在 `sdk/jvm/src/test/java/com/ttzip/` 扩充多格式与高级参数断言测试。
- [ ] **T017**: 在 `sdk/go/ttzip/` 扩充多格式与上下文取消测试。
- [ ] **T018**: 在 `sdk/cpp/` 与 `sdk/c/` 扩充 C++20 / C11 选项结构体与边界测试。
- [ ] **T019**: 在 `sdk/dotnet/` 扩充 C# 密码校验与异步枚举测试。
- [ ] **T020**: 在 `sdk/dart/test/` 扩充 Dart 隔离测试。
- [ ] **T021**: 在 `python/tests/` 扩充 Python 极端压缩级别与异常测试。

### Phase 4: 本地 CI 与冒烟测试全量验证 (T022 - T025)
- [ ] **T022**: 执行 `make test-all-sdk` 验证 9 大语言原生测试集全绿。
- [ ] **T023**: 执行 `make test-out-of-tree-smoke` 验证无源码环境多语言示例通过。
- [ ] **T024**: 执行 `scripts/lint_loc_gate.py` 验证全量文件 $\le 800$ LOC。
- [ ] **T025**: 提交代码并推送远程主分支。
