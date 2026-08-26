# Quickstart & Verification Guide: DeepSWE Pareto Visualization

## 验证场景 1：真实 100MB Wikipedia 语料软件级 PK 渲染

### 1. Command (可执行命令)
```bash
SPECIFY_FEATURE_DIRECTORY="specs/087-deep-swe-pareto-visualization" swift test --filter SoftwareParetoFrontierPkTests
```

### 2. Expected Output (成功输出断言)
```text
Test Suite 'SoftwareParetoFrontierPkTests' passed (8.5s)
📂 真实测试样本: .../enwik8.xml (大小: 95.37 MB)
========================================================================
🏆 真实语料 100MB enwik8 软件级 PK 帕累托图表已生成:
   图片路径: .../software_pareto_pk.png
------------------------------------------------------------------------
• TTZip (TAR.ZST) | 压缩速度: > 4000 MB/s | 空间节省: ~96.9% | 状态: 👑 帕累托前沿最优
• 7-Zip 26.02 (7Z Fast) | 压缩速度: > 2500 MB/s | 空间节省: ~97.2% | 状态: 👑 帕累托前沿最优
• 7-Zip 26.02 (7Z Ultra)| 压缩速度: ~18 MB/s    | 空间节省: 100.0% | 状态: 👑 帕累托前沿最优
========================================================================
```

### 3. Failure Diagnostic (失败排查路径)
- **现象**：`7-Zip` 未被测试到或速度为 0。
  - **排查**：检查本机 Homebrew 是否安装 `7zz`（执行 `which 7zz`），若缺失则自动跳过外部软件或执行 `brew install sevenzip`。
- **现象**：`enwik8.xml` 缺失。
  - **排查**：检查 `~/Library/Caches/com.ttzip.tests/fixtures/enwik8.xml` 是否存在，若缺失则自动回退到 `Sources/` 源码树真实多文件打包。

---

## 验证场景 2：CLI 内存基准图表生成

### 1. Command (可执行命令)
```bash
swift run ttzip-cli bench --in-memory --png-out docs/benchmarks/pareto_test.png --svg-out docs/benchmarks/pareto_test.svg
```

### 2. Expected Output (成功输出断言)
```text
🖼️  High-resolution PNG Pareto chart exported: docs/benchmarks/pareto_test.png
📈 Interactive SVG Pareto chart exported: docs/benchmarks/pareto_test.svg
```

### 3. Failure Diagnostic (失败排查路径)
- **现象**：提示 `Failed to encode PNG`。
  - **排查**：确认当前环境支持 `AppKit` 与 `CoreGraphics`，且目标目录具备写权限。
