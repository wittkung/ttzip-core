# Standalone source build and tests

The standalone repository builds without the MetastudyLine monorepo, sibling
checkouts or private GitHub repositories. The Swift dependency is the public
`wittkung/ttlog` package at version **1.0.1**.

## macOS prerequisites

- A Rust toolchain with Cargo, and Xcode with its command-line tools selected.
- Git LFS for the Brotli RFC dictionary.
- Native libraries: libdeflate, LZ4, Zstandard and liblzma. With Homebrew:

```sh
brew install git-lfs libdeflate lz4 zstd xz
```

The Apple platform manifest declares macOS 14 and later. The validation record
identifies the actual host used; other operating systems and architectures need
their own validation.

## Build from a clean checkout

```sh
git clone https://github.com/wittkung/ttzip-core.git
cd ttzip-core
git lfs install --local
git lfs pull

# Build the Rust workspace, including the CLI used by Swift end-to-end tests.
cargo build --workspace --manifest-path rust/Cargo.toml
cargo test --workspace --manifest-path rust/Cargo.toml

# Generate the native XCFramework and matching UniFFI bindings from source.
bash scripts/build_rust.sh --debug
swift build
swift test
```

`scripts/build_rust.sh --release` generates an optimized native library instead.
Generated XCFramework binaries are local build outputs and are not committed.
The generated Swift bindings and C header must come from the same Rust library.
The script updates both together.

## Native dependency ownership

The export includes the complete Fast-LZMA2 and LZFSE source directories under
`third_party/`, including their licenses and provenance. Other native libraries
are supplied by the platform/package manager. No native sources are downloaded
implicitly during a Cargo build.

Cargo validates that both source directories exist before compiling. A missing
native dependency must fail the build rather than leave an unresolved function
to be called at runtime. `TTZIP_THIRD_PARTY_DIR` can explicitly select a source
directory for local development; a normal standalone checkout uses its own
bundled sources. The monorepo export preserves this dependency closure.

Rust executables and tests use normal symbol resolution. Python's extension
module linker arguments are restricted to its `cdylib`; Node's host integration
is similarly handled by its existing N-API build dependency.

## SDK boundary and snapshot repair

The old public snapshot retained Android device models, device-management tests
and C functions returning empty device results after their Rust implementation
had moved out of Core. Device management lives in `ttzip-apple`. Those stale Core
interfaces and their matching device-only tests are removed to match that
existing boundary. Archive, codec, crypto and VFS tests remain in Core.

The SIGSEGV regression was a 7z mutation test calling an unresolved
`FL2_createCStreamMt` function. Bundling and compiling Fast-LZMA2, correcting the
codec crate's repository root and removing global `dynamic_lookup` resolve the
missing implementation. The original regression and the complete Rust suite
are part of validation.

Some Swift tests also retained imports for Bazel-only split modules. Standalone
SwiftPM tests import the combined `TTZipCore` module and find the CLI in Cargo's
workspace target directory. `TTZIP_BIN_PATH` remains available for an explicit
CLI path.
