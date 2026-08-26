# Quickstart & Verification Guide: 044-libarchive-test-harness-and-logging

**Feature**: [spec.md](./spec.md)  
**Created**: 2026-08-17  
**Status**: Completed  

---

## 1. 验证场景一：运行本地极速 Native 诊断测试 (`ttzip-cli test --fast`)

### Command
```bash
swift run ttzip-cli test --fast
```

### Expected Output
- 控制台输出 ANSI 格式化的 TTZip Native Diagnostic Suite 进度。
- 16 种格式的压缩、解压与 CRC32 往返验证通过。
- 汇总统计显示：`100% PASS`，总耗时 $\le 2.0\text{s}$，进程退出码为 `0`。

### Failure Diagnostic
- **排查路径**：若出现格式失败，查看控制台输出的首个分歧 CRC32 与格式名称。
- **常见原因**：相关 C 桥接引擎静态库未链接或缓冲区大小越界。

---

## 2. 验证场景二：针对特定用例运行并输出详细诊断报告 (`ttzip-cli test --filter ... -v`)

### Command
```bash
swift run ttzip-cli test --filter "LibarchiveGoldenCorpusTests" -v
```

### Expected Output
- 仅执行匹配 `LibarchiveGoldenCorpusTests` 的测试用例（30+ 黄金语料包）。
- 控制台以树状结构输出每个用例的耗时与断言数。
- 进程退出码为 `0`。

### Failure Diagnostic
- **排查路径**：若某项 `.uu` 样本解码失败，启用 `-vv` 查看具体字节偏移量与 UUDecoder 状态机报错。
- **常见原因**：`Vendor/libarchive-upstream/libarchive/test/` 语料文件缺失或路径变动。

---

## 3. 验证场景三：生成持久化 Markdown 与 JSON 报告

### Command
```bash
swift run ttzip-cli test --fast --json-report docs/test_reports/quickstart_test.json --markdown-report docs/test_reports/quickstart_test.md
```

### Expected Output
- 磁盘生成 `docs/test_reports/quickstart_test.json`，且通过 JSON Schema 强类型校验。
- 磁盘生成 `docs/test_reports/quickstart_test.md`，包含 KPI 仪表盘与测试明细表格。

### Failure Diagnostic
- **排查路径**：检查 `docs/test_reports/` 目录是否存在写入权限，核验生成的 JSON 文件字段完整性。
- **常见原因**：父目录未自动创建或序列化发生循环引用。

---

## 4. 验证场景四：一键本地 CI 全回归执行 (`./scripts/run_local_ci.sh`)

### Command
```bash
./scripts/run_local_ci.sh --quick
```

### Expected Output
- 顺序执行：
  1. 代码风格静态检查 (`SwiftLint`)
  2. 极速 Native 诊断测试 (`ttzip-cli test --fast`)
  3. 核心单元测试 (`MmapBufferHandleTests`, `LibarchiveGoldenCorpusTests`, `FrontendPerformanceGateTests`)
- 控制台输出统一的绿色通关徽章 `[✓ ALL LOCAL CI GATES PASSED]`，退出码为 `0`。

### Failure Diagnostic
- **排查路径**：查看 `./scripts/run_local_ci.sh` 打印的标红失败步骤。
- **常见原因**：代码格式未通过 Lint 规则或性能门禁发生倒退。
