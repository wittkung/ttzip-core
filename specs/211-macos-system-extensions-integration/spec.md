# Spec: macOS System Extensions Integration (QuickLook & FinderSync)

**Feature**: `211-macos-system-extensions-integration`  
**Classification**: `[Full SDD]` (Introduces macOS App Extension targets, IPC contracts, and QuickLook/FinderSync system integration)  
**Status**: `IN_PROGRESS`  

---

## 1. Objectives & User Experience

This feature integrates macOS system-level App Extensions into the TTZip ecosystem:

1. **QuickLook Preview App Extension (`TTZipQuickLook.appex`)**:
   - Allows users to press the **Spacebar** in Finder on any of the 16 supported archive formats (`.zip`, `.7z`, `.tar`, `.gz`, `.bz2`, `.xz`, `.zst`, `.lz4`, `.aar`, `.wim`, `.dmg`, `.iso`, `.rar`, `.cab`, etc.) to instantly view archive contents without decompressing to disk.
   - Renders a responsive, high-performance HTML/WebKit preview constructed by `QuickLookPreviewEngine`, displaying file counts, sizes, compression ratios, encryption badges, and hierarchical directory trees.

2. **Finder Sync App Extension (`TTZipFinderSync.appex`)**:
   - Integrates native context menu items into macOS Finder right-click menus via `FIFinderSync` and `FinderSyncHelper`.
   - Provides instant actions:
     - ⚡️ Extract Here (`extract_here`)
     - 📂 Extract to Subfolder (`extract_to_subfolder`)
     - 🔍 Inspect Archive (`inspect_archive`)
     - 🔑 Autofill Vault Password (`autofill_password`)
   - Dispatches requests to `TTZip.app` via registered `ttzip://` custom URL scheme (`ttzip://extract?path=...` / `ttzip://inspect?path=...`).

3. **Application Bundle Integration**:
   - `TTZipQuickLook.appex` and `TTZipFinderSync.appex` are automatically compiled, bundled into `TTZip.app/Contents/PlugIns/`, and signed with deep codesign during release packaging.

---

## 2. Invariants & Architecture Boundaries

1. **Sandboxing & Privileges**:
   - Extensions run in their respective sandboxes (`com.apple.quicklook.preview` and `com.apple.FinderSync`).
   - Communication with the host app is handled via standardized IPC/URL schemes.
2. **Speed & Zero Allocation**:
   - QuickLook preview header sniffing must complete within $< 10\text{ms}$.
   - FinderSync context menu construction must complete in $< 1\text{ms}$ with zero blocking of Finder's main thread.
3. **Single-File LOC Defense Gate**:
   - All extension files must be strictly $\le 800\text{ LOC}$.
