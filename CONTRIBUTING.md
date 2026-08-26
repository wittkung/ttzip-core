# Contributing to TTZip

Thank you for your interest in contributing to **TTZip**! We welcome contributions from systems programmers, performance engineers, and macOS developers passionate about high-performance archiving, SIMD compression acceleration, and native software craftsmanship on Apple Silicon and Intel hardware.

---

## 1. Code of Conduct & Philosophy

- **Extreme Engineering Rigor**: TTZip is built on 100% in-process C static library bindings with zero external CLI subprocess spawning. Every line on hot paths must adhere to zero-heap allocation in tight loops and lock-free concurrency.
- **Architectural Symmetry & Design Patterns**: We value clean, decoupled abstractions using established design patterns (Bridge, Strategy, Factory, Template Method, and Flyweight) in orchestration layers, while strictly isolating data planes and hot loops from pattern overhead.
- **Zero Unnecessary Divergence**: When integrating or porting from upstream reference implementations (such as `libarchive`, `libdeflate`, `zstd`, or `LZMA SDK`), code, comments, and conventions must closely mirror upstream references to minimize cognitive overhead for reviewers.
- **Community Standards & Humility**: We adhere strictly to the [Contributor Covenant v2.1](CODE_OF_CONDUCT.md). We communicate with clarity, empirical data, and technical precision.

---

## 2. Development & Toolchain Requirements

- **Operating System**: macOS Sonoma 14.0+ or macOS Sequoia 15.0+ (Apple Silicon M1/M2/M3/M4/M5 recommended, Intel x86_64 supported).
- **Languages & Compilers**:
  - Swift 6.0 (`swift-tools-version: 6.0`) with Strict Concurrency Checking (`.enableUpcomingFeature("StrictConcurrency")`).
  - Rust 1.80+ (Stable toolchain with `cargo`).
  - C11 / C++20 standard with Clang / LLVM (for C-ABI and native codecs).
- **Dependencies**: Rust microkernel (`core/rust/ttzip-engine`), static C-ABI bridge (`CTTZipBridge`), binary XCFramework (`Vendor/TTZipVendor.xcframework`), and shared localization (`TTLocalizationKit`).

---

## 3. Git Branching Strategy & Commit Conventions

TTZip enforces a disciplined Git branching model and commit message standard. For the complete specification, refer to [**`docs/governance/BRANCHING_STRATEGY.md`**](docs/governance/BRANCHING_STRATEGY.md).

### 3.1 Branching Taxonomy

All branches branch from and merge back to `main` via Pull Requests:

| Prefix | Description & Purpose | Example |
| :--- | :--- | :--- |
| `main` | Protected production branch. 100% green tests, linear history. | `main` |
| `feat/<name>` | New user-facing features, format additions, or SDK bindings. | `feat/snappy-streaming` |
| `perf/<format>-<optimization>` | Algorithmic, SIMD, and parallel throughput optimizations. | `perf/lzma2-swar-matchfinder` |
| `fix/<issue-id>-<slug>` | Bug fixes, memory leak resolutions, or security patches. | `fix/142-cbr-utf8-crash` |
| `upstream/<lib>-<patch>` | Isolated upstream patches (follows the Upstream Triplet Commit rule). | `upstream/libarchive-pmull-crc32` |
| `release/v<version>` | Release stabilization, version bumping, and appcast signing. | `release/v1.4.0` |
| `docs/<name>` | Documentation updates, architecture guides, and benchmark specs. | `docs/branching-strategy` |
| `chore/<name>` | Build configuration, `.gitignore` maintenance, and linter rules. | `chore/swiftlint-rules` |

### 3.2 Conventional Commits v1.0.0

All commit messages must follow the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) format:

```git
type(scope): subject description

[optional body with technical context]

[optional footer: Closes #123]
```

- **Types**: `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `chore`, `ci`, `build`, `revert`.
- **Allowed Scopes**:
  - *Formats*: `zip`, `7z`, `tar`, `zstd`, `lzma2`, `lz4`, `brotli`, `lzip`, `lrzip`, `wim`, `dmg`, `iso`, `snappy`, `aar`, `cab`, `rar`.
  - *Core & Crypto*: `crypto`, `bridge`, `stream`, `security`, `scanner`, `vfs`.
  - *SDK & Bindings*: `sdk`, `c`, `cpp`, `python`, `node`, `jvm`, `go`, `csharp`, `dart`.
  - *CLI & Benchmarks*: `cli`, `tui`, `bench`.
  - *Infrastructure*: `build`, `ci`, `vendor`, `deps`, `governance`.

---

## 4. Local Building, Testing & Verification Commands

```bash
# 1. Clone the repository
git clone https://github.com/wittkung/ttzip.git
cd ttzip/core

# 2. Build Rust Microkernel & Generate UniFFI Bindings
./scripts/build_rust.sh

# 3. Build Swift Core (Debug & Release)
swift build
swift build -c release

# 4. Run full Swift unit test suite (Actor concurrency, VFS, UniFFI bindings)
swift test --parallel

# 5. Run Rust Workspace tests
cd rust && cargo test --workspace && cd ..

# 6. Run full-matrix in-memory benchmark & CI gate
swift run ttzip-bench gate
swift run ttzip-bench pipeline

# 7. Run full local automated CI gate (Format, License, UniFFI symbol parity, Tests)
./scripts/run_local_ci_gate.sh
```

---

## 5. Architectural Invariants & Hot-Path Rules

TTZip enforces non-negotiable throughput floors and safety invariants across all archive formats:

1. **100% Mozilla UniFFI Standard**: All Tier-1 language bindings must be generated via Mozilla UniFFI or C-ABI 2.0 with strict memory ownership.
2. **Zero Intermediate Heap Allocation**: In tight compression/decompression loops, do not allocate dynamic tree/visitor objects or per-file `Data(count:)` buffers. Utilize page-aligned buffers, `memmap2`, and stack buffers.
3. **Lock-Free Concurrency**: Multi-core workloads utilize Rayon work-stealing thread pools balanced across host CPU cores with lock-free atomic cancellation tokens (`CancellationToken`).
4. **Hardware SIMD Acceleration**: Prioritize ARM64 NEON, PMULL, and AES hardware crypto pipelines on Apple Silicon while maintaining portable x86_64 AVX2 fallbacks.
5. **Strict Memory Scrubbing**: Sensitive cryptographic credentials in memory buffers are zero-filled immediately upon release (`zeroize` / `SecureBytes`).

---

## 6. Pull Request Verification Gates

Every Pull Request must satisfy the verification pipeline:

- **Gate A · Compilation Cleanliness**: Zero compiler warnings (`-warnings-as-errors`) across Swift and Rust crates.
- **Gate B · UniFFI Symbol Parity**: `scripts/verify_uniffi_symbols.sh` passes with 100% matching export symbols between `ttzip_engineFFI.h` and `libTTZipVendor.a`.
- **Gate C · Test Suite Matrix**: 100% pass rate in `swift test` and `cargo test --workspace`.
- **Gate D · Zero Memory Leaks & Invariants**: VFS search zero-allocation and single-entry zero-disk-IO invariant tests pass cleanly.
- **Gate E · LOC Threshold Gate**: No single source file exceeds 800 LOC (`scripts/lint_loc_gate.py`).

---

Thank you for helping keep TTZip the fastest, safest, and most refined native archiving ecosystem!
