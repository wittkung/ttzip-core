# Data Model: Near-Optimal DP Acceleration and Full-Spectrum Pareto Supremacy

## Entities & Type Definitions

### 1. `NearOptimalDPOptions`
Configuration options controlling dynamic programming passes and convergence thresholds.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `max_optim_passes` | `uint32_t` | $[1, 10]$ | Maximum number of DP optimization passes |
| `min_improvement_to_continue` | `uint32_t` | $\ge 1$ | Minimum bit improvement required to trigger next pass |
| `use_slot_boundary_pruning` | `bool` | `true` or `false` | True when evaluating transitions only at length slot endpoints |
| `use_pareto_edge_filtering` | `bool` | `true` or `false` | True when dominated short matches are pruned before DP |
| `target_throughput_mbs` | `double` | $\ge 35.0$ | Minimum throughput floor in MB/s |
| `target_compressed_size_mb` | `double` | $\le 3.05$ | Maximum compressed size in MB for 100MB corpora |

---

### 2. `DualOrderSignatureProbe`
Descriptor for in-table 64-bit dual-order hash probing.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `window_relative_pos` | `uint16_t` | $[0, 32767]$ | 16-bit sliding window position |
| `secondary_signature` | `uint16_t` | $[0, 65535]$ | 16-bit composite hash3/byte4 signature |
| `sibling_relative_pos` | `uint16_t` | $[0, 32767]$ | 16-bit 2-way sibling match position |
| `sibling_secondary_signature` | `uint16_t` | $[0, 65535]$ | 16-bit sibling composite signature |

---

### 3. `SpectrumTierProfile`
Specification for complete 8-tier compression spectrum.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `tier_index` | `uint32_t` | $[0, 7]$ | Tier index 0 to 7 |
| `name` | `string` | non-empty | User-facing preset name |
| `deflate_level` | `uint32_t` | $[0, 15]$ | Internal compressor engine level |
| `expected_throughput_mbs` | `double` | $> 0.0$ | Expected throughput floor |
| `expected_compressed_size_mb`| `double` | $> 0.0$ | Expected compressed size limit |
| `is_pareto_frontier_point` | `bool` | `true` | True when mathematically verified on convex hull |
