# Phase 0 Research: 199-purge-obsolete-examples-and-hidden-build-dirs

## Research Item R001: Examples Directory Audit
- **Decision**: 
  - Delete `examples/quickstart.c` and `examples/` directory.
- **Rationale**: 
  - `quickstart.c` references non-existent `ttzip_api.h` and legacy functions from the prototype phase.
  - Modern API and CLI quickstart is documented directly in `README.md`.
- **Alternatives Considered**: 
  - *Keep it*: Causes compilation failure if an external developer attempts to build `examples/quickstart.c`.
- **Source**: 
  - `examples/quickstart.c`

---

## Research Item R002: Hidden Build Folders Audit
- **Decision**: 
  - Delete `.build_custom/`, `.build_di_test/`, `.build_tmp/`.
- **Rationale**: 
  - Leftover hidden build directories from past manual experiment runs.
- **Alternatives Considered**: 
  - *Keep them*: Unnecessary directory clutter.
- **Source**: 
  - Repository root scan.
