# Data Model: Apple Silicon P/E 核异构调度与 L2 Cache 簇感知 (Feature 165)

## 1. 硬件微架构拓扑模型 (`ttzip_cpu_topology_t`)

Represents the physical and logical CPU cluster topology detected at runtime.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `nperflevels` | `integer` | No | Number of performance tiers (typically 2 on Apple Silicon: P and E) |
| `p_cores` | `integer` | No | Count of Performance Cores (`perflevel0`) |
| `p_l2_cache_bytes` | `integer` | No | L2 cache size of P-core cluster (e.g. 12MB, 16MB, 24MB) |
| `p_cpus_per_l2` | `integer` | No | Number of P-cores sharing one L2 cache cluster |
| `p_l1d_cache_bytes` | `integer` | No | L1 data cache size per P-core (128 KB) |
| `e_cores` | `integer` | No | Count of Efficiency Cores (`perflevel1`) |
| `e_l2_cache_bytes` | `integer` | No | L2 cache size of E-core cluster (e.g. 4MB) |
| `e_cpus_per_l2` | `integer` | No | Number of E-cores sharing one L2 cache cluster |
| `e_l1d_cache_bytes` | `integer` | No | L1 data cache size per E-core (64 KB) |
| `cacheline_bytes` | `integer` | No | Cache line width (128 bytes on Apple Silicon ARM64, 64 bytes on x86) |

---

## 2. 异构调度 QoS 策略模型 (`ttzip_qos_policy_t`)

Defines execution routing and slicing parameters for task dispatch.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `qos_tier` | `string` | No | One of: `performance`, `efficiency`, `all_cores` |
| `target_qos_class` | `string` | No | Darwin QoS: `QOS_CLASS_USER_INTERACTIVE` or `QOS_CLASS_UTILITY` |
| `optimal_chunk_size_bytes` | `integer` | No | L2 cache cluster aligned chunk size (64KB ~ 4MB) |
| `worker_thread_count` | `integer` | No | Allocated threads dedicated to this tier |
| `energy_saving_mode` | `boolean` | No | True when dispatch is restricted to E-cores to minimize heat/power |
