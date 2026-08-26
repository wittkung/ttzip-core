# Feature Specification: TTZip 全格式与高级设置深度接入示例、全套 SDK 测试用例与完整接入文档体系 (Comprehensive Multi-Format & Advanced Settings Living Examples, Full SDK Test Suites, and Comprehensive Developer Guides)

- **Feature ID**: `008-comprehensive-sdk-examples-tests-and-documentation`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `SPECIFIED`
- **Created**: 2026-08-24
- **Target Subsystems & Packages in Scope**:
  - `core/docs/sdk/` (Comprehensive Multi-Language Developer Manuals & Advanced Setting Recipes)
  - `core/examples/` (Multi-Format & Advanced Settings Living Examples for all 10 Ecosystems)
  - `core/sdk/` & `core/tests/` (100% Format & Advanced Setting Test Suites per SDK)
  - `core/tests/matrix/` (16 Formats $\times$ Advanced Settings Interoperability Gate)
  - `core/scripts/` (Automated Doc Examples Smoke Verification Gate)

---

## 1. 业务背景与问题定义 (Problem Statement & Motivation)

TTZip 的底层微内核与全语言零配置分发体系已经就绪。然而，在面对复杂生产级业务场景时，不同语言的集成开发者需要深入使用全部 **16 种归档与压缩格式** 以及 **8 大高级核心设置**（多线程并发度、压缩级别 1–22、固实压缩 Solid Block、AES-256/Argon2id 强加密、Reed-Solomon RS-ECC 恢复记录、Glob/Regex 过滤规则、响应式流式进度与中断、内存 VFS / Span 零拷贝流）。

当前代码库在以下方面仍需建立标准体系：
1. **全格式与高级设置活体示例缺失**：现有 quickstart 样例主要展示基础 zip 压缩，缺乏 7z 固实加密、tar.zst 多线程流、RS-ECC 容错生成、VFS 内存挂载等高级配置的端到端可运行代码。
2. **SDK 原生测试覆盖不均衡**：部分语言 SDK 单元测试仅覆盖了常规格式，缺乏对 16 种格式矩阵和极端参数（如 level=22, recovery=20%）的显式断言测试。
3. **缺乏体系化的 SDK 开发者指南与秘籍 (Recipes)**：缺乏包含语言专属惯用法（如 Swift 6 `AsyncStream`, Kotlin `Flow`, C# `IAsyncEnumerable`, Go `io/fs.FS`, C++20 `std::span`/`std::expected`）的完整接入文档和复制即用的最佳实践。

---

## 2. 用户故事与核心用例 (User Stories & Scenarios)

### User Story 1 (P1): 10 大语言生态全格式与高级设置活体接入示例 (Living Examples across 10 Ecosystems)
> **作为** 任意主流技术栈开发者（Rust, Swift 6, Python, Java 22+, Kotlin, C++20, C11, Go, Dart/Flutter, C# .NET 8, Node.js/TypeScript），
> **我希望** 在 `examples/<lang>/` 中直接找到覆盖全部 16 种格式与 8 大高级配置的独立可运行工程，
> **以便于** 在 10 秒内直接复制生产级代码接入项目。

- **Scenario 1.1 (全 16 格式矩阵覆盖)**: 示例包含 ZIP (Deflate/Zstd/Bzip2/Store)、7Z (LZMA2/BCJ2)、TAR (POSIX pax)、TAR.GZ/TGZ、TAR.ZST/TZST、TAR.BZ2/TBZ2、TAR.XZ/TXZ、单流 (GZ/ZST/BZ2/XZ) 及只读 (ISO/CPIO/AR)。
- **Scenario 1.2 (高级配置生产范式)**: 示例展示 (1) 自定义线程池并发度，(2) 极致压缩级别 (Level 1–22)，(3) 7z 固实字典与分卷，(4) AES-256 头部与内容双重加密，(5) 5%–20% Reed-Solomon 恢复记录，(6) Glob/Regex 白名单与黑名单过滤，(7) 实时进度监听与优雅取消，(8) 内存流与零拷贝 Span/MemorySegment 处理。

### User Story 2 (P1): 16 格式与高级参数 SDK 自动化测试套件 (Full Format & Advanced Setting Test Matrix)
> **作为** SDK 质量负责人，
> **我希望** 每个 SDK 的原生测试框架（`cargo test`, `swift test`, `pytest`, `mvn test`, `ctest`, `go test`, `dart test`, `dotnet test`）均包含针对全部 16 种格式及高级特性的自动化断言，
> **以便于** 确保每个生态的绑定都 100% 具备工业级稳定性与边界安全。

- **Scenario 2.1 (参数边界断言)**: 测试不同压缩级别、密码校验（正确与错误分支）、恢复记录自动修复能力、路径过滤正则匹配。
- **Scenario 2.2 (跨语言格式兼容断言)**: 验证任一语言 SDK 使用高级配置（如 Zstd Level 19 加密 7z）生成的文件可被其他语言 SDK 无损读取。

### User Story 3 (P1): 体系化多语言接入文档与高级场景实战秘籍 (SDK Guides & Advanced Recipes)
> **作为** 架构师与技术文档阅读者，
> **我希望** 查阅结构清晰、排版专业、附带完整类型签名与错误处理说明的官方接入文档与高级秘籍，
> **以便于** 快速选型、评估性能并遵循最佳实践集成。

- **Scenario 3.1 (全语言导航与格式支持矩阵)**: `docs/sdk/README.md` 提供完整格式兼容矩阵、性能基准速查与各语言 SDK 导航。
- **Scenario 3.2 (专属语言深度指南)**: 为 Rust, Swift, Python, Java/Kotlin, C++/C, Go, Dart/Flutter, .NET, Node.js 分别撰写详细的手册。
- **Scenario 3.3 (高级设置实战秘籍库)**: `docs/sdk/ADVANCED_SETTINGS_RECIPES.md` 针对加密、容错恢复、内存流、流式进度、取消控制提供多语言代码对照。

---

## 3. 验收标准 (Acceptance Criteria)

1. **示例可执行性**: 所有 `examples/*/` 目录下的示例工程均可通过对应语言的构建工具直接编译与执行，且退出码为 0。
2. **测试覆盖率**: 9 大语言 SDK 原生测试集新增并跑通全部 16 种格式与高级设置测试用例。
3. **文档完备性**: `docs/sdk/` 包含 1 篇 Master README、9 篇语言深度指南与 1 篇高级设置实战秘籍，全部包含真实代码与类型签名。
4. **CI 门禁**: `make -C core test-all-sdk` 与 `make -C core test-out-of-tree-smoke` 100% 通过。
5. **代码规范**: 所有新增/修改文件遵循 $\le 800$ LOC 单文件门禁。
