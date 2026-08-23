# Technical Research: 072-cli-packaging-homebrew-gui-integration

## R001: Universal 2 Release Packaging, Symbol Stripping, and Homebrew Formula Architecture

- **Decision**: 
  1. **Dual-Strategy Compilation in `scripts/package_cli_release.sh`**:
     - Default to unified SPM multi-architecture compilation: `swift build -c release --arch arm64 --arch x86_64 --product ttzip-cli`.
     - Automatic fallback to individual architecture slices (`--arch arm64` and `--arch x86_64`) fused via `lipo -create -output <dist_bin> <arm64_bin> <x86_64_bin>`.
  2. **Two-Step Debug Extraction and Symbol Stripping**:
     - Step 1: `dsymutil "${UNSTRIPPED_BIN}" -o "${DIST_DIR}/ttzip-cli.dSYM"` before symbol stripping.
     - Step 2: `strip -x "${DIST_DIR}/bin/ttzip-cli"` to remove local/debug symbols while preserving dyld exports and dynamic relocations.
  3. **Standard UNIX Hierarchy & Homebrew Formula**:
     - Assemble `bin/ttzip-cli`, `share/man/man1/ttzip-cli.1` (via `ttzip-cli man`), `share/zsh/site-functions/_ttzip-cli` (via `ttzip-cli completion zsh`), `share/bash-completion/completions/ttzip-cli` (via `ttzip-cli completion bash`), `share/fish/vendor_completions.d/ttzip-cli.fish` (via `ttzip-cli completion fish`).
     - Generate `Formula/ttzip-cli.rb` with `bin.install`, `man1.install`, `zsh_completion.install`, `bash_completion.install`, and `fish_completion.install`.
  4. **Clean macOS Tarball Creation**:
     - Use `COPYFILE_DISABLE=1 tar --no-mac-metadata --no-xattrs -czf` to ensure 0 `._*` AppleDouble pollution.

- **Rationale**:
  - `strip -x` shrinks the binary by 40–70% without altering machine code, entry points, or C static library linkages.
  - `COPYFILE_DISABLE=1` and `--no-mac-metadata` guarantee bit-exact cross-platform portability without hidden metadata.
  - Generating man page and completions directly from the compiled binary guarantees 0 maintenance drift between code, documentation, and completion scripts.

- **Alternatives Considered**:
  - *Unflagged `strip` or `strip -s`*: Rejected because it strips dyld export entries, risking startup crashes on macOS.
  - *Source build Homebrew formula*: Rejected for binary tap distribution because compiling Swift Package + C static libraries from source requires full developer toolchains on end-user machines.

- **Source**:
  - Apple Developer `man strip(1)`, `man dsymutil(1)`, `man copyfile(3)`
  - Homebrew Formula Cookbook (`brew.sh/docs/Formula-Cookbook`)
  - Project build scripts: `Package.swift`, `scripts/run_local_ci_gate.sh`

---

## R002: Desktop GUI Diagnostics & Standards Integration in TTZipApp

- **Decision**:
  1. **ViewModel Architecture**:
     - Implement `@MainActor`-bound `ArchiveInspectorViewModel: ObservableObject` under `Sources/TTZipApp/ViewModels/`.
     - Execute diagnostic scans in `Task.detached(priority: .userInitiated)` with cancellation and in-memory cache keyed by `(path, size, mtime)`.
     - Add `OverlayState.showArchiveInspectorModal` and `OverlayState.inspectingArchivePath` in `AppSubStates.swift`.
  2. **Multi-Tab Inspector Presentation (`ArchiveInspectorSheet`)**:
     - Tab 1: **Standard Specifications (标准规范)**: RFC/ISO/POSIX citations, Apple UTI, MIME type.
     - Tab 2: **Magic Signatures & Anchors (幻数锚点)**: Anchor offsets, matching status, byte patterns.
     - Tab 3: **ZIP Extra Fields (扩展字段)**: 0x5455 UTC timestamps, 0x0001 Zip64, 0x9901 AES, 0x7875 Info-ZIP.
     - Tab 4: **Compliance & Health Check (合规体检)**: Pass/warning/violation badges from `StandardsComplianceChecker`.
  3. **Access Points**:
     - `RightInspectorSidePanel` quick diagnostic badge and "Detailed Inspection" button.
     - `MillerColumnItemRowView` right-click context menu ("Inspect Archive Standards...").
     - AppKit Main Menu (`Tools` -> `Archive Inspector`).

- **Rationale**:
  - Running diagnostics on detached background tasks with thread-safe caching guarantees 0 UI thread freezes on multi-gigabyte files.
  - Direct in-process reuse of `TTZipCore/Standards/` ensures 100% functional parity between CLI diagnostic commands and GUI views.

- **Alternatives Considered**:
  - *Synchronous parsing on view mount*: Rejected because reading large archives directly on `@MainActor` causes UI stutters.
  - *Sidebar-only display*: Rejected because full RFC citations, hex dumps, and violations require a dedicated modal sheet for legibility.

- **Source**:
  - `Sources/TTZipApp/ViewModels/AppViewState.swift`, `AppSubStates.swift`
  - `Sources/TTZipApp/Views/Components/RightInspectorSidePanel.swift`
  - `Sources/TTZipCore/Standards/ArchiveFormatStandardSpec.swift`
  - `Sources/TTZipCore/Standards/StandardsComplianceChecker.swift`
