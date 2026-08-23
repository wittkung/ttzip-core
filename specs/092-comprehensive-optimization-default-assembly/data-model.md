# Phase 1 Data Model: Comprehensive Optimization Default Assembly

**Feature**: `specs/092-comprehensive-optimization-default-assembly`  
**Date**: 2026-08-18  

---

## 1. Entities & Structural Definitions

```mermaid
classDiagram
    class AdaptivePipelineOrchestrator {
        +probe(sample_data, length) TuningDecision
        +orchestrate_compression(input_path, format, level) CompressionPipelineConfig
        +is_high_entropy(shannon_entropy) bool
        +is_scientific_float(stride_score, exponent_sd) bool
    }

    class TuningDecision {
        +double shannon_entropy
        +uint8_t detected_type_size
        +bool is_uniform
        +uint8_t special_code
        +uint64_t repeat_pattern
        +bool recommend_direct_store
        +bool recommend_bitgroom
        +uint8_t recommend_nsd
    }

    class MultiModalDatasetGenerator {
        +generate_float32_sensor(dest_path, size_bytes) void
        +generate_high_entropy_binary(dest_path, size_bytes) void
        +generate_sparse_extent_image(dest_path, virtual_bytes, allocated_bytes) void
        +generate_structured_json_stream(dest_path, size_bytes) void
    }

    class CompetitorMatrixHarness {
        +run_all_competitors(dataset, format, level) CompetitorBenchmarkRow
        +verify_integrity(archive_path, extract_dir) bool
    }

    AdaptivePipelineOrchestrator --> TuningDecision : Produces
    MultiModalDatasetGenerator --> CompetitorMatrixHarness : Feeds Datasets
    CompetitorMatrixHarness --> AdaptivePipelineOrchestrator : Transparently Uses
```

---

## 2. Invariants & Data Constraints

1. **Zero-Configuration Invariant (Default Transparent Behavior)**:
   - For all regular files $\ge 16\text{KB}$, the adaptive probe MUST execute within $5.0\,\mu\text{s}$ per file without requiring any explicit user flags.
   - When Shannon entropy $H > 7.65$, compression method MUST automatically degrade to Store/Direct (Method 0 / raw uncompressed copy).
2. **Float32 Detection Invariant**:
   - Stride autocorrelation $R(4) \ge 0.70$ AND normalized float ratio $\ge 0.95$ AND exponent standard deviation $\sigma_E \le 16.0$ MUST be satisfied before injecting Bit-Grooming.
   - Bounded relative error for $\text{NSD}=3$ MUST be $\le 0.5\%$.
3. **Zero Heap Allocation on Stream Ingestion**:
   - Dataset generation and micro-sampling MUST use page-aligned POSIX buffers (`PlatformMemory.allocateAlignedPageBuffer(byteCount: 65536)`) and direct file descriptors. No `Data(count:)` multi-megabyte heap allocation.
