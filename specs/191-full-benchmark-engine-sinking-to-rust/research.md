# Phase 0 Research: 191-full-benchmark-engine-sinking-to-rust

## Research Item R001: Rust Native Multi-Codec Matrix & Corpus Runner
- **Decision**: Implement `rust/ttzip-glue/src/benchmark/runner.rs` executing multi-level compression across Libdeflate, Zstd, LZ4, LZFSE, Snappy, Brotli, and Bzip2 with Rayon work-stealing and nanosecond monotonic clocking.
- **Rationale**: 
  - Rust micro-benchmarking avoids Swift ARC and bridging overhead, providing pure hardware-level measurements.
- **Alternatives Considered**: 
  - *Keep Swift runners*: Inconsistent timing across platforms and duplicate codec invocation logic.
- **Source**: 
  - `rust/ttzip-glue/src/compression/`
  - `rust/ttzip-glue/src/benchmark/clock.rs`

---

## Research Item R002: Fritsch-Carlson Spline & SVG/HTML Dashboard in Rust
- **Decision**: Implement `rust/ttzip-glue/src/benchmark/plotter.rs` with monotone cubic Hermite interpolation and SVG/HTML templating.
- **Rationale**: 
  - Standalone binary output without external dependencies; allows generating reports directly on Linux/Windows headless servers.
- **Alternatives Considered**: 
  - *Keep SVG plotting in Swift*: Non-portable to Linux/Windows CI without Swift runtime.
- **Source**: 
  - `Sources/TTZipBench/Pareto/SVGParetoPlotter.swift`
  - `Sources/TTZipBench/Pareto/HTMLParetoDashboardGenerator.swift`
