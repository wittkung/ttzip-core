# TTZip Project Constitution & Architectural Governance Guardrails

- **Scope**: All Subsystems, Microkernel, C-ABI Bridges, and Multi-Language SDKs
- **Version**: 2.0.0
- **Updated**: 2026-08-24

---

## 1. 核心架构宪章原则 (Core Architectural Invariants)

1. **100% Mozilla UniFFI Mandatory Standard (Mozilla UniFFI 跨语言强制标准原则)**:
   - **唯一互操作协议**：Swift 6、Python、Kotlin 等所有 Tier-1 SDK 必须 100% 基于 Mozilla UniFFI Proc-Macro (`#[uniffi::export]`, `#[derive(uniffi::Record)]`, `#[derive(uniffi::Object)]`) 自动生成安全绑定与内存屏障。
   - **严禁手写 C-ABI**：绝对禁止手写非受管 C 指针 (`*const c_char`, `unsafe extern "C"`, `void*`) 或自行管理跨语言裸指针分配与释放。
   - **单一数据源 (SSOT) 与禁止重复实现**：所有计算密集型、I/O 密集型、密码哈希、数据结构（Trie 树/VFS）、原地归档改写、分卷切片与 i18n 词库必须在 Rust 内核实现并通过 UniFFI 统一导出。Swift 严禁在应用层重复实现已存在于 Rust 的逻辑，严禁在 Swift 维护重复的静态数据字典。
   - **零子进程政策 (Zero-Subprocess Policy)**：全语言 SDK 严禁通过 `ProcessBuilder`, `subprocess.run`, `Process.run`, `std::system` 启动 CLI 二进制。

2. **Swift 6 职责边界与纯粹性 (Swift 6 Presentation Boundary)**:
   - Swift 专职负责 SwiftUI 声明式渲染、`@Observable` 状态流管理、macOS 专有平台框架（QuickLook, FinderSync, AVFoundation, Keychain, NSOpenPanel）及 UniFFI 强类型异步调用。
   - 所有数据密集型计算与文件 I/O 必须下沉至 Rust 内核。

3. **Strict Single-File LOC Threshold ($\le 800$ LOC)**:
   - 单文件行数硬性上限为 800 行，目标均值 $\le 350$ 行。超限即刻阻断 CI 提交流水线。

4. **Zero In-Tree Path Invariant (零源码树相对路径不变量)**:
   - 全语言 SDK 动态库加载器与头文件包含严禁假设当前运行在 Git 源码根目录下。
   - Java/Kotlin 必须基于 `NativeLoader` 自提取与 SHA-256 校验和缓存；Go 必须自包含 `include/ttzip.h` 与 CGO 规范；C/C++ 必须导出标准 CMake 目标 `ttzip::ttzip_cpp` / `ttzip::ttzip_c` 与 `ttzip.pc`。

5. **Distribution-Centric CI & Smoke Testing (分发驱动型 CI 门禁)**:
   - CI 不仅要验证源码编译（`cargo test`, `swift test`），必须在完全隔离的无源码空白临时目录中执行 `make test-out-of-tree-smoke`，验证预编译包分发与开箱即用性。

6. **Living & Executable Examples (活体可执行样例规范)**:
   - 每个语言生态必须在 `examples/<lang>/` 下维护独立可编译运行的 Quickstart 工程。
   - 所有样例工程在每次 CI commit 必须被编译和执行，严防文档样例退化（Bitrot）。

7. **Transparent Packaging Manifests & Type Stubs (显式打包契约与类型存根)**:
   - 所有打包清单（`pyproject.toml`, `CMakeLists.txt`, `ttzip.pc.in`, `pubspec.yaml`, `*.csproj`）必须声明完整的私有传递依赖（`libarchive`, `libbz2`, `libz`, `liblzma`）。
   - 动态语言必须导出完备类型存根（`py.typed`, `_ttzip.pyi`）与 `__version__ = "1.0.0"`。
