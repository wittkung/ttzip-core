# Phase 0 Research: 187-ultimate-swift-legacy-code-purge-and-thin-facade

## Research Item R001: Independent Ultra-Thin Swift Facades
- **Decision**: Ensure `ArchiveWriter.swift`, `ArchiveExtractor.swift`, `ArchiveReader.swift`, `ArchiveRepairEngine.swift`, `ArchiveIntegrityChecker.swift`, `PasswordVaultManager.swift`, `PasswordRecoveryEngine.swift`, `SplitVolumeEngine.swift` directly invoke C-ABI methods without referencing any legacy subdirectories (`Zip/`, `SevenZip/`, `TemplateMethod/`, etc.).
- **Rationale**: 
  - Allows us to delete all legacy subdirectories safely in one atomic step without breaking public API callers.
- **Alternatives Considered**: 
  - *Gradual file-by-file deletion*: High risk of dangling cross-references and broken intermediate states.
- **Source**: 
  - `Sources/TTZipCore/ArchiveWriter.swift`
  - `Sources/TTZipCore/ArchiveExtractor.swift`
  - `Sources/TTZipCore/ArchiveReader.swift`

---

## Research Item R002: Test Alignment & Swift Test Streamlining
- **Decision**: Delete Swift test files in `Tests/TTZipTests/` that test internal classes from the removed directories (e.g. `TemplateMethodPatternTests`, `StatePatternTests`, `MediatorPatternTests`, `ObserverPatternTests`, `ChainOfResponsibilityTests`).
- **Rationale**: 
  - These tests verify internal Swift scaffolding classes that are being purged; the underlying logic is 100% verified in `rust/ttzip-glue/tests/`.
- **Alternatives Considered**: 
  - *Refactoring test mocks in Swift*: Wasted effort for obsolete internal scaffolding.
- **Source**: 
  - `Tests/TTZipTests/`
