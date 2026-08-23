# Phase 0 Research: 190-domain-responsibility-realignment-and-core-lightweighting

## Research Item R001: Benchmark & Plotting Isolation
- **Decision**: Remove 48 benchmark plotting and corpus files from `Sources/TTZipCore/Benchmark/` and keep required models in `Sources/TTZipBench/`.
- **Rationale**: 
  - `TTZipCore` should be focused strictly on production compression/decompression. Benchmarking is a developer tool.
- **Alternatives Considered**: 
  - *Keep benchmark in core*: Increases bundle size and confuses API boundaries.
- **Source**: 
  - `Sources/TTZipCore/Benchmark/`
  - `Sources/TTZipBench/`

---

## Research Item R002: TUI & Concurrency Patterns Decommissioning
- **Decision**: Delete `Sources/TTZipCore/CLI/TUI/` (6 files) and `Sources/TTZipCore/ConcurrencyPatterns/` (20 files).
- **Rationale**: 
  - `rust/ttzip-tui/` provides a native, zero-dependency TUI binary (`bin/ttzip`).
  - Rayon and Swift 6 native concurrency provide superior safety and throughput.
- **Alternatives Considered**: 
  - *Maintain Swift TUI*: Redundant code with lower performance.
- **Source**: 
  - `rust/ttzip-tui/`
  - `Sources/TTZipCore/ConcurrencyPatterns/`
