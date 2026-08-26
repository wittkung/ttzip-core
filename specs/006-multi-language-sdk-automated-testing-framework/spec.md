# Feature Specification: TTZip 全语言 SDK 自动化测试体系与跨语言一致性验证矩阵 (Full Multilingual SDK Testing System)

- **Feature ID**: `006-multi-language-sdk-automated-testing-framework`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `SPECIFIED`
- **Created**: 2026-08-24
- **Target Subsystems & SDKs in Scope**:
  - `core/rust/ttzip-engine` (Rust Core Test Harness & Sanitizers)
  - `core/Sources/TTZipCore` (Swift 6 Strict Concurrency Test Suite)
  - `core/python/` & `core/rust/ttzip-python` (Python PyO3 / PyTest Matrix)
  - `core/sdk/jvm/` (Java 22+ JUnit 5 FFM & Kotlin Coroutines Test Harness)
  - `core/sdk/dart/` (Dart / Flutter `dart test` & Isolate Isolation Test Harness)
  - `core/sdk/dotnet/` (C# .NET 8 xUnit & Span Memory Safety Test Harness)
  - `core/sdk/cpp/` (Modern C++20 RAII Native Test Suite)
  - `core/sdk/c/` (C11 Native ABI Conformance Test Suite)
  - `core/sdk/go/` (Go `go test` with `testing/quick` & `io/fs.FS` Conformance)
  - `core/tests/matrix/` (Cross-Language $N \times N$ Round-Trip Interoperability Suite)
  - `core/tests/security/` (Security Fuzzing, Zip Bomb, Path Traversal & Truncation Matrix)
  - `core/tests/sanitizers/` (ASan / LSan / TSan Automated Leak & Concurrency Detection Gate)
  - `core/tests/benchmark/` (Cross-Language Throughput & RSS Regression Suite)

---

## 1. 业务背景与问题定义 (Problem Statement & Motivation)

TTZip 已具备全语言 Tier-1 原生 SDK（覆盖 Rust、Swift 6、Python、Java 22+、Kotlin、Dart、C#、C++20、C11、Go），并全面根除了子进程调用的反模式，实现了统一的规范化 C-ABI 2.0 与 Project Panama / FFM / CGO 原生直接绑定。

然而，在 SDK 的测试体系层面，当前仓库面临以下核心痛点：
1. **测试碎片化与规范不统一**：各语言 SDK 拥有各自孤立的运行方式，缺乏统一的标准执行入口、输出契约和状态报告。
2. **缺乏跨语言 $N \times N$ 互操作性闭环 (Cross-Language Round-Trip)**：缺乏严格的交叉测试（例如：Swift 创建的加密 7z / tar.zst 能否被 Java FFM、Go 或 Python 准确无损解压并校验哈希一致性）。
3. **安全与边界测试覆盖盲区**：缺乏针对恶意构造文件（目录穿越 `../../`、Zip 炸弹、无效 CRC、损坏数据流、未闭合帧）在全 SDK 层面的防御性测试。
4. **内存泄漏与并发数据竞争自动化门禁缺失**：FFI 跨语言交互极易出现内存泄漏、悬垂指针与并发竞争，缺乏集成的 ASan、LSan、TSan 自动化测试门禁。
5. **性能衰退与内存常驻未量化**：缺乏横向对比各 SDK 在相同 Silesia 黄金语料库下的吞吐率（MB/s）与内存峰值（RSS）的基准测试看板。

---

## 2. 用户故事与核心用例 (User Stories & Scenarios)

### User Story 1 (P1): 一键式全语言原生测试编排与标准报告 (Unified Test Orchestration)
> **作为** TTZip 开发者或 CI/CD 流水线，
> **我希望** 通过统一的命令（如 `bash core/scripts/run_sdk_test_matrix.sh` 或测试套件 CLI）一键运行全部 9 大语言生态的真实单元测试与 FFI 验证，
> **以便于** 在 60 秒内获得符合 JUnit XML / JSON Schema 格式的标准测试矩阵报告，明确各语言生态的通过率与失败根因。

- **Scenario 1.1 (全量通过)**: 在具备环境的机器上执行测试矩阵，9 个语言 SDK 均调用原生测试框架（`cargo test`, `swift test`, `pytest`, `mvn test`/JUnit, `dart test`, `dotnet test`, `go test`, `ctest`），全部执行且输出结构化 JSON 报告。
- **Scenario 1.2 (环境优雅探测与跳过)**: 在缺失某些语言 SDK 工具链的机器上，测试编排器能够自动探测工具链可用性，优雅将不可用生态标记为 `SKIPPED` 并给出安装引导，绝不允许因缺少单一工具链导致全局假阳性或误报失败。

### User Story 2 (P1): 跨语言 $N \times N$ 互操作性与格式一致性验证 (Cross-Language Interoperability)
> **作为** 多语言集成开发者，
> **我希望** 任意 SDK（如 Rust）打包生成的 ZIP、7z、TAR、TAR.ZST 等归档，能被其他任意 SDK（如 Java、Go、Python、Swift、C++）无损读取、流式解压、目录遍历并校验 CRC32，
> **以便于** 保证多语言生态之间归档数据的 100% 格式兼容与互通。

- **Scenario 2.1 (跨语言打包与解压矩阵)**: 选取 5 大核心格式（ZIP、7Z、TAR.GZ、TAR.ZST、TAR.XZ）和 4 类测试数据集（纯文本、多级深层嵌套目录、CJK/Emoji 文件名、稀疏大文件），由 SDK-A 创建归档，并在 SDK-B 至 SDK-I 中分别执行解压与哈希比对，验证提取内容与源文件 SHA-256 绝对一致。
- **Scenario 2.2 (密码与加密交叉验证)**: 验证 Swift / Rust SDK 生成的 AES-256 加密 ZIP / 7z 能够被 Python、Go、Java、C# 等 SDK 凭正确密码成功解压，且在错误密码时统一抛出 `InvalidPasswordException` / `ErrInvalidPassword`。

### User Story 3 (P1): 恶意输入防御与安全性模糊测试 (Security & Malicious Fixture Gates)
> **作为** 安全架构师，
> **我希望** 全套 SDK 均通过恶意样本防御测试，
> **以便于** 杜绝由于解压恶意文件导致的路径穿越、系统提权、磁盘打满或内存炸弹崩溃。

- **Scenario 3.1 (Zip Slip 目录穿越防御)**: 传入包含 `../../../../etc/shadow` 或绝对路径 `/tmp/pwn` 的恶意归档，所有 SDK 必须拦截并安全清洗为相对安全路径，或抛出 `PathTraversalSecurityException`，严禁写入目标解压目录之外。
- **Scenario 3.2 (Zip Bomb 内存炸弹防御)**: 传入高压缩比递归炸弹（如 42.zip），所有 SDK 必须在达到安全解压膨胀比上限（如 1000:1）或内存配额时主动中止并报错，严禁导致进程 OOM 崩溃。
- **Scenario 3.3 (损坏头部与流截断容错)**: 传入被截断或随机翻转字节的损坏归档，所有 SDK 必须安全返回错误码，严禁发生 C-ABI 段错误（SIGSEGV）、空指针解引用或不可恢复 Panic。

### User Story 4 (P2): 内存泄漏与数据竞争检测 (Sanitizers Gate)
> **作为** 核心系统工程师，
> **我希望** 在 CI 中为涉及原生 FFI 的 SDK 启用 ASan/LSan/TSan 检测，
> **以便于** 确保高并发压缩解压场景下 0 内存泄漏、0 Use-After-Free、0 数据竞争。

- **Scenario 4.1 (ASan/LSan 内存泄漏门禁)**: 在 AddressSanitizer 开启状态下，连续执行 10,000 次短生命周期归档创建与解压，验证进程退出时堆内存无任何未释放字节（0 bytes leaked）。
- **Scenario 4.2 (TSan 线程竞争门禁)**: 在 ThreadSanitizer 开启状态下，启动 32 个并发线程同时调用各 SDK 接口执行读写，验证 0 数据竞争告警。

---

## 3. 功能需求 (Functional Requirements: FR-01 ~ FR-24)

### 3.1 统一测试编排与报告体系 (FR-01 ~ FR-06)
- **FR-01**: 必须实现统一的测试运行编排脚本 `core/scripts/run_sdk_test_matrix.sh`，支持全量运行、单语言指定运行（如 `--sdk=python,go`）和按测试分类运行（如 `--category=unit,interop,security`）。
- **FR-02**: 必须支持 `--json <path>` 参数，按结构化 Schema 输出包含整体汇总、各 SDK 详细测试项、耗时、通过状态的 JSON 报告。
- **FR-03**: 必须支持 `--junit <dir>` 参数，输出符合标准 JUnit XML 格式的测试报告，支持 CI 系统原生解析展示。
- **FR-04**: 编排器必须具备动态工具链探测机制，缺失某 SDK 工具链时输出黄色警告并标记 `SKIPPED`，环境完整时强制执行真实测试套件。
- **FR-05**: 必须在根目录提供简便的 Makefile / cargo / npm target（如 `make test-all-sdk`），简化开发者日常测试执行。
- **FR-06**: 测试运行器必须严格遵守退出码规范（所有执行项通过返回 0，存在任何失败返回 1，参数错误返回 2）。

### 3.2 各 SDK 原生单元测试体系补全 (FR-07 ~ FR-14)
- **FR-07 (Rust)**: `core/rust/ttzip-engine` 必须包含 C-ABI 2.0 边界、Fluent Builder、Mmap 流式提取的全面属性测试与单元测试。
- **FR-08 (Swift 6)**: `core/Tests/TTZipTests/` 必须包含 Swift 6 Actor 隔离、`AsyncStream` 进度流、并发取消、大文件解压的真实测试用例。
- **FR-09 (Python)**: `core/python/tests/` 必须覆盖 `zipfile.ZipFile` drop-in 兼容类、`PyBuffer` 零拷贝解压、GIL 释放并发测试与 16 种格式矩阵。
- **FR-10 (Java/Kotlin)**: `core/sdk/jvm/src/test/` 必须实现真正的 JUnit 5 测试套件，包含 Java 22+ Panama FFM `MemorySegment` 读写、CRC 硬件加速与 Kotlin `Flow` 异步测试。
- **FR-11 (Dart/Flutter)**: `core/sdk/dart/test/` 必须实现真正的 `dart test` 测试套件，包含 `dart:ffi` 动态链接库调用、后台 `Isolate` 计算与 `Stream<ArchiveProgress>` 监听。
- **FR-12 (C# / .NET 8)**: `core/sdk/dotnet/` 必须包含 xUnit 测试项目，验证 `ReadOnlySpan<byte>` 零分配切片、`SafeHandleZeroAlloc` 生命周期与 `IAsyncEnumerable` 异步流。
- **FR-13 (C++20 & C11)**: `core/sdk/cpp/` 与 `core/sdk/c/` 必须包含独立的测试套件，验证 C++20 `std::span` / `std::expected` / RAII 管理与 C11 C-ABI 函数指针。
- **FR-14 (Go)**: `core/sdk/go/ttzip/` 必须包含完整的 `go test`，覆盖 `io/fs.FS` 虚拟文件系统、`testing/quick` 快速属性测试与 `context.Context` 超时取消。

### 3.3 跨语言互操作性矩阵 (FR-15 ~ FR-18)
- **FR-15**: 必须在 `core/tests/interop/` 建立标准交叉测试语料生成器，生成包含标准结构、空目录、大体积文件、特殊权限的基准归档。
- **FR-16**: 跨语言测试必须验证任一语言 SDK 打包的文件可被其余全部 SDK 正常列出目录（List）、读取条目元数据（Stat）、解压（Extract）且 SHA-256 完全吻合。
- **FR-17**: 必须覆盖多卷分卷归档（Multi-volume Zip / 7z）在各语言 SDK 之间的流式解压一致性。
- **FR-18**: 必须覆盖 UTF-8 Unicode（中文、日文、韩文、Emoji、法文重音）文件名在所有 SDK 之间的无损编码与解压还原。

### 3.4 恶意输入防御与安全门禁 (FR-19 ~ FR-21)
- **FR-19**: 必须建立 `core/tests/security/fixtures/` 安全测试样本库，包含 Zip Slip 路径穿越、超高压缩比炸弹、畸形 EOCD、损坏 Tar 头等样本。
- **FR-20**: 所有 SDK 在遇到 Zip Slip 样本时必须安全阻断或重命名，严禁越界写盘。
- **FR-21**: 所有 SDK 在遇到损坏流时必须抛出强类型错误（如 `CorruptArchiveException`），严禁发生不可控崩溃。

### 3.5 内存安全门禁与性能基准体系 (FR-22 ~ FR-24)
- **FR-22**: 必须提供 `core/scripts/run_sanitizers.sh`，支持在 AddressSanitizer (ASan) 和 ThreadSanitizer (TSan) 环境下自动化执行原生 FFI 测试。
- **FR-23**: 必须实现跨语言基准测试脚本 `core/scripts/run_sdk_benchmarks.sh`，横向测量各 SDK 在标准 Silesia 语料库上的压缩/解压吞吐量（MB/s）与内存峰值（RSS）。
- **FR-24**: 基准测试结果必须能以 Markdown 格式输出对比表格并支持检测性能衰退（阈值 > 10% 报警）。

---

## 4. 成功指标 (Success Criteria: SC-01 ~ SC-08)

- **SC-01**: 全语言 SDK 测试矩阵在具备工具链的环境下实现 **100% 真实原生测试通过（9/9 生态系统）**。
- **SC-02**: 跨语言 $N \times N$ 互操作性矩阵覆盖率达到 **100%**，各语言交叉解压数据校验一致率达到 100%。
- **SC-03**: 安全测试样本库对所有 SDK 的 Zip Slip 攻击防御成功率达到 **100%（0 越界写入）**。
- **SC-04**: 全套 SDK 自动化测试矩阵单次全量执行时间控制在 **$\le 60$ 秒以内**（多核并发执行）。
- **SC-05**: 原生 C-ABI、C++、Go CGO、Swift、Python 扩展在 ASan / TSan 下运行 **0 内存泄漏（0 byte leak）、0 数据竞争（0 data race）**。
- **SC-06**: 测试报告必须 100% 符合 JSON Schema 契约并可生成兼容 Jenkins / GitHub Actions 的 JUnit XML 报告。
- **SC-07**: 所有 SDK 源码及测试代码严格遵守单文件行数 $\le 800$ 行的架构约束。
- **SC-08**: 开发者在本地只需运行单条指令即可完成全套测试体系的启动与结果汇总。

---

## 5. 实体与数据模型 (Entities & Testing Data Model)

1. **`SdkTestRunContext`**: 包含执行模式、指定语言过滤列表、指定测试分类、输出格式、临时工件路径。
2. **`SdkTestResult`**:
   - `language`: SDK 生态（`rust`, `swift`, `python`, `jvm`, `dart`, `dotnet`, `cpp`, `c`, `go`）
   - `toolchainAvailable`: 布尔值，工具链是否可用
   - `status`: `passed` | `failed` | `skipped`
   - `durationMs`: 执行耗时（毫秒）
   - `totalTests`: 总测试用例数
   - `passedTests`: 通过用例数
   - `failedTests`: 失败用例数
   - `errorDetails`: 错误堆栈或异常描述
3. **`InteropTestMatrixEntry`**:
   - `creatorSdk`: 归档创建方 SDK
   - `extractorSdk`: 归档解压方 SDK
   - `format`: 归档格式（`zip`, `7z`, `tar.gz`, `tar.zst`）
   - `fixtureType`: 语料类型（`text`, `nested`, `unicode`, `large`）
   - `status`: `passed` | `mismatch` | `error`
   - `sha256Match`: 布尔值，解压内容 SHA-256 是否完全匹配
