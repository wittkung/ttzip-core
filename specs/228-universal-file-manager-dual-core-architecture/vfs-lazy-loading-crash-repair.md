# VFS1 Crash Repair & Lazy Hierarchy Boundary Contract (`EXC_BAD_ACCESS` / `SIGBUS` on Cooperative Thread 18)

- **Owner**: VFS1
- **Status**: In-Scope Code & Verification Complete; Ready for H Snapshot Integration & Q1/P2 Acceptance
- **Incident Identifier**: `7DE8FC7D-1F71-4B98-A136-701809CFA067` (`TTZip [25731]`, macOS 27.0.1 `26A434`, ARM-64)
- **Triggering Thread**: Thread 18 (`com.apple.root.user-initiated-qos.cooperative`), `EXC_BAD_ACCESS (SIGBUS)` / `KERN_PROTECTION_FAILURE at 0x000000016d7f3fc0` (`Stack Guard 0x16d7f0000-0x16d7f4000`, 544 KB cooperative thread stack)

---

## 1. Crash Root Cause & Distinction: Design Gaps vs. Implementation Bugs

### 1.1 Crash Evidence Location
- **Original Crash Report**: Conversation transcript step 1 (`Incident Identifier: 7DE8FC7D-1F71-4B98-A136-701809CFA067`, `Launch Time: 2026-10-17 20:44:43.6268 +0800`, `Crash Time: 2026-10-17 22:00:21.8571 +0800`).
- **Faulting Stack Trace (Thread 18)**:
  - Frames `6..510`: Mutual recursion between `ArchiveTreeNode.toComponent()` (`ArchiveTreeNode+Composite.swift:41`), `Sequence.map`, and `ArchiveTreeNode.children.getter` (`ArchiveTreeNode.swift:42` -> `UniFfiVfsTree.getChildrenPaged(subpath:offset:limit:)`).
  - Frame `511`: `closure #1 in ArchiveHierarchySessionCache.getOrFetchSession(for:password:autoVaultUnlock:)` (`ArchiveHierarchySessionCache.swift:426`).

### 1.2 Design Gaps vs. Implementation Bugs

