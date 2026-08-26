# Quickstart & Validation Guide: 045-cross-platform-architecture-and-code-standards

本验证指南提供 4 大核心验证场景，用于校验跨平台抽象层 (PAL)、路径净化器与硬件特化分发器的正确性。

---

## 场景 1: 跨平台路径净化与 Windows 保留设备名拦截 (PlatformPathSanitizer)

- **Command**:
  ```bash
  swift test --filter PlatformPathSanitizerTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'PlatformPathSanitizerTests' passed at ...
  Executed 8 tests, with 0 failures (0 unexpected) in 0.010 seconds
  ```
- **Failure Diagnostic**:
  - 若测试失败，检查 `PlatformPathSanitizer.swift` 中对 Windows 保留名（`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`）的大写转换与剥离逻辑；
  - 检查反斜杠 `\` 与正斜杠 `/` 的双向正规化。

---

## 场景 2: 跨平台硬件探测与 SIMD 指令集掩码校验 (PlatformHardware)

- **Command**:
  ```bash
  swift test --filter PlatformHardwareTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'PlatformHardwareTests' passed at ...
  Executed 4 tests, with 0 failures (0 unexpected) in 0.005 seconds
  ```
- **Failure Diagnostic**:
  - 若在 Apple Silicon 机器上未识别出 `hasARMNeon = true` 或 `hasARMCrypto = true`，检查 `PlatformHardware.swift` 中的 `#if arch(arm64)` 分支。

---

## 场景 3: 跨平台虚拟内存映射 (PlatformMemory) 生命周期校验

- **Command**:
  ```bash
  swift test --filter PlatformMemoryTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'PlatformMemoryTests' passed at ...
  Executed 4 tests, with 0 failures (0 unexpected) in 0.006 seconds
  ```
- **Failure Diagnostic**:
  - 检查页对齐内存申请 `allocateAlignedPages` 与 `deallocateAlignedPages` 是否成对释放，且内存首地址为 4096 / 16384 字节对齐。

---

## 场景 4: 本地 CI/CD 全格式极速全回归与硬性能门禁验证

- **Command**:
  ```bash
  ./scripts/run_local_ci.sh --quick
  ```
- **Expected Output**:
  ```text
  ================================================================================
     🏆 [ALL LOCAL CI GATES PASSED] Total Pipeline Duration: <= 15s
  ================================================================================
  ```
- **Failure Diagnostic**:
  - 若 Stage 2 或 Stage 4 报错，查看 `docs/test_reports/local_ci_report.md` 中的错误上下文并修正。
