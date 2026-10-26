# Implementation Plan: 075 Mac App Store Release Sprint

## 1. Technical Context & Current Status

- **Status**: `ad-hoc 渠道包及本地产物一致性检查完成`（正式签名、独立签名包集成、公证和商店验收仍未完成）。
- The Bazel release packaging and verification pipeline enforces:
  1. Deterministic 3-channel (`mas`, `direct`, `steam`) packaging via `products/ttzip/apple/scripts/build_bazel_channels.sh` with per-channel `.zip` archives, `source_snapshot` (`git_head_commit`, `git_diff_head_sha256`, `dirty_files_sha256`), and `signing` metadata.
  2. Pre-notarization manifest & SHA-256 verification via `products/ttzip/apple/scripts/bazel_notarize_dmg.sh`.
  3. Public API cross-process file access (`QuarantineWorkerClient` -> `ttarchive-worker`) using Foundation/CoreFoundation implicit security-scoped URL bookmarks (`minimalBookmark` / `CFURLCreateByResolvingBookmarkData` + `CFURLStartAccessingSecurityScopedResource`) alongside `--archive-fd 4` and `--target-dir-fd 5` (`DirJail`), with zero private `sandbox_extension_*` APIs in production code.
  4. Checked-in sandbox verification tools:
     - `products/ttzip/apple/scripts/verify_synthetic_sandbox_extension.py`: Synthetic sandbox extension authorization diagnostic test (`合成 sandbox extension 授权测试`).
     - `products/ttzip/apple/scripts/verify_nsopenpanel_sandbox_host.py` & `products/ttzip/apple/Tests/SandboxPowerboxHost/`: Real App-Sandboxed host driving `NSOpenPanel` external archive & destination selection through production `QuarantineWorkerClient`.
  5. Build-closure-driven license harvesting (`products/ttzip/core/scripts/generate_acknowledgements.py`) querying actual channel build dependencies and reading full upstream `LICENSE`/`COPYING`/`NOTICE` files without fallback placeholders.

## 2. Component Changes

- `products/ttzip/core/Sources/TTZipCore/Pipeline/QuarantineWorkerClient.swift` [MODIFY]
- `products/ttzip/workers/ttarchive-worker/src/main.rs` & `src/bookmark_macos.rs` [MODIFY/NEW]
- `products/ttzip/workers/ttarchive-worker/BUILD.bazel` & `Cargo.toml` [MODIFY]
- `products/ttzip/apple/scripts/verify_synthetic_sandbox_extension.py` [NEW]
- `products/ttzip/apple/scripts/verify_nsopenpanel_sandbox_host.py` & `products/ttzip/apple/Tests/SandboxPowerboxHost/` [NEW]
- `products/ttzip/core/scripts/generate_acknowledgements.py` [MODIFY]
- `products/ttzip/apple/ACKNOWLEDGEMENTS.md` & `Sources/TTZipApp/Resources/Acknowledgements.plist` [MODIFY]

