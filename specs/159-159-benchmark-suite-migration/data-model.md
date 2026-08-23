# Data Model: Native C11 Benchmark Architecture & Result Schema

**Feature**: `159-159-benchmark-suite-migration`  
**Date**: 2026-08-20  
**Status**: Completed  

---

## 1. C Struct Models in `ttzip_benchmark_harness.h`

### 1.1 `ttzip_bench_result_t`
Represents an individual codec or checksum benchmark measurement.

```c
typedef struct {
    char        name[64];           // Benchmark item name (e.g. "Deflate (L1)")
    char        category[32];       // Category (e.g. "Codec", "Checksum", "VFS")
    size_t      src_size;           // Input size in bytes
    size_t      dst_size;           // Output size in bytes
    uint64_t    elapsed_nanos;      // Duration in nanoseconds
    double      throughput_mbs;     // Processing speed in MB/s
    double      compression_ratio;  // Ratio percentage (dst_size / src_size * 100.0)
    double      mips_score;         // Computed efficiency MIPS score
    bool        passed;             // Status of assertion check
} ttzip_bench_result_t;
```

---

### 1.2 `ttzip_pareto_point_t`
Represents a coordinate point on the Pareto efficiency frontier.

```c
typedef struct {
    char        codec_name[32];
    double      ratio_pct;          // Compression ratio (lower is better)
    double      speed_mbs;          // Throughput (higher is better)
    bool        is_pareto_optimal;  // True if non-dominated
} ttzip_pareto_point_t;
```

---

## 2. JSON Schema Mapping

The data model maps 1:1 to `contracts/benchmark-report-schema.json`.
- `schema_version`: string `"1.0.0"`
- `total_benchmarks`: integer $\ge 1$
- `total_duration_ms`: number
- `results`: array of `ttzip_bench_result_t`
- `pareto_points`: array of `ttzip_pareto_point_t`
