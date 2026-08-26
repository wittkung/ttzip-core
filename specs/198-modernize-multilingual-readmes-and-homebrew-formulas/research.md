# Phase 0 Research: 198-modernize-multilingual-readmes-and-homebrew-formulas

## Research Item R001: Multilingual README Alignment
- **Decision**: 
  - Update `README.md`, `README_zh.md`, `README_ja.md`, `README_ko.md`.
  - Remove `CMake 3.20+` badge, add `Rust Cargo` & `SwiftPM` badges.
  - Update build instructions to:
    - `swift build -c release`
    - `cargo build --workspace --release`
    - `make reinstall`
    - `Install-TTZip.command`
  - Remove dead `#include <ttzip/ttzip_api.h>` snippets and present Swift 6 + Rust microkernel integration.
- **Rationale**: 
  - Prevents build failures for external open-source contributors and reflects true architecture.
- **Alternatives Considered**: 
  - *Keep old README*: Causes immediate failure for anyone cloning and running `cmake`.
- **Source**: 
  - `Package.swift`
  - `rust/Cargo.toml`
  - `Makefile`

---

## Research Item R002: Homebrew Formula Synchronization
- **Decision**: 
  - Update `scripts/package_local_release.sh` to emit both `Formula/ttzip-cli.rb` and `Formula/ttzip.rb`.
  - Ensure both point to `https://github.com/wittkung/TTZip/releases/download/v${VERSION}/ttzip-cli-v${VERSION}-darwin-${TARGET_ARCH}.tar.gz`.
- **Rationale**: 
  - `brew install wittkung/ttzip/ttzip` and `brew install wittkung/ttzip/ttzip-cli` should both resolve smoothly.
- **Alternatives Considered**: 
  - *Only keep one*: Users frequently type `brew install ttzip` instead of `ttzip-cli`.
- **Source**: 
  - `Formula/ttzip.rb`
  - `Formula/ttzip-cli.rb`
  - `scripts/package_local_release.sh`
