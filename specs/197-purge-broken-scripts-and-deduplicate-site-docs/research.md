# Phase 0 Research: 197-purge-broken-scripts-and-deduplicate-site-docs

## Research Item R001: Script Suite Health Audit
- **Decision**: 
  - Delete `scripts/run_delta_audit.sh`.
- **Rationale**: 
  - `run_delta_audit.sh` calls `swift run ttzip-bench delta "$@"`.
  - In Feature 191, `ttzip-bench` was refactored into a streamlined 105 LOC binary exposing `matrix`, `gate`, `plot`.
  - Statistical delta reporting is handled by `scripts/statistical_delta.py` and `rust/ttzip-glue/src/benchmark/delta.rs`.
- **Alternatives Considered**: 
  - *Keep the script*: Creates broken user experience (exit code 64).
- **Source**: 
  - `scripts/run_delta_audit.sh`
  - `Sources/TTZipBench/main.swift`

---

## Research Item R002: Static Site File Deduplication
- **Decision**: 
  - Delete `site/` folder (8 files).
  - Delete loose duplicate files in `docs/`: `docs/index.html`, `docs/privacy.html`, `docs/terms.html`, `docs/CNAME`, `docs/appcast.xml`.
  - Preserve root files (`index.html`, `cli.html`, `formats.html`, `performance.html`, `privacy.html`, `terms.html`, `CNAME`, `appcast.xml`) as official single source of truth.
- **Rationale**: 
  - `site/` files are byte-for-byte identical duplicates of root files.
  - `docs/` contains partial duplicate files that risk drift.
- **Alternatives Considered**: 
  - *Keep 3 copies*: Violates Single Source of Truth (SSOT) and causes sync errors.
- **Source**: 
  - Python `filecmp.cmp` audit output.
