# Research & Technical Decisions: TTZip 全语言 SDK 零配置分发与外部开发者极致易用性体系

- **Feature ID**: `007-zero-config-sdk-distribution-and-developer-experience`
- **Date**: 2026-08-24
- **Author**: TTZip Core Architecture Team

---

## 1. Java 22+ Panama FFM 动态库分发与零配置自提取机制

- **Decision**: 采用类似 RocksDB / Netty 的 `NativeLoader` 内嵌架构。在 Java 构建时，将各平台的 `libttzip_engine.dylib`、`libttzip_engine.so`、`ttzip_engine.dll` 打包入 JAR 包的 `/META-INF/natives/{os}-{arch}/` 路径。在运行时首次调用时，通过 SHA-256 校验和自动提取至 `~/.ttzip/natives/{version}/` 并通过 `SymbolLookup.libraryLookup(extractedPath, Arena.global())` 绑定。
- **Rationale**: 
  - 传统 JNI 或 Panama FFM 默认依赖 `java.library.path` 或系统路径，对于 Maven/Gradle 终端使用者极其不友好。
  - 基于 SHA-256 的内容寻址缓存能避免每次 JVM 重启重复写磁盘，初次解压仅需 ~2ms，后续常驻预热复用 $< 0.1\text{ms}$。
- **Alternatives Considered**:
  - *要求用户手动传递 `-Dttzip.lib.path`*: 开发者体验极差，Spring Boot / 微服务容器每次启动都要额外配启动参数。
  - *JNA 默认提取器*: 引入繁重的第三方 JNA 依赖，违背零外部依赖设计。

---

## 2. Python PyPI 零编译分发与 ABI3 Limited API 矩阵

- **Decision**: 采用 `maturin` + PyO3 `abi3-py310` 架构，结合 `cibuildwheel` 自动化产出覆盖 5 大平台架构的预编译 Binary Wheels。同时完整导出 `__version__ = "1.0.0"`，打包 PEP 561 `py.typed` 与 `_ttzip.pyi` 类型存根。
- **Rationale**: 
  - `abi3-py310` 允许一套 Wheel 跨 Python 3.10, 3.11, 3.12, 3.13, 3.14 运行，免去为每个 Python 次版本重复构建分发。
  - 外部开发者执行 `pip install ttzip` 可在 2 秒内下载预编译二进制安装，无需在本地配置 Rust/Cargo 环境。
- **Alternatives Considered**:
  - *仅发布 Source Distribution (sdist)*: 用户在 `pip install` 时必须本地编译 Rust 源码，耗时数分钟且依赖 C 编译器和 Rust 工具链。

---

## 3. C++20 & C11 现代 CMake 目标拓扑与 pkg-config 导出

- **Decision**: 在根目录与 `core/` 建立标准 `CMakeLists.txt`，导出命名空间目标 `ttzip::ttzip_cpp`（C++20 RAII，依赖 `ttzip::ttzip_c`）与 `ttzip::ttzip_c`（C11，包含私有依赖）。生成完整的 `ttzipConfig.cmake` 与包含 `Libs.private: -larchive -lbz2 -lz -llzma -lpthread -framework Security -framework CoreFoundation` 的 `ttzip.pc`。
- **Rationale**: 
  - 外部 CMake 项目可以直接使用 `find_package(ttzip REQUIRED)` 并链接 `target_link_libraries(app PRIVATE ttzip::ttzip_cpp)`，CMake 会自动传递编译标准（`cxx_std_20`）、头文件路径和所有底层私有链接库，无需用户手动寻找 `-larchive` 等 6 个依赖。
- **Alternatives Considered**:
  - *仅提供 `libttzip_engine.a` 静态库文件*: 导致外部开发者必须手动排查未解析符号并自行拼接 6 个链接参数。

---

## 4. Go, Flutter 与 .NET 8 生态标准打包

- **Decision**:
  - **Go**: 采用 `#cgo` 条件编译标签，分发独立的 `include/ttzip.h` 和 `libs/<os>_<arch>/libttzip_engine.a`，杜绝目录向上穿越。
  - **Flutter**: 规范化 `pubspec.yaml` 的 `ffiPlugin: true`，提供完整的 macOS/iOS Podspec 和 Android Gradle `jniLibs` 挂载。
  - **.NET 8**: 采用 NuGet 标准 `runtimes/<RID>/native/` 布局，配合 C# 12 `[ModuleInitializer]` 与 `NativeLibrary.SetDllImportResolver`。
- **Rationale**: 符合各语言生态最地道（Idiomatic）的包管理规范，开发者无需额外配置即可开箱即用。

---

## 5. 纯净容器隔离冒烟测试门禁 (Out-Of-Tree Smoke Test Gate)

- **Decision**: 在本地 CI 中新增 `make test-out-of-tree-smoke` / `core/scripts/run_out_of_tree_smoke.sh`。测试脚本在一个完全隔离的临时目录（无 Git 仓库、无 Rust 源码）中，仅通过已构建的 wheel / jar / cmake install 产物安装依赖，并编译运行各语言的 `quickstart` 示例。
- **Rationale**: 阻断任何对 In-Tree 源码树相对路径的假定，从根本上杜绝“在我机器/仓库里是好的，到外部开发者手里就崩”的系统性缺陷。
