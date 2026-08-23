# Quickstart Validation Guide (Feature 016)

**Feature**: 100% Grand Slam Win Rate Across All 16 Archive Formats  
**Directory**: `specs/016-all-16-formats-100-percent-grand-slam/`

---

## Scenario 1: 全 16 格式竞品 1v1 PK 基准测试与 100% 胜率验证

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  - `Test Suite 'AllFormatsPkSuiteTests' passed.`
  - 自动生成 `docs/benchmarks/benchmark_report_<TIMESTAMP>.json`。
  - 统计断言: `Total PK comparisons: 280, Wins: 280, Losses: 0, Win Rate: 100.00%`。
- **Failure Diagnostic**:
  - 若出现负场（Loss），运行 `python3 scripts/audit_performance_regression.py` 查阅失利格式与场景，排查对应 C 桥接或 Swift 模板分发分支。

---

## Scenario 2: 零性能倒退审计验证

- **Command**:
  ```bash
  python3 scripts/audit_performance_regression.py
  ```
- **Expected Output**:
  - `[PASS] 审计通过: 无严重性能倒退场景。`
  - 倒退场景数（`regressedCount`）为 0 或核心场景倒退率严格 `< 3.0%`。
- **Failure Diagnostic**:
  - 若出现 `[FAIL]` 倒退告警，检查近期提交的代码改动，确保没有在并发热循环中引入锁操作、零填充内存分配或冗余系统调用。

---

## Scenario 3: 性能硬门禁全量测试

- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
- **Expected Output**:
  - 全部 11 大性能门禁（ZIP Level 1/6 压缩/解压、Store Direct I/O、7Z LZMA2/Level 1/AES KDF 等）全绿 PASS。
- **Failure Diagnostic**:
  - 若任一测试失败，检查 `Sources/TTZipCore/Zip/` 与 `Sources/CTTZipBridge/` 热路径是否遭到破坏。

---

## Scenario 4: 全量自动化回归套件

- **Command**:
  ```bash
  ./scripts/run_all_tests.sh
  ```
- **Expected Output**:
  - `Executed 560+ tests, with 0 failures.`
- **Failure Diagnostic**:
  - 若出现单测失败，定位具体测试用例并在对应模块修复。
