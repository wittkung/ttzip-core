# Quickstart & Verification Guide (Feature 019)

**Feature**: Historical Peak Gap Closure & Unified Fast-Path Alignment  
**Directory**: `specs/019-historical-peak-gap-closure-and-unified-fast-path/`

---

## 1. 场景一：全格式跑分与历史峰值差距验证

### Command
```bash
TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
```

### Expected Output
- `Executed 1 test, with 0 failures (0 unexpected)`.
- 16 种格式基准报告输出至 `docs/benchmarks/`.

---

## 2. 场景二：双级零倒退门禁全量校验

### Command
```bash
python3 scripts/audit_performance_regression.py docs/benchmarks/peak_performance_matrix.json $(ls -t docs/benchmarks/benchmark_report_*.json | head -n 1)
```

### Expected Output
- 严重倒退项（$> 10.0\%$）数大幅收敛至 0。
- 退出码为 `0`。

---

## 3. 场景三：单元测试与门禁通过

### Command
```bash
swift test -c release --filter XCTestPerformanceMeasureTests && swift test
```

### Expected Output
- 11/11 性能门禁通过，591+ 单元测试 100% 绿灯。
