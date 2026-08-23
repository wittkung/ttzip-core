# Quickstart Validation Guide: 022-full-matrix-zero-regression-and-throughput-closure

## 验证场景 1: TAR.ZST Direct 50MB 打包 $\ge 19,000\text{ MB/s}$ 门禁验证

### Command
```bash
swift test -c release --filter XCTestPerformanceMeasureTests/testTarZstdDirect_50MB_ThroughputFloor
```

### Expected Output
- 测试通过（`PASS`）。
- 吞吐测算结果 `throughputMBs >= 19000.0`（实测预计 25,000 ~ 35,000 MB/s）。

### Failure Diagnostic
- 若吞吐 < 19,000 MB/s，检查 `ttzip_tar_zstd_direct.c` 中 `s_tar_zstd_cctx` 静态复用是否生效，检查 `adaptive_job_sz` 是否被错误覆盖。

---

## 验证场景 2: 11 项 Release 性能硬门禁 100% 绿灯验证

### Command
```bash
swift test -c release --filter XCTestPerformanceMeasureTests
```

### Expected Output
- 11 个测试用例全部执行通过，0 失败。
- 包括 ZIP Store Direct I/O、ZIP L1/L6、7Z L1/L5、7Z 解压、7Z KDF、TAR.ZST Direct 50MB 等全部高于门禁底线。

### Failure Diagnostic
- 逐项检查失败项的耗时日志，确认各格式 Fast-Path 是否正常触发。

---

## 验证场景 3: 全格式 46 项基准测试 28 项倒退清零验证

### Command
```bash
TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json
```

### Expected Output
- 终端输出 `[AUDIT PASSED] 0-Regression Verified!`。
- 严重倒退项（`delta < -10.0%`）数量为 **0**。

### Failure Diagnostic
- 查阅 `docs/benchmarks/latest_regression_audit.md` 定位仍有倒退的格式与场景，比对 C 桥接层 I/O 缓冲区与并发分块参数。