| Classification | Component | Root Cause Description | Resolution |
| :--- | :--- | :--- | :--- |
| **Design Gap 1: Eager Full-Tree Materialization on Cooperative Stack** | `ArchiveHierarchySessionCache.swift` / `ArchiveTreeNode+Composite.swift` | `getOrFetchSession` loaded the O(1) contiguous Rust `UniFfiVfsTree` arena, then immediately called `rootNode.toComponent()`, recursively paging and materializing the entire archive hierarchy into nested Swift `ArchiveCompositeDirectory` value types on a 544 KB Swift Concurrency cooperative thread stack. Furthermore, even if built iteratively, a 5,000-level nested `ArchiveCompositeDirectory` value type overflows the 544 KB cooperative stack during Swift compiler-generated value-witness `destroy` (`Dictionary<String, ArchiveCompositeDirectory>` deallocation). | `getOrFetchSession` now builds a 1-level shallow `ArchiveCompositeDirectory` (`makeShallowVfsRootComposite`) holding top-level lazy `ArchiveTreeNode` items, while `ArchiveSubpathMap` resolves `""` and all subpaths directly via `vfsTree`. `toComponent()` now uses an explicit heap worklist bounded at `maxEagerCompositeDepth = 64` and seamlessly attaches deeper directories as lazy `ArchiveTreeNode` (`ArchiveComponentProtocol`) items in `customComponents`—preventing both silent truncation and recursive destruction stack overflow. |
| **Design Gap 2: Unspecified Collision & Identity Contract after Path Normalization** | `VfsCompactArena` / `vfs_loader.rs` / R2 Extraction | When raw archive entries normalize to the same logical path (e.g., `./dup.txt` and `dup.txt`) or when a regular file shares a normalized path with a directory (`app` file vs. `app/config.json` directory), path-only lookup is ambiguous. | Defined explicit VFS collision & identity contract (Section 2): VFS preserves every entry without dropping colliding files, records 0-based `raw_entry_idx` in `arena.entry_indices`, orders directories before files with deterministic node-ID tie-breaking, and pairs with `entry_index`-based extraction/streaming. |
| **Implementation Bug 1: Root Self-Loop via `.` / `./` / `a/.` Segments** | `arena.rs`, `vfs_loader.rs`, `vfs.rs` | `trim_matches('/')` turned `"./"` into `"."` and allocated a child directory node named `"."` with `full_path: "."` under Root (ID 0). When Swift queried `getChildrenPaged(subpath: ".")`, Rust `resolve_path(".")` skipped `"."` segments and returned Root ID `0`, creating an infinite `Root -> "." -> Root` cycle. | Added canonical segment normalization across all 4 VFS construction layers (`build_from_packed`, `build_from_entries`, `UniFFIVfsTree::build`, `load_archive_vfs_tree` / `open_vfs_session`) plus defensive Swift filtering (`!name.isEmpty && name != "." && path != parent.path`). |
| **Implementation Bug 2: Silent File Drop on File-vs-Directory Collision** | `arena.rs`, `vfs_loader.rs` | `if let Some(&existing_id) = dir_map.get(clean_path) { if is_dir { ... } } else { alloc_node(...) }` silently dropped any regular file whose normalized path matched an existing directory in `dir_map` because `dir_map.get` succeeded while `is_dir` was `false`. | Guarded the `dir_map` merge branch with `if is_dir { if let Some(&existing_id) = dir_map.get(clean_path) { ...; return; } }` so regular files sharing a name with a directory are always allocated as file nodes. |
| **Implementation Bug 3: POSIX Mode Truncation (`mode as u8`)** | `vfs_loader.rs` | `let flags = (entry.mode as u8) | if entry.is_dir { VFS_FLAG_IS_DIR } else { 0 }` truncated POSIX permissions (e.g. `0o755` -> `0xED`, bit `0` = `1`), marking every executable regular file (`0o755`) as a directory (`VFS_FLAG_IS_DIR`). | Fixed `flags` construction to only set `VFS_FLAG_IS_DIR` and `VFS_FLAG_IS_ENCRYPTED` from explicit entry flags / trailing directory markers. |
| **Implementation Bug 4: Recursive `calculateArchiveComponentStats`** | `FolderMediaArtboardView+Grid.swift` | `calculateArchiveComponentStats` used a nested recursive `traverse` function on the cooperative stack. | Converted to an iterative heap-allocated worklist (`[(component: ArchiveComponentProtocol, depth: Int)]`) with `visitedPaths` cycle protection. |

---

## 2. Root Node, Path Normalization & Collision Identity Contract

### 2.1 Canonical Path Normalization Contract (All 4 VFS Layers)
Applied identically in:
1. `VfsCompactArena::build_from_packed` (`products/ttzip/core/rust/crates/ttzip-archive/src/fs/vfs/arena.rs`)
2. `VfsCompactArena::build_from_entries` (`products/ttzip/core/rust/crates/ttzip-archive/src/fs/vfs/arena.rs`)
3. `UniFFIVfsTree::build` (`products/ttzip/core/rust/ttzip-engine/src/uniffi_api/vfs.rs`)
4. `build_arena_from_archive_path` / `load_archive_vfs_tree` / `open_vfs_session` (`products/ttzip/core/rust/ttzip-engine/src/uniffi_api/vfs_loader.rs`)

