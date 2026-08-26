# Phase 0 Research: 186-grand-test-suite-rust-migration-and-swift-deduplication

## Research Item R001: Swift Redundant Test Inventory & Purge Strategy
- **Decision**: Purge the 14 pattern tests (`AdapterPatternTests`, `BridgePatternTests`, `CompositePatternTests`, `DecoratorPatternTests`, `FacadePatternTests`, `FlyweightPatternTests`, `InterpreterPatternTests`, `IteratorPatternTests`, `ProxyPatternTests`, `ReadWriteLockPatternTests`, `StrategyPatternTests`, `TemplateMethodPatternTests`, `VisitorPatternTests`, `WorkerPoolPatternTests`) and low-level memory/stream buffer tests.
- **Rationale**: 
  - These tests verify internal Swift scaffolding classes that are already covered by Rust FFI tests.
- **Alternatives Considered**: 
  - *Keep duplicate tests in Swift*: Wastes CI runtime and slows development iteration without adding test coverage.
- **Source**: 
  - `Tests/TTZipTests/*PatternTests.swift`
  - `rust/ttzip-glue/tests/`

---

## Research Item R002: Rust Integration Test Expansion
- **Decision**: Consolidate and expand `rust/ttzip-glue/tests/` with all container roundtrips, multi-volume spanning, encryption invariants, and differential oracle verifications.
- **Rationale**: 
  - Enables full regression testing in Linux and Windows CI environments with `cargo test`.
- **Alternatives Considered**: 
  - *Rely only on Swift tests*: Prevents cross-platform CI from running without Swift toolchain.
- **Source**: 
  - `rust/ttzip-glue/tests/`
