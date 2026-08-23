# Phase 0 Research: 188-grand-swift-test-purge-and-minimal-facade-tests

## Research Item R001: Swift Redundant Test Elimination
- **Decision**: Remove all 70+ redundant tests from `Tests/TTZipTests/` that mirror Rust integration tests.
- **Rationale**: 
  - Rust `cargo test` already executes 220+ tests covering 17 codecs, AES-GCM, RS-FEC, and VFS trees.
- **Alternatives Considered**: 
  - *Keep running both*: Slows CI and creates maintenance friction when refactoring.
- **Source**: 
  - `Tests/TTZipTests/`
  - `rust/ttzip-glue/tests/`

---

## Research Item R002: Minimal High-Level Swift Facade Tests
- **Decision**: Group all Swift public API checks into `TTZipCoreIntegrationTests.swift` covering archive create, extract, inspect, and split.
- **Rationale**: 
  - Provides 100% confidence for macOS application and command-line usage in $<0.5\text{s}$.
- **Alternatives Considered**: 
  - *No Swift tests*: Risk of regressions in Swift C-ABI bridge bindings.
- **Source**: 
  - `Sources/TTZipCore/ArchiveWriter.swift`
  - `Sources/TTZipCore/ArchiveExtractor.swift`
  - `Sources/TTZipCore/ArchiveReader.swift`
