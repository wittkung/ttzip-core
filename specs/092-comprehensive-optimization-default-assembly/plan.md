# Implementation Plan: Comprehensive Optimization Default Assembly

**Feature Directory**: `specs/092-comprehensive-optimization-default-assembly/`  
**Status**: APPROVED FOR IMPLEMENTATION  
**Priority**: P1  
**Created**: 2026-08-18  

---

## 1. Technical Context & Constitution Check

### Technical Context
- **Target Subsystems**: `Sources/TTZipCore/` (Pipeline Orchestrator, Template Method Skeleton, Competitor Benchmark Runner) + `Sources/CTTZipBridge/` (NEON Cascade Probes, Float Detectors).
- **Target Architectures**: Apple Silicon ARM64 (NEON, PMULL) + Intel x86_64 fallback.
- **Invariants**: Zero configuration burden for users (rule §5.7), zero heap allocations on hot ingestion paths, $< 5\,\mu\text{s}$ per-file probe budget.

### Constitution Check
- [x] **Zero-Cost Abstraction**: All adaptive heuristics evaluate over 16KB stack-resident buffers in $< 3.5\,\mu\text{s}$.
- [x] **Zero Dynamic Allocation on Hot Paths**: Probing and dataset streaming use POSIX page-aligned buffers.
- [x] **Fast-Path Preservation**: Built-in ZIP and 7z direct routes remain fully active without indirect jump penalties.

---

## 2. Phase 0: Grounded Research Index

- `R001` [SUBAGENT:research]: Transparent Adaptive Heuristic Probing Integration in Generic ArchiveWriter and BaseArchiveEngineTemplate.
- `R002` [SUBAGENT:research]: Scientific Float Automatic Detection (Stride Autocorrelation & Exponent Variance) and Transparent Bit-Grooming Pipeline Coupling.
- `R003` [SUBAGENT:research]: Full-Stack Competitor Benchmark Matrix Wiring with Multi-Modal Datasets in CompetitorBenchmarkRunner.

---

## 3. Phase 1: Design & Contract Index

- `Data Model`: [`data-model.md`](data-model.md)
- `Contracts`:
  - [`contracts/heuristic-probe-contract.json`](contracts/heuristic-probe-contract.json)
  - [`contracts/scientific-float-contract.json`](contracts/scientific-float-contract.json)
  - [`contracts/benchmark-wiring-contract.json`](contracts/benchmark-wiring-contract.json)
- `Quickstart Validation Guide`: [`quickstart.md`](quickstart.md)

---

## 4. Component Change Manifest

### C Native Bridge Layer (`Sources/CTTZipBridge/`)
- `[MODIFY]` `include/CTTZipHeuristicTuner.h`: Add `ttzip_detect_scientific_float_neon` declaration.
- `[MODIFY]` `CTTZipHeuristicTuner.c`: Implement 16KB NEON IEEE-754 exponent variance and stride detector.

### Swift Platform & Engine Layer (`Sources/TTZipCore/`)
- `[NEW]` `Sources/TTZipCore/Adaptive/AdaptivePipelineOrchestrator.swift`: Core orchestrator evaluating 16KB micro-samples and directing pipelines (Store downgrade, Special-Value bypass, Bit-Grooming).
- `[MODIFY]` `Sources/TTZipCore/TemplateMethod/BaseArchiveEngineTemplate.swift`: Inject transparent adaptive check into workflow skeleton.
- `[MODIFY]` `Sources/TTZipCore/ArchiveWriter+Dispatch.swift`: Hook adaptive probe to transparently bypass compression for high-entropy inputs.
- `[NEW]` `Sources/TTZipCore/Benchmark/MultiModalDatasetGenerator.swift`: Zero-heap streaming generator for Float32, High-Entropy, Sparse, and JSON datasets.
- `[MODIFY]` `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner+Setup.swift`: Wire multi-modal datasets into PK evaluation.

### Test Suite (`Tests/TTZipTests/`)
- `[NEW]` `Tests/TTZipTests/TransparentAdaptivePipelineTests.swift`
- `[NEW]` `Tests/TTZipTests/CompetitorMultiModalBenchmarkTests.swift`
