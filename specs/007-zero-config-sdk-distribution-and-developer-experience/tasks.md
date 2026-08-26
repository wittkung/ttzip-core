# Tasks: TTZip 全语言 SDK 零配置分发与外部开发者极致易用性体系 (Zero-Config SDK Distribution & Out-Of-Tree DX System)

- **Feature ID**: `007-zero-config-sdk-distribution-and-developer-experience`
- **Specification**: [`specs/007-zero-config-sdk-distribution-and-developer-experience/spec.md`](file:///Users/kevintung/Documents/dev/products/ttzip/specs/007-zero-config-sdk-distribution-and-developer-experience/spec.md)
- **Implementation Plan**: [`specs/007-zero-config-sdk-distribution-and-developer-experience/plan.md`](file:///Users/kevintung/Documents/dev/products/ttzip/specs/007-zero-config-sdk-distribution-and-developer-experience/plan.md)
- **Status**: `COMPLETED`

---

## Phase 1: Setup & Foundational Infrastructure

- [x] T001 Verify contract schemas in `specs/007-zero-config-sdk-distribution-and-developer-experience/contracts/native-loader-contract.json` and `specs/007-zero-config-sdk-distribution-and-developer-experience/contracts/out-of-tree-smoke-contract.json`
- [x] T002 [P] Establish standalone examples directory layout `core/examples/{cpp,c,python,jvm,kotlin,go,dart,dotnet}`

---

## Phase 2: User Story 1 (P1) - Java 22+ & Kotlin 零配置 NativeLoader

**Goal**: 实现自包含 `NativeLoader`，自动提取 `/META-INF/natives/{os}-{arch}/` 动态库并通过 SHA-256 校验和缓存秒级复用，彻底废除强制配置 `-Dttzip.lib.path` 的要求。

- [x] T003 [P] [US1] Implement OS/Arch classifier and SHA-256 caching in `core/sdk/jvm/src/main/java/com/ttzip/NativeLoader.java`
- [x] T004 [US1] Refactor `core/sdk/jvm/src/main/java/com/ttzip/TTZip.java` to bind via `NativeLoader.load()` and remove hardcoded relative dev paths and swallowed link errors
- [x] T005 [P] [US1] Update Kotlin Coroutines and Flow extensions in `core/sdk/jvm/src/main/kotlin/com/ttzip/TTZipExtensions.kt`
- [x] T006 [P] [US1] Implement unit test suite in `core/sdk/jvm/src/test/java/com/ttzip/NativeLoaderTest.java`
- [x] T007 [US1] Implement standalone runnable quickstart in `core/examples/jvm/Quickstart.java` and `core/examples/kotlin/Quickstart.kt`

---

## Phase 3: User Story 2 (P1) - Python PEP 517/621 ABI3 Wheel 矩阵与类型导出

**Goal**: 完善 `maturin` + PyO3 `abi3-py310` 预编译 Wheel 方案，并补全 PEP 561 `py.typed`、`_ttzip.pyi` 与 `__version__ = "1.0.0"`。

- [x] T008 [P] [US2] Update `core/pyproject.toml` with PEP 517/621 metadata, `maturin` build backend, and ABI3 package inclusion rules
- [x] T009 [P] [US2] Export `__version__ = "1.0.0"` in `core/python/ttzip/__init__.py` and `core/python/ttzip/__init__.pyi`
- [x] T010 [P] [US2] Add PEP 561 marker `core/python/ttzip/py.typed` and sync C-extension stub to `core/python/ttzip/_ttzip.pyi`
- [x] T011 [US2] Implement standalone runnable quickstart in `core/examples/python/quickstart.py`

---

## Phase 4: User Story 3 (P1) - C++20 & C11 现代 CMake 目标与 pkg-config 导出

**Goal**: 生成现代 CMake 目标 `ttzip::ttzip_cpp`（C++20 RAII）与 `ttzip::ttzip_c`（C11），自动携带 `libarchive`、`libbz2` 等全部私有传递依赖。

- [x] T012 [P] [US3] Implement top-level unified `core/CMakeLists.txt` with targets `ttzip::ttzip_cpp` and `ttzip::ttzip_c`
- [x] T013 [P] [US3] Implement CMake config file template `core/cmake/ttzipConfig.cmake.in` and `core/cmake/FindTTZip.cmake`
- [x] T014 [P] [US3] Update `core/ttzip.pc.in` and `core/scripts/generate_pkg_config.sh` with complete `Libs.private` definitions
- [x] T015 [US3] Implement standalone runnable CMake quickstart in `core/examples/cpp/CMakeLists.txt` + `core/examples/cpp/main.cpp` and `core/examples/c/CMakeLists.txt` + `core/examples/c/main.c`

---

## Phase 5: User Story 4 (P1) - Go, Dart/Flutter & .NET 8 生态标准打包

**Goal**: 分别适配 CGO 静态库内嵌、Flutter 官方 `ffiPlugin: true` 与 NuGet `runtimes/<RID>/native/` 规范。

- [x] T016 [P] [US4] Create self-contained `core/sdk/go/ttzip/include/ttzip.h` and configure `core/sdk/go/ttzip/cgo_flags.go` to eliminate path traversal
- [x] T017 [P] [US4] Update `core/sdk/dart/pubspec.yaml` with `ffiPlugin: true` and configure `core/sdk/dart/lib/src/native_loader.dart`
- [x] T018 [P] [US4] Create `core/sdk/dotnet/src/TTZip/TTZip.csproj` with `runtimes/<RID>/native/` packaging and `core/sdk/dotnet/src/TTZip/Native/NativeResolver.cs`
- [x] T019 [US4] Implement standalone runnable quickstarts in `core/examples/go/quickstart.go`, `core/examples/dart/quickstart.dart`, and `core/examples/dotnet/Program.cs`

---

## Phase 6: User Story 5 (P1) - Out-Of-Tree 纯净容器冒烟测试 CI 门禁与宪章守护

**Goal**: 在完全脱离 Git 仓库的独立空白临时目录中，仅通过构建出的 Distribution Artifact 安装并运行全语言 `quickstart` 样例。

- [x] T020 [US5] Implement master smoke orchestrator in `core/scripts/run_out_of_tree_smoke.sh` executing quickstarts in isolated temporary directory
- [x] T021 [US5] Add `test-out-of-tree-smoke` target to `core/Makefile`
- [x] T022 [US5] Amend project constitution rules in `core/memory/constitution.md` with Zero In-Tree Path Invariant and Living Examples Specification
- [x] T023 [US5] Execute full test suite `make -C core test-all-sdk` and `make -C core test-out-of-tree-smoke`
- [x] T024 [US5] Validate that all modified and created source files conform to $\le 800$ LOC threshold
