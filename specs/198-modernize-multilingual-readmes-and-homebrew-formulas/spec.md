# Feature Specification: 198-modernize-multilingual-readmes-and-homebrew-formulas

## 1. Executive Summary & Strategic Motivation
1. Delete redundant root file `安装TTZip.command` (keep only `Install-TTZip.command`).
2. Synchronize Homebrew Formula generation in `scripts/package_local_release.sh` so both `Formula/ttzip-cli.rb` and `Formula/ttzip.rb` are generated with the valid `ttzip-cli-v${VERSION}-darwin-${TARGET_ARCH}.tar.gz` download URL and accurate SHA256 hashes.
3. Modernize all 4 multilingual READMEs (`README.md`, `README_zh.md`, `README_ja.md`, `README_ko.md`):
   - Replace CMake badges with Swift 6 and Rust Cargo badges.
   - Replace outdated CMake instructions with modern SwiftPM & Rust Cargo build commands (`swift build -c release`, `./scripts/build_rust.sh`, `make reinstall`).
   - Remove dead `#include <ttzip/ttzip_api.h>` snippets and align architecture to Swift 6 + Safe Rust microkernel.
4. Pass all 4 stages of local CI gate in $< 10\text{s}$.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Authentic Multilingual Documentation
- **Given** an external developer reading any of the 4 README files
- **When** following the build instructions
- **Then** `swift build -c release` and `make reinstall` work out of the box with zero missing CMake files.

### User Scenario 2: Reliable Homebrew Installation
- **Given** a user running `brew install wittkung/ttzip/ttzip` or `brew install wittkung/ttzip/ttzip-cli`
- **When** Homebrew fetches the formula
- **Then** the release URL matches `ttzip-cli-v*-darwin-*.tar.gz` and installs correctly.

---

## 3. Success Metrics
1. Delete `安装TTZip.command`.
2. Update `Formula/ttzip.rb` & `Formula/ttzip-cli.rb` and `scripts/package_local_release.sh`.
3. Modernize `README.md`, `README_zh.md`, `README_ja.md`, `README_ko.md`.
4. Ensure all files $\le 800\text{ LOC}$.
5. Pass local CI/CD automated gate.
