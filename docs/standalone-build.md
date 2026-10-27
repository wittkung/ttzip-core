# Standalone Build Guide (`ttzip-core`)

This guide explains how to build and verify `ttzip-core` (both the Rust microkernel workspace and the Swift `TTZipCore` package) from a clean checkout without requiring the outer monorepo or Bazel.

## 1. Prerequisites (macOS 14.0+)

- **Rust Toolchain**: `rustc` / `cargo` (1.82+ recommended)
- **Xcode / Swift Toolchain**: Swift 6.0+ (`xcode-select --install` or full Xcode 16+)
- **System Libraries** (via Homebrew or pkg-config):
  ```bash
  brew install libarchive libdeflate lz4 zstd xz
  ```
  *(Note: `fast-lzma2` and `lzfse` C sources and `llama.cpp` C headers are vendored under `third_party/` and compiled automatically by `build.rs`.)*

## 2. Building and Testing the Rust Workspace

From the repository root:

```bash
# Verify dependency graph and workspace metadata
cargo metadata --manifest-path rust/Cargo.toml --format-version 1

# Run the single-file <= 800 LOC architecture gate
bash scripts/lint_loc_gate.sh

# Build and run the Rust workspace test suite
cargo test --workspace --manifest-path rust/Cargo.toml

# Run Clippy lints across the workspace
cargo clippy --workspace --manifest-path rust/Cargo.toml
```

### Optional Real GGUF Semantic Model Tier

By default, `ttzip-semantic` tests verify quantization math (`digup_reference_vectors_v1.json`), C-ABI layout offsets (`b11461`), persistent `.ttvec` index integrity, `DeterministicEmbeddingProvider` control flow, and honest lexical degradation (`CheckpointStatus::Blocked` / `DegradedLexicalOnly`) when the external 309 MB `embeddinggemma-2-Q8_0.gguf` model and `libllamacpp.dylib` runtime are not installed on the machine.

To enforce the real GGUF inference tier in an environment where the model and runtime dylib are installed, set:

```bash
TTZIP_REQUIRE_REAL_GGUF=1 cargo test --manifest-path rust/Cargo.toml -p ttzip-semantic
```

## 3. Generating `TTZipVendor.xcframework` & UniFFI Swift Bindings

Before running `swift build` or `swift test`, build the static library (`libTTZipVendor.a`), `TTZipVendor.xcframework`, and Mozilla UniFFI Swift bindings using `scripts/build_rust.sh`:

```bash
# Fast debug slice for local development and testing
bash scripts/build_rust.sh --debug

# Or optimized release slice (or --universal for arm64 + x86_64)
bash scripts/build_rust.sh --release
```

Also build the `ttzip` CLI binary if running `CLICommandE2ETests`:

```bash
cargo build --manifest-path rust/Cargo.toml -p ttzip-tui --bin ttzip
```

## 4. Building and Testing the Swift Package (`TTZipCore`)

Once `Frameworks/TTZipVendor.xcframework` is generated:

```bash
swift build
swift test
```
