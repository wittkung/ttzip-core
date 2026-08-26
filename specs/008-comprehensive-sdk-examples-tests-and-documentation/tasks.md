# Tasks: TTZip 全格式与高级设置深度接入示例、全套 SDK 测试用例与完整接入文档体系 (Feature 008)

- **Feature ID**: `008-comprehensive-sdk-examples-tests-and-documentation`
- **Specification**: [`specs/008-comprehensive-sdk-examples-tests-and-documentation/spec.md`](file:///Users/kevintung/Documents/dev/products/ttzip/specs/008-comprehensive-sdk-examples-tests-and-documentation/spec.md)
- **Implementation Plan**: [`specs/008-comprehensive-sdk-examples-tests-and-documentation/plan.md`](file:///Users/kevintung/Documents/dev/products/ttzip/specs/008-comprehensive-sdk-examples-tests-and-documentation/plan.md)
- **Status**: `COMPLETED`

---

## Phase 1: 体系化 SDK 开发者文档与高级设置秘籍 (T001 - T005)

- [x] T001 [P] [US3] Write master documentation in `core/docs/sdk/README.md` with full 16-format compatibility matrix, architecture principles, and language navigation
- [x] T002 [P] [US3] Write dedicated SDK guides in `core/docs/sdk/RUST_GUIDE.md`, `core/docs/sdk/SWIFT_GUIDE.md`, and `core/docs/sdk/PYTHON_GUIDE.md`
- [x] T003 [P] [US3] Write dedicated SDK guides in `core/docs/sdk/JVM_KOTLIN_GUIDE.md`, `core/docs/sdk/CPP_C_GUIDE.md`, and `core/docs/sdk/GO_GUIDE.md`
- [x] T004 [P] [US3] Write dedicated SDK guides in `core/docs/sdk/DART_FLUTTER_GUIDE.md`, `core/docs/sdk/DOTNET_GUIDE.md`, and `core/docs/sdk/NODE_TYPESCRIPT_GUIDE.md`
- [x] T005 [P] [US3] Write multi-language recipe comparison in `core/docs/sdk/ADVANCED_SETTINGS_RECIPES.md` (Encryption, Reed-Solomon, VFS, Progress, Cancellation)

---

## Phase 2: 10 大语言全格式与高级设置可运行示例工程 (`examples/`) (T006 - T015)

- [x] T006 [P] [US1] Implement comprehensive multi-format & advanced settings example in `core/examples/rust/`
- [x] T007 [P] [US1] Implement Swift 6 Actor & AsyncStream advanced settings example in `core/examples/swift/`
- [x] T008 [P] [US1] Expand Python 16-format & advanced settings example in `core/examples/python/advanced_example.py`
- [x] T009 [P] [US1] Expand Java 22+ Panama FFM & Kotlin Flow advanced settings example in `core/examples/jvm/AdvancedExample.java` & `core/examples/kotlin/AdvancedExample.kt`
- [x] T010 [P] [US1] Expand C++20 `std::span` zero-copy & RAII advanced settings example in `core/examples/cpp/advanced_example.cpp`
- [x] T011 [P] [US1] Expand C11 native options structure & multi-format example in `core/examples/c/advanced_example.c`
- [x] T012 [P] [US1] Expand Go `io/fs.FS` & `context.Context` cancellation advanced settings example in `core/examples/go/advanced_example.go`
- [x] T013 [P] [US1] Expand Dart/Flutter background `Isolate` & `Stream<ArchiveProgress>` example in `core/examples/dart/advanced_example.dart`
- [x] T014 [P] [US1] Expand C# .NET 8 `ReadOnlySpan` & `IAsyncEnumerable` example in `core/examples/dotnet/AdvancedExample.cs`
- [x] T015 [P] [US1] Expand Node.js/TypeScript Promise & Stream example in `core/examples/node/advanced_example.ts`

---

## Phase 3: 全语言 SDK 原生测试套件 16 格式与高级设置扩充 (T016 - T021)

- [x] T016 [P] [US2] Expand Java 22+ JUnit 5 test suite in `core/sdk/jvm/src/test/java/com/ttzip/TTZipTest.java` with 16-format & option matrix assertions
- [x] T017 [P] [US2] Expand Go test suite in `core/sdk/go/ttzip/ttzip_test.go` with multi-format & context cancellation assertions
- [x] T018 [P] [US2] Expand C++20 & C11 test suites in `core/sdk/cpp/test_cpp_sdk.cpp` and `core/sdk/c/test_c_sdk.c`
- [x] T019 [P] [US2] Expand C# .NET 8 test suite in `core/sdk/dotnet/TTZipTest.cs` with password and async stream assertions
- [x] T020 [P] [US2] Expand Dart test suite in `core/sdk/dart/test/ttzip_test.dart`
- [x] T021 [P] [US2] Expand Python test matrix in `core/python/tests/test_all_16_formats.py` with extreme compression levels and error boundaries

---

## Phase 4: 本地 CI 与冒烟测试全量验证 (T022 - T025)

- [x] T022 [US2] Execute `make -C core test-all-sdk` verifying all 9 SDK native test suites pass
- [x] T023 [US1] Execute `make -C core test-out-of-tree-smoke` verifying out-of-tree examples pass
- [x] T024 Validate that all created/modified files conform to single-file $\le 800$ LOC defense gate
- [x] T025 Commit and push changes to `origin/main`
