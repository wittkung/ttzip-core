# Implementation Plan: TTZip 全语言 SDK 零配置分发与外部开发者极致易用性体系 (Zero-Config SDK Distribution & Out-Of-Tree DX System)

- **Feature ID**: `007-zero-config-sdk-distribution-and-developer-experience`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `PLANNED`
- **Created**: 2026-08-24

---

## 1. 技术上下文与架构全景 (Technical Context)

### 架构目标
1. **消灭所有 In-Tree 相对路径假设**：
   - Java/Kotlin: 实现基于 SHA-256 校验缓存的自包含 `NativeLoader`，自动提取 `/META-INF/natives/{os}-{arch}/` 动态库。
   - Go: 实现内嵌 `include/ttzip.h` 与 `libs/<os>_<arch>/libttzip_engine.a`，解除相对目录穿越。
   - Python: 打包 ABI3 Limited API Wheels、PEP 561 `py.typed`、`_ttzip.pyi` 与 `__version__ = "1.0.0"`。
   - C/C++: 实现导出现代 CMake 命名空间目标 `ttzip::ttzip_cpp` / `ttzip::ttzip_c` 与 `Libs.private` 的 `ttzip.pc`。
   - Flutter & .NET: 规范化 `ffiPlugin` 与 `runtimes/<RID>/native/`。
2. **建立全语言独立可运行 Quickstart 示例套件** (`core/examples/`)。
3. **建立纯净容器 Out-Of-Tree 冒烟测试 CI 门禁** (`core/scripts/run_out_of_tree_smoke.sh` & `Makefile`)。

---

## 2. 任务分期与实施规划 (Phased Implementation Plan)

### Phase 1: Java 22+ & Kotlin 零配置 `NativeLoader` (T001 - T005)
- [ ] **T001**: 在 `core/sdk/jvm/src/main/java/com/ttzip/NativeLoader.java` 实现全平台识别、JAR 资源提取、SHA-256 内容寻址缓存与多级 Fallback。
- [ ] **T002**: 重构 `core/sdk/jvm/src/main/java/com/ttzip/TTZip.java` 接入 `NativeLoader.load()`，移除硬编码相对路径与静默异常捕获。
- [ ] **T003**: 在 `core/sdk/jvm/src/main/kotlin/com/ttzip/TTZipExtensions.kt` 实现 Coroutines / Flow 扩展。
- [ ] **T004**: 在 `core/sdk/jvm/src/test/java/com/ttzip/NativeLoaderTest.java` 编写加载器单元测试。
- [ ] **T005**: 验证 Java & Kotlin 在无 `-Dttzip.lib.path` 时的零配置测试通过。

### Phase 2: Python PEP 517/621 ABI3 Wheel 矩阵与类型注解导出 (T006 - T009)
- [ ] **T006**: 在 `core/pyproject.toml` 完善 Maturin 构建后端、元数据与 ABI3 包含规则。
- [ ] **T007**: 同步 `core/rust/ttzip-python/ttzip.pyi` 至 `core/python/ttzip/_ttzip.pyi`，添加 `core/python/ttzip/py.typed`。
- [ ] **T008**: 在 `core/python/ttzip/__init__.py` 和 `__init__.pyi` 导出 `__version__ = "1.0.0"`。
- [ ] **T009**: 验证 `maturin build --release` 产出的 wheel 包可直接在干净虚拟环境安装并跑通 Mypy。

### Phase 3: C++20 & C11 现代 CMake 目标与 pkg-config 导出 (T010 - T014)
- [ ] **T010**: 在 `core/CMakeLists.txt` 实现双目标拓扑 `ttzip::ttzip_cpp` 与 `ttzip::ttzip_c`，自动处理私有传递依赖。
- [ ] **T011**: 在 `core/cmake/ttzipConfig.cmake.in` 与 `core/cmake/FindTTZip.cmake` 完善依赖寻找与 Target 属性。
- [ ] **T012**: 在 `core/ttzip.pc.in` 与 `core/scripts/generate_pkg_config.sh` 完善 `Libs.private`。
- [ ] **T013**: 在 `core/examples/cpp/` 和 `core/examples/c/` 提供独立的 `CMakeLists.txt` 与 `main.cpp` / `main.c`。
- [ ] **T014**: 验证外部 CMake 工程 `find_package(ttzip REQUIRED)` 编译与运行。

### Phase 4: Go, Dart/Flutter & .NET 8 生态标准打包 (T015 - T019)
- [ ] **T015**: 在 `core/sdk/go/ttzip/` 建立自包含 `include/ttzip.h`，更新 `cgo_flags.go` 条件编译指令。
- [ ] **T016**: 在 `core/sdk/dart/` 规范化 `pubspec.yaml` 的 `ffiPlugin: true`，提供 `native_loader.dart` 与 Podspec/Gradle 配置文件。
- [ ] **T017**: 在 `core/sdk/dotnet/` 建立 `TTZip.csproj`，实现 `runtimes/<RID>/native/` 资源布局与 `NativeResolver.cs`。
- [ ] **T018**: 在 `core/examples/go/`、`core/examples/dart/`、`core/examples/dotnet/` 建立独立示例工程。
- [ ] **T019**: 验证各生态示例在独立目录直接运行。

### Phase 5: Out-Of-Tree 纯净容器冒烟测试 CI 门禁与宪章守护 (T020 - T024)
- [ ] **T020**: 在 `core/scripts/run_out_of_tree_smoke.sh` 实现无源码隔离环境端到端冒烟测试器。
- [ ] **T021**: 在 `core/Makefile` 增加 `test-out-of-tree-smoke` 目标。
- [ ] **T022**: 将 4 项分发治理宪章守则写入项目宪章与 Git 门禁。
- [ ] **T023**: 执行全量本地 CI 门禁与冒烟测试，验证 100% 通过。
- [ ] **T024**: 确保所有新增/修改文件遵循 $\le 800$ LOC 规范。
