# Feature Specification: 工业级 Git Worktree A/B 基准对标与统计显著性自动化流水线 (Feature 166)

**Feature ID**: `166-worktree-ab-benchmark-and-statistical-delta-workflow`  
**Created**: 2026-08-21  
**Status**: In Progress (Specification Phase)  
**Priority**: P0 (Infrastructure, Performance Engineering, Statistical Rigor)

---

## 1. Executive Summary

在极致性能与系统级底层开发中，任何性能增益（如微架构调优、NEON 向量指令、P/E 核调度、缓存分片）必须具备**可复现、无污染、严谨受控的同机对照验证**。
传统手动切分支测性能存在严重痛点：
1. **工作区污染与构建缓存残留**：频繁 `git checkout` 导致 CMake/Ninja 时间戳失效，脏目标文件破坏测量准确性；
2. **热节流（Thermal Throttling）与系统偶发噪声**：连续单轮运行受 CPU 降频或后台任务干扰，缺乏交替采样（Interleaved Sampling）；
3. **缺乏统计学置信度**：仅凭单次绝对数值对比容易误判（将系统抖动误认为性能提升）。

本特性的目标是：**建立工业级全自动 Git Worktree A/B 对标与统计差分引擎（`./scripts/benchmark_ab.sh` + `scripts/statistical_delta.py`），支持一键挂载任意两个 Git 引用（如 `HEAD` vs `HEAD~1`、`WIP` vs `origin/main`），执行自动隔离编译、N 轮交替采样、Welch 检验统计显著性分析，并输出 Markdown/JSON 双格式对标报告**。

---

## 2. User Scenarios

### User Scenario 1 (US1) - 一键对比任意两代 Commit / 分支性能 (One-Command Git A/B Benchmark)
- **As a**: 性能调优工程师 / 核心架构师
- **I want to**: 输入 `./scripts/benchmark_ab.sh <baseline_ref> <candidate_ref>`
- **So that**: 脚本全自动建立独立 worktree，使用相同 `-O3` 参数编译两端，自动交替采样多轮，最后自动清理环境。

### User Scenario 2 (US2) - 实验性未提交代码 (WIP) 与当前 HEAD 即时对标 (WIP vs HEAD Fast Differential)
- **As a**: 正在本地微调算法的开发者
- **I want to**: 直接运行 `./scripts/benchmark_ab.sh HEAD WIP`
- **So that**: 无需手动 commit 即可将当前工作区脏树与上一版本进行严谨的 A/B 跑分对比。

### User Scenario 3 (US3) - 统计学置信度与回归门禁拦截 (Statistical Delta & Regression CI Guard)
- **As a**: CI/CD 质量与发布管理者
- **I want to**: 获得包含均值（Mean）、标准差（$\sigma$）、中位数（Median）、变化率（$\Delta\%$）与 p-value（统计显著性）的对标报告
- **So that**: 若关键解压或编解码性能出现 $> 3\%$ 且 $p < 0.01$ 的确定性回退，自动在终端标红并作为 CI 红灯退出。

---

## 3. Functional Requirements

- **REQ-001 (Isolated Worktree Lifecycle)**: `scripts/benchmark_ab.sh` 必须支持 `git worktree add -f` 挂载临时工作区，并通过 `trap ... EXIT INT TERM` 确保在任何异常或退出时 100% 自动执行 `git worktree remove --force`。
- **REQ-002 (Hermetic Build Isolation)**: Baseline 与 Candidate 必须分别在各自独立的 `build/` 目录下完成 CMake Release (`-DCMAKE_BUILD_TYPE=Release -DCMAKE_C_FLAGS="-O3"`) 独立编译，严禁共享中间产物。
- **REQ-003 (Interleaved Sampling Loop)**: 支持配置轮数 $N$（默认 5 轮），在每轮中交替运行：$B_1 	o C_1 	o B_2 	o C_2 \dots 	o B_N 	o C_N$，并在首轮前执行 1 轮丢弃的预热（Warm-up）。
- **REQ-004 (Statistical Engine)**: `scripts/statistical_delta.py` 解析采集的原始 JSON 遥测数组，计算：
  - 均值 $\mu$，标准差 $\sigma$
  - Welch 异方差 t 检验计算 p-value
  - 置信度判定（$p < 0.05$ 判定为统计显著差异，否则标记为系统噪声 `~`）
- **REQ-005 (Dual Report Output)**: 终端打印 ANSI 色彩对比表（绿色提速、红色回退、灰色无显著差异），并持久化落盘至 `reports/ab_bench_<timestamp>.md` 与 `reports/ab_bench_<timestamp>.json`。
- **REQ-006 (Regression Threshold Thresholding)**: 支持参数 `--threshold <float>`（如 0.03 表示 3%），当任一核心算法出现显著负向退化时以非零状态码退出。

---

## 4. Success Criteria

1. **自动化闭环率**: 运行 `./scripts/benchmark_ab.sh` 全流程无人工干预，无论成功与否工作区 0 残留；
2. **统计有效性**: 报告提供 $N \ge 3$ 轮采样的真实均值与标准差，彻底消除单次偶然抖动；
3. **多算法覆盖**: 覆盖 10 大编解码器（Deflate, Zstd, LZ4, FL2, LZFSE, Snappy, Brotli, Bzip2, Blosc）、硬件 SIMD 校验和及容器打包解包；
4. **执行效率**: 5 轮全量交替基准对比耗时控制在 **< 15 秒**。
