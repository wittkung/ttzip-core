# Quickstart & Verification Guide: Test Suite Architecture Governance, Target Isolation & Unified Corpus Infrastructure

**Feature Directory**: `specs/135-test-suite-governance-and-target-isolation`  
**Status**: Approved  

---

## 1. 验证场景一：50 点内存矩阵与微秒级延迟输出

### 命令 (Command)
```bash
swift test --filter TTZipCoreCodecBenchmarkTests/test50PointInMemeoryMatrixExecution
```

### 预期输出 (Expected Output)
```text
==========================================================================================================================
⚡️ TTZip Deflate-Bench Unified In-Memory Matrix (Total Points: 50)
==========================================================================================================================
[Idx] Engine     | Corpus        | Size  | Lvl | Comp Time  | Comp Rate   | Decomp Time| Decomp Rate | Ratio  | Status
--------------------------------------------------------------------------------------------------------------------------
...
Summary: 50/50 Points PASSED | Total Matrix Time: 1.070s | Median CV: 0.95%
==========================================================================================================================
```

### 故障排查 (Failure Diagnostic)
- 若某点输出 FAIL：检查 `memcmp(pool.inputBuffer, pool.decompressedBuffer, inSize)` 是否返回非零，排查该压缩引擎对应 Level 的边界条件。
- 若总耗时 $> 2.0	ext{ s}$：检查是否有后台进程抢占 CPU，或单点多轮测量未命中 L1/L2 缓存。

---

## 2. 验证场景二：全量单测极速秒级通过

### 命令 (Command)
```bash
swift test
```

### 预期输出 (Expected Output)
```text
Executed 400+ tests, with 0 failures (0 unexpected) in < 3.0 seconds
```

### 故障排查 (Failure Diagnostic)
- 若遇到路径找不到：检查 `SystemBinaryResolver` 是否正确解析系统 `unzip` / `tar` / `zstd`。
- 若遇到性能回退错误：检查是否设置了 `TTZIP_RUN_BENCHMARKS=1` 触发了非隔离的完整宏观压测。
