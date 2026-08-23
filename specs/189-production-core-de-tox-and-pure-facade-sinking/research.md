# Phase 0 Research: 189-production-core-de-tox-and-pure-facade-sinking

## Research Item R001: Filter DSL Direct Delegation to Rust
- **Decision**: Remove Swift hand-written AST lexer/parser in `Sources/TTZipCore/InterpreterPattern/` and route `ArchiveFilter.evaluate` directly to `ttzip_rust_eval_filter_dsl`.
- **Rationale**: 
  - `rust/ttzip-glue/src/fs/filter_dsl.rs` already supports all operators (`==`, `!=`, `<`, `>`, `AND`, `OR`, `NOT`, glob matching) in $<10\mu\text{s}$ with zero allocations.
- **Alternatives Considered**: 
  - *Keep Swift parser alongside Rust*: Redundant double maintenance.
- **Source**: 
  - `rust/ttzip-glue/src/fs/filter_dsl.rs`
  - `Sources/CTTZipBridge/include/ttzip_rust_glue.h`

---

## Research Item R002: Production Code Purity & Testing Decoupling
- **Decision**: Purge `Sources/TTZipCore/Testing/` (17 files) and `Sources/TTZipCore/Mocks/` (1 file).
- **Rationale**: 
  - Benchmark runners in `Sources/TTZipBench` and differential tests in `rust/ttzip-glue/src/testing/` already contain all necessary test logic.
- **Alternatives Considered**: 
  - *Keep testing in Core*: Pollutes the public header namespace and wastes binary size.
- **Source**: 
  - `Sources/TTZipCore/Testing/`
  - `rust/ttzip-glue/src/testing/`
