# Research & Technical Decisions: Workspace Deduplication & Infrastructure Realignment

## Decision 1: Relative Path Structure to TTKit Infra
- **Decision**: Update Cargo.toml and Package.swift relative paths from `ttkit-localization` to `../../infra/ttkit` (or `../../../../infra/ttkit/tt-i18n-core`).
- **Rationale**: Keeps local development working out-of-the-box when repositories are checked out under the user's standard `dev/` directory tree (`dev/infra/ttkit` vs `dev/products/ttzip`), without polluting the product repository with infra source code.
- **Alternatives Considered**:
  - Git submodule: Overhead in local rapid prototyping across infra and products.
  - Remote git repository URLs: Requires committing and pushing to infra repo before testing local changes in product apps.

## Decision 2: Zero-Loss Unique Asset Extraction
- **Decision**: Inspect all items existing only in root or differing from `core/` and move/copy them before deletion:
  - `Tests/ci` -> `core/Tests/ci`
  - `Tests/cross_language` -> `core/Tests/cross_language`
  - `scripts/lint_repo_hygiene.sh` -> `core/scripts/lint_repo_hygiene.sh`
  - `specs/001-*` .. `021-*` -> `core/specs/`
  - `metadata/` -> `apple/metadata/`
- **Rationale**: Guarantees zero regression and retains test harnesses and store metadata.

## Decision 3: Root Workspace Layout
- **Decision**: The root directory `/Users/kevintung/Documents/dev/products/ttzip` becomes a clean container for the multi-repo workspace containing `core/`, `apple/`, `homebrew/`, `upstream/`, `memory/`, `.agents/`, and a top-level `README.md`.
- **Rationale**: Aligns directly with Spec 216 two-repo split architecture and eliminates developer confusion.
