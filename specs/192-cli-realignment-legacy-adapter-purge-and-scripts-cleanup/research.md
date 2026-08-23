# Phase 0 Research: 192-cli-realignment-legacy-adapter-purge-and-scripts-cleanup

## Research Item R001: CLI Tooling Isolation to TTZipCLI Target
- **Decision**: Move all 20 CLI files from `Sources/TTZipCore/CLI/` into `Sources/TTZipCLI/` subdirectories.
- **Rationale**: 
  - `TTZipCore` should have zero CLI dependencies (parsers, man pages, shell completion).
- **Alternatives Considered**: 
  - *Keep in Core*: Leaks CLI concepts into headless framework consumers.
- **Source**: 
  - `Sources/TTZipCore/CLI/`
  - `Sources/TTZipCLI/`

---

## Research Item R002: Script Consolidation & Dead C-Adapter Removal
- **Decision**: 
  - Delete `Sources/TTZipCore/Adapters/` (9 files), `Proxies/` (4 files), `RepositoryPattern/` (7 files).
  - Delete 15 redundant/obsolete scripts in `scripts/`.
- **Rationale**: 
  - Simplifies repository structure, reduces build times, and eliminates dead code.
- **Alternatives Considered**: 
  - *Keep legacy scripts*: Leads to developer confusion about which CI script to run.
- **Source**: 
  - `scripts/`
  - `Sources/TTZipCore/Adapters/`