Rules:
1. **Unicode Normalization**: Input paths are normalized to Unicode **NFC** (`normalize_to_nfc`), ensuring macOS HFS+/APFS NFD paths (`cafe\u{301}.txt`) match NFC paths (`café.txt`). Zero-allocation `Cow::Borrowed` fast path is used for clean ASCII/NFC POSIX paths.
2. **Separator & Segment Cleaning**: Both `/` and `\` are treated as path separators. Empty segments (`//`, leading/trailing `/`) and current-directory segments (`.`, `./`, `././`, `a/./b`, `a/.`) are stripped.
3. **Directory Inference**: An entry is treated as a directory if `raw_is_dir` is true OR the raw path ends with `/`, `\`, `/.`, `\.`, or is `.`.
4. **Root Self-Reference Prohibition**: If normalization yields zero segments (e.g. `.`, `./`, `././`, `/`), the entry refers to the archive root itself and **never** allocates a child node under Root (ID 0). Every non-root node `i > 0` satisfies `parent_ids[i] != i`, `node_name(i) != "."`, `node_name(i) != ".."`, and `node_full_path_str(i) != ""`.
5. **Traversal Rejection**: Any path containing a `..` segment (`../escape`, `a/../../escape`) is rejected (`None` during ingestion; `VfsSessionError::PathTraversalDetected` during `resolve_path`).

### 2.2 Collision & Entry Identity Contract
1. **Two Raw Entries Normalizing to the Same File Path** (e.g. `./dup.txt` at raw index 0 and `dup.txt` at raw index 1):
   - Neither entry is dropped. Both are allocated as distinct leaf nodes under the parent directory with their respective sizes, CRCs, timestamps, and 0-based `entry_indices` (`arena.entry_indices[node_id] = raw_entry_idx`).
   - In `finalize_tree`, sibling nodes with identical `(is_dir, natural_cmp(name))` are deterministically ordered by ascending `node_id` (`.then_with(|| a.cmp(&b))`), preserving archive order.
2. **File and Directory Sharing the Same Normalized Path** (e.g. directory `app/` from `app/config.json` and regular file `app`):
   - Directory deduplication in `dir_map` applies strictly to directories (`if is_dir`). The regular file `app` is allocated as a separate file node (`is_dir == false`) alongside the directory node `app` (`is_dir == true`).
   - `resolve_path("app/")` or `resolve_path("app/config.json")` resolves through the directory partition (`dir_slice`), whereas `get_children_paged` exposes both the directory `app` and the file `app` in the parent listing.
3. **Raw Entry Identity Mapping**:
   - Every arena node originating from an archive header stores its 0-based `raw_entry_idx` in `VfsCompactArena::entry_indices`.
   - `extract_single_entry_stream(archive_path, entry_index, password)` and `extract_single_entry_memory(archive, None, entry_index, password)` address the exact raw archive entry by `entry_index` without path ambiguity.

### 2.3 R2 Handoff: Extraction Path-Matching Gaps (Do Not Modify R2 Files)
Per instruction 2, VFS1 did **not** modify R2's files (`extract_single.rs`, `selection.rs`, `extraction.rs`). The following exact reproduction cases and interface recommendations are submitted for coordinator H to route to R2:

| # | Raw Archive Entry Path | VFS Normalized Path | Failing R2 Operation & Location | Observed Failure | Recommended R2 Fix / Interface |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **R2-1** | `./a/./b/note.txt`, `a//b/file.txt`, `win\sub\file.txt`, NFD `cafe\u{301}.txt` | `a/b/note.txt`, `a/b/file.txt`, `win/sub/file.txt`, NFC `café.txt` | `extract_single_entry_memory` (`ttzip-archive/src/archive/unified/extract_single.rs:209,257,290`) & `EntrySelection::normalize_entry_path` (`ttzip-archive/src/archive/unified/selection.rs:15-26`) | Only strips leading `./` and `/`; does not normalize internal `/./`, `//`, `\`, trailing `/.`, or Unicode NFC. Path-based preview/selective extraction of normalized VFS path returns `ErrFileNotFound` / 0 extracted files. | Use shared canonical path normalizer (NFC + split on `['/', '\\']` + skip `""`/`.` + reject `..`) in `selection::normalize_entry_path` and `extract_single.rs`. |
| **R2-2** | Entry 0: `sub/foo.txt`, Entry 1: `foo.txt` (ZIP / 7z) | `sub/foo.txt`, `foo.txt` | `extract_single_entry_by_path` (`ttzip-engine/src/uniffi_api/extraction.rs:158,199`) | Uses `e.rel_path == clean_target \|\| e.rel_path.ends_with(&format!("/{}", clean_target))`. Querying root `"foo.txt"` falsely matches `"sub/foo.txt"` first and returns the wrong file's bytes. | Remove the `ends_with("/{clean_target}")` suffix heuristic; compare canonical normalized relative paths for equality, or resolve via `entry_index`. |
| **R2-3** | Entry 0: `./dup.txt`, Entry 1: `dup.txt` | `dup.txt` (2 sibling nodes with `entry_indices` `0` and `1`) | Path-only preview/extraction APIs (`extract_single_entry_by_path(archive, "dup.txt", pwd)`) | Path-only lookup always matches Entry 0 and cannot address Entry 1. | Expose `entry_index: Option<u32>` on `UniFFIVfsNodeSummary` / `VfsNodeDetail` from `arena.entry_indices[node_id]` and route preview/selective extraction by `entry_index` when present. |

