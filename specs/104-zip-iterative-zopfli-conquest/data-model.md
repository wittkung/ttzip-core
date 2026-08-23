# Data Model: Feature 104 (ZIP Iterative Zopfli Conquest Engine)

## C-Level Entities (`Sources/CTTZipBridge/include/ttzip_zopfli_engine.h`)

### `TTZipZopfliOptions`
Configuration structure passed to C compression engine:
```c
typedef struct {
    int32_t compression_level;      // 1..12
    int32_t num_iterations;         // 1..15 passes
    int32_t block_splitting;        // 0 or 1
    int32_t max_block_splits;       // 0..15
    double early_exit_threshold;    // e.g. 0.00005 (0.005%)
} TTZipZopfliOptions;
```

### `TTZipZopfliThreadContext`
Thread-local scratchpad state for zero-allocation iterative DP search:
```c
typedef struct {
    int32_t head[65536];              // Hash table: 256 KB
    uint16_t prev[32768];             // Hash chain: 64 KB
    uint16_t same[32768];             // Run-length accelerator: 64 KB
    uint32_t cost[131072 + 1];        // DAG cost array: 512 KB
    uint32_t from[131072 + 1];        // Predecessor pointers: 512 KB
    uint16_t litlens[131072];         // LZ77 literals/lengths: 256 KB
    uint16_t dists[131072];           // LZ77 distances: 256 KB
    uint32_t litlen_counts[288];      // Symbol histogram: 1.15 KB
    uint32_t dist_counts[32];         // Distance histogram: 128 B
    uint32_t litlen_costs[288];       // Q8.8 fixed-point bit costs: 1.15 KB
    uint32_t dist_costs[32];          // Q8.8 fixed-point bit costs: 128 B
} TTZipZopfliThreadContext;
```

## Swift-Level Models (`Sources/TTZipCore/Zip/ZipCompressionProfile.swift`)

### `ZipCompressionProfile`
Strong-typed configuration model for all 8 tiers:
- `id: String` (Unique identifier e.g. `zip_tier_7_extreme_peak`)
- `name: String` (Display name)
- `level: ArchiveCompressionLevel` (Mapped level)
- `deflateLevel: Int32` (Target Deflate level 0..12)
- `zopfliIterations: Int32` (Number of optimization passes 0..15)
- `blockSplitting: Bool` (Dynamic block splitting flag)
- `maxBlockSplits: Int32` (Maximum block splits)
- `earlyExitThreshold: Double` (Convergence delta threshold)
