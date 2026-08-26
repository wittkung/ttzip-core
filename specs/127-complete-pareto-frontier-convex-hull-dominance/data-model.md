# Data Model: Complete Pareto Frontier Convex Hull Dominance

## 1. Engine Entities

### `DeflateTierConfig`
- `tier`: Integer [0..12]
- `engineType`: String enum [`direct_store`, `hybrid_fast_3b`, `word4_fast`, `ht4_compact_lazy`, `deep_chain_lazy`, `zopfli_dp`]
- `maxChainDepth`: Integer [0..128]
- `niceMatchLength`: Integer [0..258]
- `earlyLazyCutoff`: Integer [0..32]
- `dynamicHuffman`: Boolean
- `zopfliIterations`: Integer [0..15]

### `ParetoBenchmarkPoint`
- `software`: String (e.g. "TTZip 1-Core", "libdeflate", "Apple Native", "7-Zip")
- `tierName`: String (e.g. "L1 (Fast)", "Level 6")
- `compressedSizeBytes`: Integer
- `compressedSizeMB`: Number
- `elapsedSeconds`: Number
- `throughputMBps`: Number
- `isConvexHull`: Boolean

---

## 2. Matchfinder Memory Topologies

| Matchfinder Struct | Table Dimensions | Memory Footprint | Cache Level |
| :--- | :--- | :--- | :--- |
| `ttzip_deflate_hybrid_fast_mf_t` | $32,768 \times 2\text{B}$ (1-way direct 3-byte) | 64 KB | 100% L1 D-Cache |
| `ttzip_deflate_word4_fast_mf_t` | $16,384 \times 4\text{B}$ (2-way bucket 4-byte) | 64 KB | 100% L1 D-Cache |
| `ttzip_deflate_4way_lazy_mf_t` | $8,192 \times 8\text{B}$ (4-way bucket HT-4) | 64 KB | 100% L1 D-Cache |
| `ttzip_deflate_chain_lazy_mf_t` | $32,768 \times 2\text{B} + 65,536 \times 2\text{B} + 32,768 \times 2\text{B}$ | 256 KB | L2 / L1 Spilling |