---

## 3. On-Demand Loading Boundary & Swift Verification Contract

### 3.1 `ArchiveTreeNode.toComponent()`, `ArchiveTreeNode.entry`, & `ArchiveTreeNode(component:)` Value-Carried Metadata Contract
- **Previous Defects**:
  1. Recursive `toComponent()` truncated at `depth < 128`, silently displaying legitimate directories deeper than 128 levels as empty directories, while still overflowing the 544 KB cooperative stack when wide/deep trees were materialized or destroyed.
  2. When `toComponent()` was updated to attach lazy `ArchiveTreeNode` components beyond `maxEagerCompositeDepth = 64` (stored in `ArchiveCompositeDirectory.customComponents`), `ArchiveTreeNode.init(component:detectedEncoding:)` only matched `ArchiveLeafFile` and `ArchiveCompositeDirectory`. Converting a materialized component tree back via `ArchiveTreeNode(component:)` caused lazy `ArchiveTreeNode` nodes at depth >= 64 to hit the fallback `else` branch (`uncompressedSize = 0`, `vfsTree = nil`, `_children = nil`, `hasChildren = false`), losing all deep children, sizes, VFS handles, and metadata.
  3. An intermediate global side-table (`ArchiveEntryExtendedMetadataStore`) was rejected because: (a) keys lacked archive identity and could collide across archives with same-path entries, (b) path-only fallback could cross-contaminate permissions/CRC/compressedSize, (c) capacity eviction could drop live metadata, and (d) `0` was treated as missing rather than a legitimate zero value (`0o000`, `compressedSize = 0`, `crc32 = 0`).
