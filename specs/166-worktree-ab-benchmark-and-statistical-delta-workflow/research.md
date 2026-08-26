# Research Findings: 工业级 Git Worktree A/B 基准对标与统计显著性自动化流水线 (Feature 166)

## R001 [SUBAGENT:research]: Git Worktree Lifecycle & Robust POSIX Shell Trap Handler (`scripts/benchmark_ab.sh`)

- **Decision**: Implement an isolated, zero-residue, dual-worktree orchestration script `scripts/benchmark_ab.sh` adhering to POSIX/Bash (`set -euo pipefail`):
  1. **Directory Isolation**: All temporary worktrees are created under `.worktrees/ab_<PID>_<TIMESTAMP>/` (git-ignored in `.gitignore`).
  2. **Branch Detachment**: Use `git worktree add --detach <path> <ref>` for baseline and candidate git refs, avoiding branch checkout lockouts.
  3. **WIP Dirty Workspace Support**: If candidate is `WIP`, `.` or `DIRTY`, run directly against the current workspace using isolated build paths (`build_ab_wip/`), while baseline is checked out in a detached worktree.
  4. **Hermetic Compilation**: Baseline and candidate build independently with `cmake -B <build_dir> -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_FLAGS="-O3 -DNDEBUG"`.
  5. **Cross-Over Interleaved Sampling**: Run 1 warm-up round (discarded), followed by $N$ iterations (default: 5) alternating order ($B \to C$ on odd rounds, $C \to B$ on even rounds) to eliminate CPU thermal throttling (DVFS) bias.
  6. **Airtight POSIX Trap**: `trap cleanup EXIT INT TERM HUP` guaranteeing automatic `git worktree remove --force` and pruning under all exit/interrupt scenarios.
- **Rationale**: In-place switching (`git checkout`) destroys build artifacts, alters mtime timestamps, risks losing uncommitted edits, and causes massive rebuild overhead. `git worktree` creates sub-100ms linked worktrees with zero disk duplication.
- **Alternatives Considered**: In-place branch switching with `git stash`. Rejected because stash conflicts can occur, interrupts leave developers on detached commits, and CMake mtimes are invalidated.
- **Source**:
  - `scripts/run_optimization_gate.sh:1-60`
  - `scripts/local-ci.sh:1-51`
  - `.gitignore:57`

---

## R002 [SUBAGENT:research]: Pure Python Zero-Dependency Statistical Engine (`scripts/statistical_delta.py`)

- **Decision**: Implement a pure Python 3 CLI engine in `scripts/statistical_delta.py` using strictly standard library modules (`math`, `statistics`, `json`, `sys`, `argparse`):
  1. **Sample Statistics**: Mean $\mu$, Standard Deviation $\sigma$ with Bessel's correction $n-1$.
  2. **Delta Percentage**: $\Delta\% = (\mu_B - \mu_A) / \mu_A \times 100\%$.
  3. **Welch's Unequal-Variance t-Test**:
     $$t = \frac{\mu_B - \mu_A}{\sqrt{\frac{s_A^2}{n_A} + \frac{s_B^2}{n_B}}}, \quad \nu = \frac{(\frac{s_A^2}{n_A} + \frac{s_B^2}{n_B})^2}{\frac{(s_A^2/n_A)^2}{n_A - 1} + \frac{(s_B^2/n_B)^2}{n_B - 1}}$$
  4. **Exact Zero-Dependency p-value via Continued Fraction**: Evaluated using Regularized Incomplete Beta Function $I_x(\nu/2, 1/2)$ via Lentz's continued fraction method in pure Python, achieving $< 10^{-12}$ error vs `scipy.stats`.
  5. **Decision Boundary**:
     - If $p < 0.05$ and $\Delta\% > +1.0\%$: `[Significant Speedup]` (🟢 GAIN)
     - If $p < 0.05$ and $\Delta\% < -1.0\%$: `[Significant Regression]` (🔴 REGRESSION)
     - If $p \ge 0.05$: `[Noise / No Change]` (⚪ Statistically Indistinguishable)
  6. **Automated CI Regression Gate**: `--threshold <float>` (default `0.03`). Exits with code `1` if any core metric exhibits statistically significant degradation beyond threshold.
- **Rationale**: Eliminates external Python package dependencies (`scipy`, `numpy`) on developer machines, while preventing flaky CI failures from single-run outlier noise.
- **Alternatives Considered**: Simple percentage threshold comparison without p-value. Rejected because background system noise causes false positive alerts on single-run outliers.
- **Source**:
  - `scripts/audit_performance_regression.py:1-231`
  - `scripts/upstream_report_gen.py:1-92`

---

## R003 [SUBAGENT:research]: Dual-Mode Report Generation (CLI ANSI & Markdown Export)

- **Decision**: Standardize telemetry aggregation across 10-Codec throughput/CPB, Hardware SIMD Checksums, and Multi-File Container Packaging/Extraction.
  - **CLI ANSI Output**: Fixed-width color-coded table (Green for speedups, Red for regressions, Dim for noise) with statistical metadata.
  - **Markdown Output**: `reports/ab_bench_<timestamp>.md` formatted with GitHub alert callouts and full sample metrics ($\mu \pm \sigma$, $\Delta\%$, $p$).
  - **JSON Output**: `reports/ab_bench_<timestamp>.json` complying strictly with Draft-07 JSON Schema.
- **Rationale**: Provides immediate terminal feedback for interactive workflows while generating structured artifacts for PR reviews and CI regression gating.
- **Source**:
  - `tests/c/bench_codecs.c:1-279`
  - `tests/c/bench_checksums.c:1-77`
  - `tests/c/bench_formats.c:1-188`
