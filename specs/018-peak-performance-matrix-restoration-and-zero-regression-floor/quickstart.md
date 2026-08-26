# Quickstart & Verification Guide (Feature 018)

**Feature**: Restoration Against Historical Peak Matrix & Hard 10% Floor Invariant  
**Directory**: `specs/018-peak-performance-matrix-restoration-and-zero-regression-floor/`

---

## 1. 场景一：全格式对比历史最高峰值矩阵零倒退审计

### Command
```bash
python3 scripts/audit_performance_regression.py docs/benchmarks/peak_performance_matrix.json $(ls -t docs/benchmarks/benchmark_report_*.json | head -n 1)
```

### Expected Output
- 控制台输出峰值比对结论。
- `【🔴 严重倒退阻断 (< -10.0%)】` 项数为 **0**。
- 退出码为 `0`。

---

## 2. 场景二：Release 11 大性能硬门禁全量校验

### Command
```bash
swift test -c release --filter XCTestPerformanceMeasureTests
```

### Expected Output
- `Executed 11 tests, with 0 failures (0 unexpected)`.

---

## 3. 场景三：全量 591+ 单元测试回归验证

### Command
```bash
swift test
```

### Expected Output
- `Executed 591 tests, with 8 tests skipped and 0 failures (0 unexpected)`.
