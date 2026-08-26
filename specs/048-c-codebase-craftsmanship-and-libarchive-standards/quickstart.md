# Quickstart & Validation Guide: 048-c-codebase-craftsmanship-and-libarchive-standards

本验证指南提供 C 代码质量、内存所有权与并发安全性的全套验证场景。

---

## 场景 1: C 桥接层无警告编译验证

- **Command**:
  ```bash
  swift build -c release
  ```
- **Expected Output**:
  ```text
  Build complete! (0 Errors, 0 Warnings)
  ```

---

## 场景 2: 7z/LZMA2 与 Tar 核心 C 引擎回归测试

- **Command**:
  ```bash
  swift test --filter "(TarNativeEngineTests|SevenZipBridgeTests|Libarchive7zEncryptionTests)"
  ```
- **Expected Output**:
  ```text
  Test Suite 'TarNativeEngineTests' passed ...
  Test Suite 'SevenZipBridgeTests' passed ...
  ```

---

## 场景 3: 本地 CI 8s 极速全回归

- **Command**:
  ```bash
  ./scripts/run_local_ci.sh --quick
  ```
- **Expected Output**:
  ```text
  🏆 [ALL LOCAL CI GATES PASSED] Total Pipeline Duration: <= 10s
  ```
