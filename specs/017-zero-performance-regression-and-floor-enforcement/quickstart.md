# Quickstart & Verification Guide (Feature 017)

**Feature**: Zero Performance Regression Governance & Hard Floor Invariant Enforcement  
**Directory**: `specs/017-zero-performance-regression-and-floor-enforcement/`

---

## 1. 场景一：全格式零性能倒退双级门禁审查

### Command
```bash
python3 scripts/audit_performance_regression.py
```

### Expected Output
- 控制台输出审计摘要。
- `【🔴 严重倒退告警 (<-10.0%)】` 项数为 **0**。
- 进程退出码为 `0` (`echo $?` -> `0`)。
- 自动生成 `docs/benchmarks/latest_regression_audit.md`。

### Failure Diagnostic
若退出码非零或发现 $>10.0\%$ 倒退项：
1. 检查 `docs/benchmarks/latest_regression_audit.md` 中的 `## 🔴 严重性能倒退阻断列表`。
2. 核对变动格式的 C 桥接层与参数映射（如 `lzip` 级别参数、`ttzip_extract_tar_native_c` 目录解包逻辑）。
3. 重新运行单项诊断测试定位瓶颈。

---

## 2. 场景二：Release 11 大性能硬门禁全量校验

### Command
```bash
swift test -c release --filter XCTestPerformanceMeasureTests
```

### Expected Output
- `Executed 11 tests, with 0 failures (0 unexpected)`.
- 11 项吞吐/耗时底线全部达成（TAR.ZST Direct 50MB $\ge$ 15000 MB/s, ZIP Decompression $\ge$ 9000 MB/s, 7Z Decompression $\ge$ 7500 MB/s, ZIP Single 50MB $\ge$ 1600 MB/s, 7Z KDF $\le$ 14 ms 等）。

### Failure Diagnostic
若任一门禁失败：
1. 检查是否在 Debug 模式运行（Release 模式要求更高吞吐）。
2. 检查是否有后台重载进程占用 CPU，重启测试以排除干扰。
3. 检查对应格式的热路径代码是否违反零堆分配约束。

---

## 3. 场景三：全量 591+ 单元测试回归验证

### Command
```bash
swift test
```

### Expected Output
- `Executed 591 tests, with 8 tests skipped and 0 failures (0 unexpected)`.
- 所有测试套件（NativeBrotliEngineTests, AllFormatDiagnosticSuiteTests, PerformanceRegressionGuardTests 等）100% 绿灯通过。

### Failure Diagnostic
若测试失败：
1. 查看失败用例的断言报错与调用栈。
2. 检查文件路径解引用是否使用了 `CUnsafeBufferAdapter`。
3. 确保临时文件与目录在 `defer` 中正确释放。
