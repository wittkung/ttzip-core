# Data Model: Single-Core L3/L4 Intermediate Pareto Dominance

## Entities & Type Definitions

### 1. `TierProfileConfig`
Represents the exact algorithmic configuration for each discrete single-core DEFLATE compression tier.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `tier_id` | `uint32_t` | $[0, 7]$ | Unique identifier for the compression tier (e.g. 3 for Normal, 4 for Maximum) |
| `tier_name` | `string` | non-empty, max 32 chars | Descriptive name of the compression profile (e.g. "Normal (3)", "Maximum (4)") |
| `matchfinder_type` | `string` | Enum: `["FAST_GREEDY", "FAST_LAZY", "DEEP_LAZY", "ZOPFLI"]` | Match finder algorithm architecture |
| `max_chain_depth` | `uint32_t` | $[0, 64]$ | Maximum number of hash chain links traversed per candidate search |
| `nice_match_len` | `uint32_t` | $[4, 258]$ | Threshold match length that triggers immediate chain search termination |
| `lookahead_steps` | `uint32_t` | $[0, 2]$ | Lookahead search steps ($0 = \text{greedy}, 1 = \text{standard lazy}, 2 = \text{2-step lazy}$) |
| `skip_intermediate_hashes` | `bool` | `true` or `false` | When true, skips per-byte hash insertion inside long matches |
| `target_throughput_mbs` | `double` | $> 0.0$ | Minimum required throughput floor in MB/s |
| `target_space_savings_pct`| `double` | $[0.0, 100.0]$ | Expected space savings percentage on standard 100MB text corpora |

---

### 2. `ChunkStreamingContext`
Defines the cache-resident chunking state for multi-block single-core DEFLATE compression.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `chunk_size_bytes` | `uint32_t` | $[32768, 131072]$ | Size of each processed slice in bytes (default 65,536 bytes) |
| `history_size_bytes`| `uint32_t` | $[0, 32768]$ | Active sliding window history size preceding the current chunk |
| `token_buffer_capacity`| `uint32_t` | $\ge 65536$ | Maximum number of `ttzip_deflate_token_t` elements in thread-local storage |
| `is_final_chunk` | `bool` | `true` or `false` | True if the current chunk terminates the stream (`BFINAL = 1`) |
| `bytes_processed` | `uint64_t` | $\ge 0$ | Total uncompressed input bytes processed across all chunks |
| `bytes_written` | `uint64_t` | $\ge 0$ | Total compressed bytes emitted to the output buffer |

---

### 3. `ParetoBenchmarkEvaluation`
Defines the analytical outcome comparing TTZip single-core tiers against external competitors.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `benchmark_id` | `string` | non-empty, max 64 chars | Identifier of benchmark run (e.g. `pareto_pk_zip_singlecore_enwik8`) |
| `ttzip_tier3_throughput_mbs` | `double` | $\ge 1200.0$ | Physical single-core throughput of TTZip Tier 3 in MB/s |
| `ttzip_tier3_savings_pct` | `double` | $\ge 65.0$ | Space savings percentage of TTZip Tier 3 |
| `ttzip_tier4_throughput_mbs` | `double` | $\ge 850.0$ | Physical single-core throughput of TTZip Tier 4 in MB/s |
| `ttzip_tier4_savings_pct` | `double` | $\ge 66.5$ | Space savings percentage of TTZip Tier 4 |
| `libdeflate_l3_throughput_mbs`| `double` | $> 0.0$ | Competitor baseline throughput for libdeflate Level 3 |
| `libdeflate_l6_throughput_mbs`| `double` | $> 0.0$ | Competitor baseline throughput for libdeflate Level 6 |
| `is_pareto_strictly_dominant` | `bool` | `true` or `false` | True if TTZip points form a strictly upper-right bounding Pareto envelope |
