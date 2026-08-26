# Quickstart Validation Guide: 043-top-tier-open

**Feature Directory**: `specs/043-top-tier-open`  
**Status**: Completed  

---

## 1. Prerequisites & Environment
- **OS**: macOS 14.0+ (Sonoma)
- **Toolchain**: Swift 6.0 (`swift-tools-version: 6.0`), Clang with AddressSanitizer/ThreadSanitizer support.

---

## 2. Validation Scenarios

### Scenario 1: SPM 纯净编译与零 unsafeFlags 验证 (SPM Clean Build Validation)
- **Command**:
  ```bash
  swift build
  ```
- **Expected Output**:
  ```
  Building for debugging...
  [x/x] Compiling CTTZipBridge...
  [x/x] Compiling TTZipCore...
  Build complete!
  ```
- **Failure Diagnostic**:
  - *Failure*: 报 `The package product 'TTZipCore' cannot be used as a dependency ... because it uses unsafe build flags`.
  - *Troubleshooting*: 检查 `Package.swift` 中是否残留 `unsafeFlags` 或绝对路径，确保 `Vendor/TTZipVendor.xcframework` 已正确声明并使用相对路径 `cSettings`。

---

### Scenario 2: 内存安全 RAII 句柄与并发无竞争验证 (Mmap RAII & Concurrency Validation)
- **Command**:
  ```bash
  swift test --filter MmapBufferHandleTests
  ```
- **Expected Output**:
  ```
  Test Suite 'MmapBufferHandleTests' passed
  Executed X tests, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - *Failure*: 触发 `SIGBUS` 或 `EXC_BAD_ACCESS`.
  - *Troubleshooting*: 检查 `MmapBufferHandle` 的 `deinit` 是否在并发闭包完全退出前被提前释放，确认引用计数生命周期。

---

### Scenario 3: 全量 CI/CD 单元测试回归验证 (Full CI Unit Test Suite Validation)
- **Command**:
  ```bash
  swift test --parallel
  ```
- **Expected Output**:
  ```
  Test Suite 'All tests' passed
  Executed 95+ test suites, with 0 failures
  ```
- **Failure Diagnostic**:
  - *Failure*: 某个格式或设计模式测试套件报错。
  - *Troubleshooting*: 检查特定测试类日志，确认是否因桥接层改造导致 ABI 符号或路径查找漂移。

---

### Scenario 4: AddressSanitizer 内存安全动态检测 (ASan Validation)
- **Command**:
  ```bash
  swift test --sanitize=address --filter "(Engine|Bridge|Crypto|Security)"
  ```
- **Expected Output**:
  ```
  Test Suite 'Selected tests' passed under AddressSanitizer
  ```
- **Failure Diagnostic**:
  - *Failure*: 输出 `==ERROR: AddressSanitizer: heap-buffer-overflow` 或 `use-after-free`.
  - *Troubleshooting*: 根据 ASan 打印的崩溃调用栈定位 C/Swift 边界指针越界位置。

---

### Scenario 5: 性能硬门禁全绿验证 (Performance Gate Floor Validation)
- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
- **Expected Output**:
  ```
  Test Suite 'XCTestPerformanceMeasureTests' passed
  ✓ ZIP Level 1 / Level 6, 7Z, TAR.ZST throughput floor all satisfied
  ```
- **Failure Diagnostic**:
  - *Failure*: 吞吐跌破门禁基准线。
  - *Troubleshooting*: 检查 `ZipParallelExtractor` 与底层 C 调度热路径，确认无多余堆分配或锁争用。