- **New Explicit Value-Carried Optional Metadata Contract (`ArchiveEntry.swift`, `ArchiveCompositeDirectory.swift`, `ArchiveTreeNode.swift`, `ArchiveTreeNode+Composite.swift`)**:
  - **Value-Carried Optional Fields (Backward-Compatible Initializers)**:
    - `ArchiveEntry` (`ArchiveEntry.swift`) carries `public let posixPermissions: UInt32?`, `public let compressedSize: Int64?`, and `public let crc32: UInt32?` with default `nil` parameters in `init(...)`.
    - `ArchiveLeafFile` and `ArchiveCompositeDirectory` (`Types/ArchiveCompositeDirectory.swift`) carry `public let posixPermissions: UInt32?`, `public let compressedSizeBytes: Int64?`, `public let crc32: UInt32?`, `public let isEncrypted: Bool?`, and `public let detectedEncoding: String?` (defaulting to `entry`'s corresponding optional fields when omitted).
    - `ArchiveTreeNode` (`ArchiveTreeNode.swift`) stores explicit optional backing fields `_posixPermissions: UInt32?`, `_compressedSize: Int64?`, `_crc32: UInt32?`, `_isEncrypted: Bool?`, and `_detectedEncoding: String?` while exposing non-optional public properties (`posixPermissions: UInt32`, `compressedSize: Int64`, `crc32: UInt32`, `isEncrypted: Bool`, `detectedEncoding: String`) for full source compatibility.
    - Because `ArchiveTreeNode.init(...)` accepts `posixPermissions: UInt32? = nil`, `compressedSize: Int64? = nil`, `crc32: UInt32? = nil`, any caller passing an explicit `0` passes `.some(0)` (preserving legitimate zero permissions/CRC/compressedSize), whereas callers omitting the parameter pass `nil` and inherit `entry`'s optional metadata.
  - **`ArchiveTreeNode.entry` Auto-Construction (`ArchiveTreeNode.swift`)**:
    - When `_entry == nil`, `node.entry` constructs an `ArchiveEntry` carrying `posixPermissions: _posixPermissions`, `compressedSize: _compressedSize`, and `crc32: _crc32` directly by value.
    - When `_entry != nil`, `node.entry` returns `_entry` directly if no node-level override differs, or returns an updated `ArchiveEntry` merging `_posixPermissions ?? explicit.posixPermissions`, `_compressedSize ?? explicit.compressedSize`, and `_crc32 ?? explicit.crc32` while preserving `explicit`'s encryption introspection fields.
  - **Forward (`toComponent()`) & Reverse (`ArchiveTreeNode.init(component:detectedEncoding:)`)**:
    - `toComponent()` copies `_posixPermissions`, `_compressedSize`, `_crc32`, `_isEncrypted`, `_detectedEncoding`, `modificationDate`, and `entry` directly into `ArchiveLeafFile` and `ArchiveCompositeDirectory` up to `maxEagerCompositeDepth = 64`, and attaches deeper directories as lazy `ArchiveTreeNode` components.
    - `ArchiveTreeNode.init(component:detectedEncoding:)` restores `ArchiveTreeNode` components directly (`if let existingNode = component as? ArchiveTreeNode`) and reconstructs `ArchiveLeafFile` / `ArchiveCompositeDirectory` iteratively from their value-carried optional fields without any global state.

### 3.2 Verified Fields & Multi-Archive Isolation Guarantees
- **Verified Scenarios Across `Node -> Component -> Node` and `node.entry`**:
  - **Multi-Archive Same-Path Interleaved Conversion**: Two independent archives containing identical relative paths (`"d0/d1"`, `"d0/d1/app.bin"`, `"d0/d1/zero.bin"`) with distinct `posixPermissions` (`0o700` vs `0o555`, `0o755` vs `0o444`), `crc32` (`0x1111_2222` vs `0xAAAA_BBBB`), and `compressedSize` (`512` vs `256`), interleaved across `toComponent()` and `ArchiveTreeNode(component:)`, retain their exact per-archive metadata with zero cross-contamination.
  - **Legitimate Zero Values**: Explicit `posixPermissions = 0`, `compressedSize = 0`, and `crc32 = 0` are preserved as `.some(0)` on `node.entry?.posixPermissions`, `node.entry?.compressedSize`, `node.entry?.crc32`, and across `Node -> Component -> Node`.
  - **Shallow & Deep Regressions**: Shallow `0o755` executable files, `0o444` read-only files, `0o755` / `0o700` directories, and 300-level / 5,000-level deep lazy hierarchies preserve all leaf files, aggregate & leaf sizes, entry identities, and navigation on a 544 KB `.userInitiated` cooperative thread stack.

### 3.3 Shallow Root Shell & Subpath Resolution (`ArchiveHierarchySessionCache.swift`)
- `makeShallowVfsRootComposite(archivePath:vfs:)` populates `session.rootComposite` with the 1-level direct `ArchiveTreeNode` children of `""` (`rootNode.getChildren()`).
- Direct inspection of `session.rootComposite.getChildren()`, `session.rootComposite.findChild(named:)`, and `session.rootComposite.sizeBytes` returns accurate top-level children and aggregate size without recursive pre-materialization.
- `ArchiveSubpathMap.resolve(subpath: "")` and `resolve(subpath: normalized)` query `vfsTree` on demand and set `hasChildren` from `tree.getChildrenPaged(subpath: ..., offset: 0, limit: 1).totalCount > 0`, ensuring empty directories and populated directories are never confused during search, stats, or cache hits.

### 3.4 Doc-Twin Synchronization Recommendations for H
Coordinator H should synchronize the following shared documentation twins after snapshot integration:
1. `products/ttzip/core/specs/228-universal-file-manager-dual-core-architecture/data-model.md`: Document `VfsCompactArena.entry_indices` 0-based raw entry mapping, the value-carried optional metadata fields on `ArchiveEntry` / `ArchiveLeafFile` / `ArchiveCompositeDirectory`, and the 64-level eager / lazy-continuation round-trip contract between `ArchiveTreeNode.toComponent()` and `ArchiveTreeNode.init(component:detectedEncoding:)`.
2. `products/ttzip/core/specs/228-universal-file-manager-dual-core-architecture/spec.md`: Add the canonical VFS path normalization rules (`.`, `./`, `a/./b`, `a/.`, `a//b`, `\`, `..`, NFC/NFD), collision preservation contract, and lossless value-carried `Node -> Component -> Node` round-trip requirement across both shallow and lazy boundaries.

---

## 4. Modified File Inventory & SHA-256 Checksums

All modifications are confined to the VFS1 files plus the explicit value-carried metadata extensions in `ArchiveEntry.swift` and `Types/ArchiveCompositeDirectory.swift`:

| # | File Path | LOC | SHA-256 |
| :--- | :--- | :---: | :--- |
| 1 | `products/ttzip/core/rust/ttzip-engine/src/uniffi_api/vfs_loader.rs` | 200 | `d8a850afb7931ecdc863743179867471844f3cb9733c6aac2670a71b18159d6f` |
| 2 | `products/ttzip/core/rust/ttzip-engine/src/uniffi_api/vfs.rs` | 653 | `9c0eb4dfa25030b8a73244e31455d540f1ba327196949450fb5ffedaefdeba29` |
| 3 | `products/ttzip/core/rust/crates/ttzip-archive/src/fs/vfs/arena.rs` | 798 | `f652ce26e3877fcd5b44bdc6cda4387a9701b38c8556adba7ff1a815b3ddb13f` |
| 4 | `products/ttzip/core/Sources/TTZipCore/ArchiveEntry.swift` | 86 | `5e86cbfc5d8e0288c8b3970330cab46d57a79a059949dfbefa34a191ef4875a0` |
| 5 | `products/ttzip/core/Sources/TTZipCore/Types/ArchiveCompositeDirectory.swift` | 269 | `710d56dd7eaabbbb5704cbfeb32aedeff7c7a5000851877dc552ca4d2bce23fe` |
| 6 | `products/ttzip/core/Sources/TTZipCore/ArchiveTreeNode.swift` | 214 | `78b22282119beb25a647a453b63a23813b502b7ac23eac7625c842448006f364` |
| 7 | `products/ttzip/core/Sources/TTZipCore/ArchiveTreeNode+Composite.swift` | 326 | `f02ab4635c81efa1a73db17c6e3ce2b562b71152fe11cb0a10bf8c93cbb7d571` |
| 8 | `products/ttzip/apple/Sources/TTZipApp/Services/ArchiveHierarchySessionCache.swift` | 522 | `cc3114f994779879dac8fc760f7d7fd2b3313b92ef2a828d5e4ac41ddbcacff9` |
| 9 | `products/ttzip/apple/Sources/TTZipApp/Views/Explorer/FolderMediaArtboardView+Grid.swift` | 493 | `8032e8eeef970ad385e06f47cab59077bd12fd9f479fde3d10fc15a7845261c2` |
| 10 | `products/ttzip/core/rust/ttzip-engine/tests/uniffi_vfs_session_tests.rs` | 775 | `a5b15a177d9222b7b419c7efcf45ec512e987b2f7e943b85219975d2064fc35b` |
| 11 | `products/ttzip/apple/Tests/TTZipAppTests/FrontendArchitectureEvolutionTests.swift` | 612 | `5ba985f5c361763bdd0d7c31a0860b466379758322194cec80e6b7273e22b4fe` |

---

## 5. Reproduction Archive SHA-256 Checksums & Verification Results

### 5.1 Deterministic Reproduction Archives
Generated in `/Users/kevintung/.gemini/antigravity/brain/5130e40d-9051-4be5-9608-2b0b93e94bcf/scratch/` and synthesized in-memory by `uniffi_vfs_session_tests`:

| Archive File | Purpose | SHA-256 |
| :--- | :--- | :--- |
| `repro_dot_self_cycle_exec.tar` | Reproduces `./` root self-cycle (`Root -> "." -> Root`) and `0o755` executable regular file (`./bin/run.sh`) | `bd59b50c1888ef4db9700a5c7fdaab2bc1544b102df3b7565dab05a2f8320bbd` |
| `repro_four_layers_norm.tar` | Covers `.`, `./`, `././`, `a/.`, `a/./b/alpha.txt`, `a//b/beta.txt`, `win\sub\gamma.txt`, NFD `cafe\u{301}/re\u{301}sume\u{301}.txt`, `../escape_root.txt`, `a/../../escape_nested.txt` | `3478ad3c9b09e74a7507fbe3f4bf9175fe980aa8a7bf8afd3fa0b5fee77fc98f` |
| `repro_sha256_identity_collision.tar` | Covers duplicate normalized paths (`./dup.txt` vs `dup.txt`), file-vs-directory collision (`app/config.json` vs `app`), and `./a/./b/note.txt` with content SHA-256 verification | `542ce88a3d2c6d86c01902705d18021a981e99435690f0f10b5b19338cbf4670` |

### 5.2 Executed Verification Commands & Raw Results
1. **Rust 4-Layer VFS Integration & SHA-256 Identity Suite** (`bazel test -c opt //products/ttzip/core/rust/ttzip-engine:uniffi_vfs_session_tests`):
   ```text
   running 11 tests
   test test_collision_contract_duplicate_normalized_and_file_dir_same_path ... ok
   test test_uniffi_vfs_tree_backward_compatibility ... ok
   test test_vfs_session_get_children_paged_sorted ... ok
   test test_vfs_session_search_top_k ... ok
   test test_vfs_session_windowed_paging ... ok
   test test_vfs_session_resolve_path_and_nfd_normalization ... ok
   test test_vfs_session_hybrid_and_semantic_search ... ok
   test test_open_archive_session_split_volume_interception ... ok
   test test_vfs_tree_strips_dot_segments_and_preserves_exec_file_type ... ok
   test test_four_vfs_layers_path_normalization_and_no_self_cycle ... ok
   test test_normalized_vfs_entry_content_sha256_across_open_preview_and_extract ... ok

   test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   ```
2. **Compiled Swift Runtime & Round-Trip Verification on `.userInitiated` Cooperative Thread (544 KB Stack)** (`swiftc -O ... -o verify_swift_vfs_runtime && ./verify_swift_vfs_runtime`):
   - Compiles and executes the real `ArchiveEntry.swift`, `ArchiveCompositeDirectory.swift`, `PrototypeCopyable.swift`, `ArchiveTreeNode.swift`, `ArchiveTreeNode+Composite.swift`, `ArchiveSubpathMap`, and `calculateArchiveComponentStats` on a 5,000-level deep hierarchy plus two independent archives sharing identical relative paths (`"d0/d1"`, `"d0/d1/app.bin"`, `"d0/d1/zero.bin"`) on `Task.detached(priority: .userInitiated)`, verifying:
     - Interleaved `Node -> Component -> Node` conversion across two archives with same-path entries (`"d0/d1/app.bin"` with `0o755`/`0xAAAA_1111`/`600` vs `0o444`/`0xBBBB_2222`/`420`) with zero cross-contamination;
     - Legitimate zero values (`posixPermissions = 0`, `compressedSize = 0`, `crc32 = 0`) preserved as `.some(0)` on `entry` and across `Node -> Component -> Node`, while omitted parameters remain `nil`;
     - Shallow (`0o755`, `0o444`, `0o700`) and deep (5,000-level) regressions:
   ```text
   SWIFT_VFS_RUNTIME_VERIFICATION_PASSED: depth=5000 folders=5000 files=3 bytes=9728 roundTripLeaf=payload.bin roundTripSize=9728 multiArchiveIsolated=true zeroValuePreserved=true
   (Exit code: 0)
   ```
3. **LOC Gate (`tools/ci/lint_loc_gate.py --dirs products/ttzip/core`)**:
   ```text
   Scanned 811 source files (212,934 total physical LOC).
   ✅ LOC GATE PASSED: All 811 source files strictly conform to the <= 800 LOC gate.
   ```
4. **Concurrent Worktree Blockers Recorded (Not Modified by VFS1)**:
   - `products/ttzip/core/rust/crates/ttzip-archive/src/sevenz/writer.rs:510`: In-flight uncommitted edit from another group references `TTZipEncryptionMethod::None` without importing `TTZipEncryptionMethod`.
   - `products/ttzip/core/Sources/TTZipCore/Types/ArchiveCompressionFormat.swift:166`: In-flight uncommitted edit from R4 references `capabilities.canWrite`.
   - `infra/ttmedia/src/demux/format_demuxer.rs:20`: In-flight uncommitted edit from V1 references `ebml` / `mkv_index`.
   - Per instruction 5, VFS1 has preserved all other groups' uncommitted edits untouched for H to integrate on a stable snapshot.
