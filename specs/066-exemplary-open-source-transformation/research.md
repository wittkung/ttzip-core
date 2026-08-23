# Phase 0 Research: Exemplary Open-Source Project Transformation

## R001: Open-Source Community Health & Governance Standards
- **Decision**: Adopt the standard GitHub Community Health suite consisting of `SECURITY.md`, `CODE_OF_CONDUCT.md` (Contributor Covenant v2.1), `CONTRIBUTING.md`, `LICENSE` (MIT), and `ACKNOWLEDGEMENTS.md`.
- **Rationale**: Demonstrates institutional maturity, provides clear security disclosure channels, and aligns with Linux Foundation / OpenSSF best practices.
- **Alternatives Considered**: Keeping only a minimal README + LICENSE. (Rejected because top-tier systems projects require structured contribution workflows and clear security handling).
- **Source**: [GitHub Community Standards Guidelines](https://docs.github.com/en/communities/setting-up-your-project-for-healthy-contributions)

---

## R002: GitHub Actions CI Matrix & Multi-Channel Verification
- **Decision**: Configure `.github/workflows/ci-cd.yml` on `macos-14` runners to execute:
  1. Swift 6.0 compilation with complete concurrency checking (`-strict-concurrency=complete`);
  2. Full unit test regression suite (`swift test`);
  3. Direct distribution build (`swift build -c release`);
  4. MAS App Sandbox build (`swift build -c release -Xswiftc -DMAS_BUILD`);
  5. Static linting via SwiftLint.
- **Rationale**: Prevents accidental sandbox breaches or data-race violations on any commit or PR.
- **Alternatives Considered**: Running CI only on push to main without PR gating. (Rejected because PRs must pass before merge).
- **Source**: `.github/workflows/ci-cd.yml`

---

## R003: Modular Documentation Architecture (`docs/`)
- **Decision**: Establish clean subdirectories under `docs/`:
  - `docs/architecture/`: Detailed data-flow and in-process C-bridge design.
  - `docs/benchmarks/`: Physical benchmark methodologies, reproducibility guides, and JSON metrics.
  - `docs/formats/`: Deep specifications for all 16 supported archive formats.
- **Rationale**: Separates quick discovery in `README.md` from deep technical dives for systems engineers and researchers.
- **Alternatives Considered**: Monolithic README with all technical specs. (Rejected because it causes reader fatigue).
- **Source**: Industry standard documentation layout (e.g. `zstd/doc`, `ripgrep/doc`).
