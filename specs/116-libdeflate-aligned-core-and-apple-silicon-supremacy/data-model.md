# Data Model: libdeflate-Aligned Single-Core DEFLATE Engine with Apple Silicon Optimization

## Entities & Type Definitions

### 1. `CanonicalDeflateSequence`
Represents the compact 8-byte intermediate record encoding a literal run followed by a match pair.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `litrunlen_and_length` | `uint32_t` | $\ge 0$ | Bitpacked: low 23 bits = literal run length, high 9 bits = match length ($3..258$ or 0 for EOB) |
| `offset` | `uint16_t` | $[0, 32768]$ | Backward match distance in sliding dictionary window |
| `offset_slot` | `uint16_t` | $[0, 29]$ | Precalculated RFC 1951 distance slot index |

---

### 2. `CompactMatchfinderState`
Represents the 256 KB 16-bit relative index match finder state.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `hash3_tab_entries` | `uint32_t` | 32768 | Number of entries in 3-byte direct hash table ($64\text{ KB}$) |
| `hash4_tab_entries` | `uint32_t` | 65536 | Number of entries in 4-byte hash bucket table ($128\text{ KB}$) |
| `next_tab_entries` | `uint32_t` | 32768 | Number of entries in hash chain collision link table ($64\text{ KB}$) |
| `total_state_bytes` | `uint32_t` | 262144 | Total state memory allocation in bytes (256 KB) |
| `window_order` | `uint32_t` | 15 | Log2 of sliding window size ($2^{15} = 32768$) |

---

### 3. `EngineOptimizationProfile`
Represents the hardware execution mode and tuning configuration.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `level` | `uint32_t` | $[1, 12]$ | Compression level index |
| `max_search_depth` | `uint32_t` | $[0, 64]$ | Hash chain search depth limit |
| `nice_match_len` | `uint32_t` | $[4, 258]$ | Early match termination threshold |
| `use_neon_lz_extend` | `bool` | `true` or `false` | True when ARM64 128-bit NEON string extension is enabled |
| `use_fused_bitstream` | `bool` | `true` or `false` | True when 64-bit fused sequence bitstream packing is active |
| `use_unrolled_load` | `bool` | `true` or `false` | True when multi-candidate load unrolling is active |
| `target_throughput_mbs` | `double` | $> 0.0$ | Expected minimum throughput floor in MB/s |
| `target_space_savings_pct`| `double` | $[0.0, 100.0]$ | Expected space savings percentage on 100MB corpora |
