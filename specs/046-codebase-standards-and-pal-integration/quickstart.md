# Quickstart & Validation Guide: 046-codebase-standards-and-pal-integration

本验证指南提供核心业务模块接入 PAL 的验证场景。

---

## 场景 1: SecurityScanner 跨平台恶意路径过滤校验

- **Command**:
  ```bash
  swift test --filter SecurityAndComplianceTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'SecurityAndComplianceTests' passed ...
  ```

---

## 场景 2: 密码库 v4 物理安全擦除与敏感内存销毁

- **Command**:
  ```bash
  swift test --filter PasswordVaultV4Tests
  ```
- **Expected Output**:
  ```text
  Test Suite 'PasswordVaultV4Tests' passed ...
  ```

---

## 场景 3: 硬件调度器与异构核心推演

- **Command**:
  ```bash
  swift test --filter AppleSiliconTunerTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'AppleSiliconTunerTests' passed ...
  ```

---

## 场景 4: 本地 CI 全自动化验证

- **Command**:
  ```bash
  ./scripts/run_local_ci.sh --quick
  ```
- **Expected Output**:
  ```text
  🏆 [ALL LOCAL CI GATES PASSED] Total Pipeline Duration: <= 15s
  ```
