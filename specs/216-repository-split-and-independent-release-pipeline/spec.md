# Feature Specification: Physical Two-Repository Split & Independent Open-Source Release Pipeline

**Feature**: `216-repository-split-and-independent-release-pipeline`  
**Classification**: `[Full SDD]` (System Boundaries: Git repository topology split, independent SPM & Cargo manifests, clean dependency decoupling, and dual local CI/CD pipelines)  
**Status**: `SPECIFIED`  

---

## 1. Executive Summary & Objective

### 1.1 Context
Following the completion of Feature 215, the TTZip codebase possesses a clean internal separation between:
1. **Core Infrastructure & SDKs**: Safe Rust compression engines (`ttzip-engine`), C11 ABI (`ttzip-glue`), 19-subcommand POSIX CLI (`ttzip-cli`), and Swift 6 SDK (`TTZipCore`), dual-licensed under `BSD-3-Clause OR Apache-2.0`.
2. **Client Applications**: macOS desktop GUI (`TTZipApp`), QuickLook extension (`TTZipQuickLook`), and FinderSync extension (`TTZipFinderSync`), licensed under `GPL-3.0-or-later`.

Currently, these two domains coexist within a single monorepo. To realize universal multi-platform adoption and establish an unencumbered open-source developer ecosystem, this feature executes the **physical decoupling into two independent Git repositories**:
- **Repository A (`wittkung/ttzip-core`)**: Fully autonomous, zero-Apple-UI dependencies, cross-platform (macOS, Linux, Windows, BSD), publishable to `crates.io`, Homebrew, and Swift Package Index.
- **Repository B (`wittkung/ttzip-apple`)**: Native Apple multiplatform client application (macOS + iOS/iPadOS), consuming `ttzip-core` via SPM, with App Store and direct Sparkle distribution channels.

---

## 2. Target Repository Topology & Manifest Contracts

### 2.1 Repository A: `ttzip-core` (Infrastructure, SDK & CLI)

```text
ttzip-core/
├── LICENSE-BSD                 # BSD 3-Clause License
├── LICENSE-APACHE              # Apache License 2.0
├── NOTICE                      # Copyright & Origin Notice
├── README.md                   # Core SDK & CLI documentation
├── Package.swift               # Autonomous SPM package (zero AppKit/Sparkle dependencies)
├── Cargo.toml                  # Workspace manifest (ttzip-engine, ttzip-glue, ttzip-cli)
├── rust/
│   ├── ttzip-engine/           # Pure algorithm crate (rlib, crates.io)
│   ├── ttzip-glue/             # C11 ABI FFI export layer (staticlib, cdylib)
│   └── ttzip-cli/              # 19-command CLI binary (bin name: ttzip)
├── Sources/
│   ├── CTTZipBridge/           # C11 bridge (ttzip.h, ttzip_rust_glue.h, modulemap)
│   └── TTZipCore/              # Swift 6 SDK (AsyncThrowingStream, Sendable)
├── Tests/
│   └── TTZipCoreTests/         # Swift unit and property tests
├── Vendor/
│   └── TTZipVendor.xcframework # Pre-compiled universal binary target for SwiftPM
└── scripts/
    ├── build_rust.sh           # Rust to XCFramework builder
    ├── install_local_git_hooks.sh # Zero-cloud pre-push hook installer
    ├── lint_loc_gate.sh        # Single-file <= 800 LOC gate
    └── run_local_ci_gate.sh    # 4-stage automated local regression gate
```

### 2.2 Repository B: `ttzip-apple` (Apple Client Applications)

```text
ttzip-apple/
├── LICENSE-GPL                 # GNU General Public License v3.0
├── NOTICE                      # Trademark, Brand & App Store Protection Notice
├── README.md                   # Desktop & Mobile App documentation
├── Package.swift               # SPM consuming ttzip-core as remote dependency
├── Sources/
│   ├── TTZipApp/               # SwiftUI + AppKit desktop application
│   ├── TTZipQuickLook/         # QuickLook preview extension
│   └── TTZipFinderSync/        # Finder integration extension
├── Tests/
│   └── TTZipAppTests/          # UI state, view model, and extension tests
├── Resources/                  # Assets, AppIcon.appiconset, Kintsugi Gold theme assets
└── scripts/
    ├── create_dmg_installer.sh # Sparkle notarized DMG packager
    ├── install_local_git_hooks.sh # Local UI regression hook installer
    └── run_local_ci_gate.sh    # UI and extension verification gate
```

---

## 3. Functional Requirements

### 3.1 Repository Splitting & History Preservation
- **REQ-SPLIT-001**: A deterministic script `scripts/split_repositories.sh` MUST be implemented to automate the repository split using `git subtree` or branch isolation.
- **REQ-SPLIT-002**: Git commit history and author attribution for Witt Kung and contributors MUST be preserved across both resulting repositories.

### 3.2 Standalone `ttzip-core` Autonomy
- **REQ-CORE-001**: `ttzip-core`'s `Package.swift` MUST NOT contain any references to Sparkle, AppKit, or UI targets.
- **REQ-CORE-002**: `ttzip-core` MUST compile, test, and pass all 4 local CI stages completely independently of `ttzip-apple`.
- **REQ-CORE-003**: `ttzip-core`'s `Cargo.toml` MUST be valid for `cargo package` / `cargo publish` onto `crates.io`.

### 3.3 Standalone `ttzip-apple` Integration
- **REQ-APP-001**: `ttzip-apple`'s `Package.swift` MUST declare a package dependency on `ttzip-core` (supporting both local path during development and remote URL `https://github.com/wittkung/ttzip-core.git` for releases).
- **REQ-APP-002**: `ttzip-apple` MUST build cleanly, link against `TTZipCore`, and pass all UI and extension unit tests.

### 3.4 Local Zero-Cloud CI/CD Independence
- **REQ-CI-001**: Each repository MUST have its own self-contained `scripts/run_local_ci_gate.sh` and `scripts/install_local_git_hooks.sh` that operate with $\$0.00$ cloud runner costs.
- **REQ-CI-002**: Both repositories MUST enforce the single-file $\le 800\text{ LOC}$ ceiling independently.

---

## 4. Non-Functional Requirements & Success Criteria

| Metric | Target | Verification Method |
| :--- | :--- | :--- |
| **`ttzip-core` Build Time** | $\le 5\text{ s}$ for Swift, $\le 8\text{ s}$ for Rust | `swift test` & `cargo test` |
| **`ttzip-core` Zero-UI Coupling** | 0 UI symbols / 0 AppKit imports | Header & symbol grep audit |
| **History Retention** | 100% commit history preserved | `git log --oneline` on both repos |
| **Local CI Execution** | $\le 45\text{ s}$ per repository | `scripts/run_local_ci_gate.sh` |
| **Cloud Runner Minutes** | 0 minutes | Local pre-push hook execution |
