# Data Model: Feature 131 - Upstream Contribution & Benchmark Verification Protocol

## 1. Core Entities

### 1.1 BenchmarkRun
Represents a single benchmark execution of a binary with its environment configuration and raw execution times.

| Field Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `run_id` | `string` (UUIDv4) | Unique identifier of the run | Yes |
| `binary_name` | `string` | Identifier of the executable (e.g., `benchmark_zlib`) | Yes |
| `commit_sha` | `string` (7-char hex) | Git commit SHA | Yes |
| `compiler_flags` | `string[]` | List of compiler flags used during build | Yes |
| `order` | `enum("candidate_first", "baseline_first")` | Execution sequence order | Yes |
| `repetition_count` | `integer` | Number of repetitions (minimum 5) | Yes |
| `measurements` | `BenchmarkPoint[]` | Array of measured execution points | Yes |

### 1.2 BenchmarkPoint
Represents a single workload/level data point.

| Field Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `name` | `string` | Benchmark workload name (e.g. `striped_rgb`, `text`) | Yes |
| `level` | `integer` | Compression level (1, 3, 6, 9) | Yes |
| `mean_cpu_time_ns` | `number` | Arithmetic mean of CPU time in nanoseconds | Yes |
| `stddev_cpu_time_ns` | `number` | Standard deviation in nanoseconds | Yes |
| `iterations` | `integer` | Total iterations executed | Yes |

### 1.3 CrossOverComparison
Represents the combined statistical comparison across mirrored runs.

| Field Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `workload` | `string` | Workload name | Yes |
| `level` | `integer` | Compression level | Yes |
| `gain_order_a_pct` | `number` | Candidate-first relative gain percentage | Yes |
| `gain_order_b_pct` | `number` | Baseline-first relative gain percentage | Yes |
| `crossover_mean_pct` | `number` | True cross-over mean relative gain | Yes |
| `classification` | `enum("significant_gain", "mild_gain", "parity", "minor_variance")` | Statistical characterization | Yes |

### 1.4 PreFlightAuditReport
Represents the pre-flight verification gate before any remote publish action.

| Field Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `flag_parity_verified` | `boolean` | Whether compiler flags match 100% | Yes |
| `cross_over_completed` | `boolean` | Whether mirrored runs completed | Yes |
| `tests_passed` | `boolean` | Whether 100% CTest/GTest passed | Yes |
| `warnings_count` | `integer` | Total compiler warnings (must be 0) | Yes |
| `user_authorized` | `boolean` | Explicit user approval recorded in session | Yes |
