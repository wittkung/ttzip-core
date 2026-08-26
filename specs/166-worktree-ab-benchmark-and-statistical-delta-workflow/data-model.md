# Data Model: Git Worktree A/B 对标与统计差分引擎 (Feature 166)

## 1. 对标执行元数据模型 (`ABBenchmarkSession`)

Represents the overall metadata and environmental context of an A/B benchmark run.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `session_id` | `string` | No | Unique session identifier (e.g. `ab_1740000000`) |
| `timestamp` | `string` | No | ISO 8601 execution timestamp |
| `baseline_ref` | `string` | No | Git ref/tag/SHA for baseline (e.g. `HEAD~1`, `v1.2.0`) |
| `baseline_commit` | `string` | No | Exact 40-char SHA of baseline |
| `candidate_ref` | `string` | No | Git ref/SHA/`WIP` for candidate |
| `candidate_commit` | `string` | No | Exact 40-char SHA or `dirty_working_tree` |
| `sample_runs` | `integer` | No | Total interleaved iteration count ($N \ge 3$) |
| `platform` | `string` | No | OS and architecture (e.g. `Darwin arm64 (Apple Silicon)`) |
| `overall_verdict` | `string` | No | One of: `PASSED_NO_REGRESSION`, `REGRESSION_DETECTED` |

---

## 2. 单项指标统计差分模型 (`MetricStatisticalDelta`)

Captures the multi-sample statistical distribution, delta, and hypothesis test for a single metric.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `category` | `string` | No | `codec_throughput`, `checksum_throughput`, `container_io`, `peak_rss` |
| `metric_name` | `string` | No | Name of metric (e.g. `LZ4_HC_L9_Decomp_MBs`) |
| `unit` | `string` | No | Unit string (e.g. `MB/s`, `GB/s`, `CPB`, `MB`) |
| `baseline_mean` | `number` | No | Sample mean $\mu_A$ of baseline |
| `baseline_std` | `number` | No | Sample standard deviation $\sigma_A$ |
| `candidate_mean` | `number` | No | Sample mean $\mu_B$ of candidate |
| `candidate_std` | `number` | No | Sample standard deviation $\sigma_B$ |
| `delta_percent` | `number` | No | Relative change percentage $(\mu_B - \mu_A) / \mu_A 	imes 100\%$ |
| `t_statistic` | `number` | No | Welch t-statistic |
| `degrees_of_freedom` | `number` | No | Welch-Satterthwaite degrees of freedom $
u$ |
| `p_value` | `number` | No | Two-tailed p-value from Student's t distribution |
| `verdict` | `string` | No | `SIGNIFICANT_SPEEDUP`, `SIGNIFICANT_REGRESSION`, `NOISE_FLAT` |
