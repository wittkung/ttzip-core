# Quickstart & Validation Guide: 047-libarchive-elegance-and-decoupling

本验证指南提供正交解耦与注释规范的自动化验证场景。

---

## 场景 1: 正交容器与流式滤镜组合验证

- **Command**:
  ```bash
  swift test --filter ArchiveOrthogonalPipelineTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'ArchiveOrthogonalPipelineTests' passed ...
  ```

---

## 场景 2: 状态机与 6 级错误恢复验证

- **Command**:
  ```bash
  swift test --filter TTZipStatusAndRecoveryTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'TTZipStatusAndRecoveryTests' passed ...
  ```

---

## 场景 3: 本地 CI 全自动化全矩阵回归

- **Command**:
  ```bash
  ./scripts/run_local_ci.sh --quick
  ```
- **Expected Output**:
  ```text
  🏆 [ALL LOCAL CI GATES PASSED] Total Pipeline Duration: <= 12s
  ```
