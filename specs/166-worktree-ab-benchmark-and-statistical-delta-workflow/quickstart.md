# Quickstart Guide: Git Worktree A/B 基准对标与统计差分引擎 (Feature 166)

## Scenario 1: 对比当前工作区 (WIP) 与当前 HEAD
- **Command**:
  ```bash
  ./scripts/benchmark_ab.sh HEAD WIP --runs 5
  ```
- **Expected Output**:
  - 自动创建 `.worktrees/` 临时构建区
  - 执行 1 轮预热 + 5 轮交替采样 ($B_1 \to C_1 \to B_2 \to C_2 \dots$)
  - 打印 ANSI 色彩统计对比表（包含 Mean $\pm \sigma$、$\Delta\%$、p-value）
  - 自动清理 worktree，并在 `reports/` 目录下生成 Markdown 与 JSON 报告。

---

## Scenario 2: 对比两个历史 Commit / Tag
- **Command**:
  ```bash
  ./scripts/benchmark_ab.sh HEAD~1 HEAD --runs 5 --threshold 0.03
  ```
- **Expected Output**:
  - 全自动挂载两个 detached worktree，独立以 `-O3` 编译
  - 若任一核心算法出现超过 3% 的显著性能回退，脚本以退出码 1 拦截并报警。
