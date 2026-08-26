# Phase 0 Research: ttzip-cli Standalone Release & Quality Hardening

## R001: macOS Universal Binary Compilation & Release Packaging
- **Decision**: Build release binaries targeting both `arm64` and `x86_64` architectures using `swift build -c release`, merge with `lipo -create` if necessary, strip unneeded symbols via `strip -x`, and package into `ttzip-cli-v1.0.0-macos-universal.tar.gz` with computed SHA256 hashes.
- **Rationale**: Guarantees zero-friction execution across both modern Apple Silicon (M1/M2/M3/M4/M5) and legacy Intel Mac systems without Rosetta 2 translation overhead.
- **Alternatives Considered**: Distributing only an `arm64` slice. (Rejected because standard open-source CLI distributions on macOS should support x86_64 for broad developer compatibility).
- **Source**: [Apple Developer Universal Binary Guide](https://developer.apple.com/documentation/apple-silicon/building-a-universal-macos-binary)

---

## R002: Homebrew Tap & Formula Distribution Standard
- **Decision**: Create a dedicated formula file `Formula/ttzip.rb` supporting `brew install wittkung/tap/ttzip`, referencing the GitHub Release universal tarball and SHA256 checksum.
- **Rationale**: Homebrew is the de facto package manager on macOS; providing a tap allows any developer to install and update `ttzip` in one command: `brew install wittkung/tap/ttzip`.
- **Alternatives Considered**: Relying solely on `curl | sh` scripts. (Rejected due to security hygiene and lack of clean update management).
- **Source**: [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)

---

## R003: POSIX Exit Codes & Machine-Readable Output Separation
- **Decision**: Implement strict Unix exit status conventions:
  - `0`: Success
  - `1`: Syntax / Argument Error
  - `2`: File I/O Error
  - `3`: Checksum / Integrity Verification Error
  - `4`: Password / Decryption Authentication Failure
  - `5`: Security Interception (e.g. Zip Slip, Zip Bomb)
  When `--json` is specified, all structured JSON payloads are written strictly to `stdout`, while diagnostic and progress logs are redirected to `stderr`.
- **Rationale**: Allows clean composition with shell tools (`ttzip-cli inspect test.zip --json | jq .files`).
- **Alternatives Considered**: Interleaving progress messages with JSON on `stdout`. (Rejected because it breaks JSON stream parsers).
- **Source**: IEEE Std 1003.1-2017 (POSIX.1)
