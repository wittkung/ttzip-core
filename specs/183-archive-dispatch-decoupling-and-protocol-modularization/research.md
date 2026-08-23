# Phase 0 Research: 183-archive-dispatch-decoupling-and-protocol-modularization

## Research Item R001: Archive Dispatch Decoupling
- **Decision**: Split `ArchiveEngineBridge.swift` into `ArchiveEngineBridge.swift` (core registry & life-cycle) and `ArchiveEngineBridge+Formats.swift` (format routing), and split `ArchiveWriter+Dispatch.swift` into `ArchiveWriter+ZipDispatch.swift` and `ArchiveWriter+TarSevenZipDispatch.swift`.
- **Rationale**: 
  - Separates archive configuration validation from lower-level C-ABI dispatching, keeping all files below 250 LOC.
- **Alternatives Considered**: 
  - *Keep monolithic file*: Breaches codebase SRP guidelines.
- **Source**: 
  - `Sources/TTZipCore/Bridge/ArchiveEngineBridge.swift`
  - `Sources/TTZipCore/ArchiveWriter+Dispatch.swift`

---

## Research Item R002: Protocols Segregation
- **Decision**: Decompose `CompressionStrategyProtocol.swift`, `ArchiveComponentProtocol.swift`, and `ArchiveProtocols.swift` into focused role interfaces.
- **Rationale**: 
  - Adheres to Interface Segregation Principle (ISP) and keeps file lengths well below 350 LOC.
- **Alternatives Considered**: 
  - *One mega-file with all protocols*: Difficult to navigate and maintain.
- **Source**: 
  - `Sources/TTZipCore/Strategies/CompressionStrategyProtocol.swift`
  - `Sources/TTZipCore/ArchiveComponentProtocol.swift`
  - `Sources/TTZipCore/ArchiveProtocols.swift`
