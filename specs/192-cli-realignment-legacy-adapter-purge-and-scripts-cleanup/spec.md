# Feature Specification: 192-cli-realignment-legacy-adapter-purge-and-scripts-cleanup

## 1. Executive Summary & Strategic Motivation
A systematic codebase audit discovered 4 domain misplacements and legacy accumulations:
1. **CLI Logic Misplaced in Core**: 20 files in `Sources/TTZipCore/CLI/` belong in `Sources/TTZipCLI/`.
2. **Obsolete C Adapters**: 9 files in `Sources/TTZipCore/Adapters/` are legacy artifacts from deleted C libraries.
3. **Over-Engineered Design Pattern Boilerplate**: 11 files in `Sources/TTZipCore/Proxies/` and `Sources/TTZipCore/RepositoryPattern/` add unnecessary indirection layers.
4. **Redundant Scripts**: 15 duplicate/obsolete scripts in `scripts/` cause confusion and maintenance burden.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Truly Headless TTZipCore
- **Given** integrating `TTZipCore` in headless or GUI environments
- **When** importing the framework
- **Then** zero CLI parsing, man page generation, or legacy C adapter classes are present.

### User Scenario 2: Consolidated Tooling Scripts
- **Given** running CI, building, or packaging
- **When** inspecting `scripts/`
- **Then** exactly one single-source-of-truth script exists for each operational responsibility (CI, Rust build, release packaging, standard linting).

---

## 3. Success Metrics
1. **Domain Realignment**: Move 20 CLI files to `Sources/TTZipCLI/`.
2. **Codebase Purge**: Delete 20 legacy Swift files (`Adapters/`, `Proxies/`, `RepositoryPattern/`) and 15 redundant scripts.
3. **Zero Regression**: 100% pass rate on `cargo test`, `swift test`, and `./scripts/run_local_ci_gate.sh`.
