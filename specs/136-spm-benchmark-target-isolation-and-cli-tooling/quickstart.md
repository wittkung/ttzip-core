# Quickstart & Verification Guide: SPM Benchmark Target Isolation & `ttzip-bench` CLI Tooling

**Feature Directory**: `specs/136-spm-benchmark-target-isolation-and-cli-tooling`  
**Status**: Approved  

---

## 1. 验证场景一：运行 `ttzip-bench matrix` 终端评测

### 命令 (Command)
```bash
swift run ttzip-bench matrix
```

### 预期输出 (Expected Output)
```text
==========================================================================================================================
⚡️ TTZip Deflate-Bench Unified In-Memory Matrix (Total Points: 50)
==========================================================================================================================
[Idx] Engine     | Corpus        | Size  | Lvl | Comp Time  | Comp Rate   | Decomp Time| Decomp Rate | Ratio  | Status
--------------------------------------------------------------------------------------------------------------------------
...
Summary: 50/50 Points PASSED | Total Matrix Time: 1.080s | Median CV: 0.95%
==========================================================================================================================
```

### 故障排查 (Failure Diagnostic)
- 若编译报错找不到 `TTZipBench`：检查 `Package.swift` 中是否正确注册了 `.executable(name: "ttzip-bench", targets: ["TTZipBench"])` 和 `.executableTarget(name: "TTZipBench", dependencies: ["TTZipCore", "CTTZipBridge"])`。

---

## 2. 验证场景二：运行 `ttzip-bench help` 检查 CLI 帮助信息

### 命令 (Command)
```bash
swift run ttzip-bench help
```

### 预期输出 (Expected Output)
```text
OVERVIEW: TTZip Benchmark & Compression Telemetry CLI

USAGE: ttzip-bench <subcommand> [options]

SUBCOMMANDS:
  matrix        Execute the 50-point in-memory codec benchmark matrix
  plot          Generate Pareto frontier ASCII / SVG charts
  gate          Run automated regression and CV stability checks for CI/CD
  help          Display help information
```

### 故障排查 (Failure Diagnostic)
- 若命令未输出 USAGE 说明：检查 `Sources/TTZipBench/main.swift` 中的路由分发分支。
