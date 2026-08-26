# Feature Specification: TTZip 全语言 SDK 零配置分发与外部开发者极致易用性体系 (Zero-Config SDK Distribution & Out-Of-Tree DX System)

- **Feature ID**: `007-zero-config-sdk-distribution-and-developer-experience`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `SPECIFIED`
- **Created**: 2026-08-24
- **Target Subsystems & Packages in Scope**:
  - `core/sdk/jvm/` & `core/sdk/jvm/src/main/java/com/ttzip/NativeLoader.java` (Java 22+ Panama FFM & Kotlin Coroutines Zero-Config Native Loader)
  - `core/pyproject.toml` & `core/python/` (Python PyPI PEP 517/621 Maturin Zero-Compile ABI3 Wheel Matrix)
  - `core/CMakeLists.txt`, `core/cmake/`, `core/ttzip.pc.in` (C++20 & C11 Modern Target Topology `ttzip::ttzip_cpp` / `ttzip::ttzip_c`)
  - `core/sdk/go/` (Go CGO Standalone Static Archive Embedding & Header Distribution)
  - `core/sdk/dart/` (Dart / Flutter Official FFI Plugin `ffiPlugin: true` & Native Assets Platform Manifests)
  - `core/sdk/dotnet/` (C# .NET 8 Runtime Identifier `runtimes/<RID>/native/` NuGet Packaging & `NativeResolver`)
  - `core/examples/` (10-Second Standalone Runnable Quickstarts per Ecosystem)
  - `core/scripts/` & CI (Out-Of-Tree Clean Container Smoke Test Gate)

---

## 1. 业务背景与问题定义 (Problem Statement & Motivation)

TTZip 的底层微内核在吞吐量（4.8 GB/s）、内存安全（7MB 常驻 RSS）以及硬件 SIMD 加速层面已达到工业级极致标准。

然而，在面对**完全独立于代码仓库的外部开发者（Out-Of-Tree Consumers）**时，各语言 SDK 的分发与使用存在显著的“末梢接入摩擦”：
1. **Java/Kotlin 动态库路径摩擦**：`TTZip.java` 依赖 `-Dttzip.lib.path` 或源码树相对路径，外部 Maven/Gradle 依赖引入后首次运行必崩。
2. **Python 缺失零编译预编译 Wheel 体系**：无预编译 ABI3 Wheels，外部用户必须在本地安装 Rust/Cargo 才能安装使用，且缺失 PEP 561 标记和 `__version__`。
3. **C/C++ 缺乏现代 CMake 目标与私有依赖传递**：外部项目链接 `libttzip_engine.a` 时因缺失 `libarchive`、`libbz2` 等 6 个私有依赖而报大量未解析符号。
4. **Go CGO 路径向上穿越**：`cgo_flags` 中硬编码相对路径，在 Go Module 缓存路径下编译必然失败。
5. **Dart/Flutter 与 .NET 缺失原生资源标准打包规范**：未接入 Flutter FFI Plugin 与 .NET RID 规范。
6. **缺乏真实纯净容器冒烟测试 (Out-Of-Tree Smoke Tests)**：CI 仅验证源码编译，未在无源码的隔离环境中测试包安装与示例执行。

---

## 2. 用户故事与核心用例 (User Stories & Scenarios)

### User Story 1 (P1): Java 22+ & Kotlin 零配置即开即用 (Zero-Config JVM Native Loader)
> **作为** Java / Kotlin 后端开发者，
> **我希望** 在 `pom.xml` 或 `build.gradle.kts` 中添加 `ttzip-jvm` 依赖后，无需任何 JVM 参数 (`-Dttzip.lib.path`) 或手动拷贝 `.dylib`/`.so`，直接在代码中调用 `TTZip.compress(...)` 或 `file.ttzipCompressFlow(...)`，
> **以便于** 在 5 秒内完成生产集成，零环境配置摩擦。

- **Scenario 1.1 (内嵌 JAR 自动提取与缓存)**: 首次调用时，`NativeLoader` 自动识别当前 OS/Arch，从 JAR 资源的 `META-INF/natives/{os}-{arch}/` 提取动态库至 `~/.ttzip/natives/`，并通过 SHA-256 校验和秒级复用，无二次解压开销。
- **Scenario 1.2 (多级优雅降级与清晰诊断)**: 若系统环境变量或属性显式指定了路径，优先加载；若所有层级均失败，输出包含平台信息与检索路径清单的清晰诊断异常，严禁静默吞没错误。

### User Story 2 (P1): Python 零编译秒装与标准库无缝平替 (Zero-Compile PyPI Wheels & Drop-in stdlib)
> **作为** Python 数据工程师或后端开发者，
> **我希望** 通过 `pip install ttzip` 在 3 秒内安装预编译二进制 Wheel，
> **以便于** 在无需本地 Rust 编译环境的情况下，使用包含完整类型提示的 `ttzip` 及 `from ttzip import zipfile` 标准库平替。

- **Scenario 2.1 (PEP 517/621 ABI3 Wheel 矩阵)**: 提供兼容 Python 3.10+ 的单一套件 Wheel (`cp310-abi3`)，覆盖 Apple Silicon, Intel Mac, Linux x86_64, Linux ARM64, Windows x64。
- **Scenario 2.2 (IDE 智能补全与契约完备)**: 导出 `__version__ = "1.0.0"` 并打包 `_ttzip.pyi` 和 `py.typed`，保证 VS Code / PyCharm 语法高亮与类型提示 100% 正确。

### User Story 3 (P1): C++20 & C11 现代 CMake 与 pkg-config 导出 (Modern C/C++ Package Export)
> **作为** C/C++ 客户端或系统开发者，
> **我希望** 通过 `find_package(ttzip REQUIRED)` 或 `pkg-config --libs --static ttzip` 一键引入 TTZip，
> **以便于** 自动配置 C++20/C11 编译标准、头文件路径以及全部私有传递依赖库，彻底杜绝链接错误。

- **Scenario 3.1 (现代 CMake 目标拓扑)**: 导出 `ttzip::ttzip_cpp`（C++20 RAII 接口）与 `ttzip::ttzip_c`（C11 原生接口），自动处理 `Threads::Threads`、`libarchive`、`libbz2`、`libz`、`liblzma` 及 macOS `Security.framework` 的私有链接。
- **Scenario 3.2 (pkg-config 完整静态描述)**: 生成的 `ttzip.pc` 中完整声明 `Libs.private`，确保传统 Makefile / Meson 构建系统通过 `pkg-config --libs --static ttzip` 即可无损编译。

### User Story 4 (P1): Go, Flutter 与 .NET 8 标准化生态打包 (Ecosystem Packaging)
> **作为** Go、Flutter 或 C# 开发者，
> **我希望** 按照各自主流包管理器的原生标准消费 TTZip，
> **以便于** 杜绝路径穿越、平台编译不兼容或缺少原生动态库的缺陷。

- **Scenario 4.1 (Go 静态归档内嵌)**: Go SDK 内嵌 `include/ttzip.h` 与 `libs/<os>_<arch>/libttzip_engine.a`，配合条件编译标签实现纯 `go get` / `go build` 零外部依赖。
- **Scenario 4.2 (Flutter FFI 官方插件)**: 配置 `pubspec.yaml` 的 `ffiPlugin: true`，提供完整的 macOS/iOS Podspec 与 Android `jniLibs` 支持。
- **Scenario 4.3 (.NET 8 RID NuGet)**: 遵循 NuGet `runtimes/<RID>/native/` 规范，配合 `NativeLibrary.SetDllImportResolver` 实现跨平台原生无缝加载。

### User Story 5 (P1): 纯净容器隔离冒烟测试门禁 (Out-Of-Tree Clean Container Smoke Gate)
> **作为** TTZip 架构师与 CI/CD 守护者，
> **我希望** CI 在每次发布前在无源码树的空目录容器中，仅通过已打包的 distribution artifact 执行端到端 Quickstart 样例，
> **以便于** 100% 阻断任何假设 In-Tree 源码树路径的回归缺陷。

---

## 3. 验收标准 (Acceptance Criteria)

1. **Java/Kotlin**: 在清空 `TTZIP_LIBRARY_PATH` 且无 `-Dttzip.lib.path` 的干净 JVM 进程中，`TTZip.compress` / `TTZip.extract` 100% 成功执行。
2. **Python**: `python -m build` 或 `maturin build` 产出的 wheel 包在干净虚拟环境中通过 `pip install` 安装后，`quickstart.py` 运行退出码为 0，`mypy` 类型检查通过。
3. **C/C++**: 通过 CMake `find_package(ttzip REQUIRED)` 构建的 C++20 / C11 独立示例工程编译运行通过，无未解析符号。
4. **Go**: 在独立的空白目录中运行 `go run quickstart.go`，无需指向上级代码目录即可直接构建并运行。
5. **CI 门禁**: 新增 `test-out-of-tree-smoke` 测试目标，在纯净临时目录中验证全语言 Quickstart 样例。
