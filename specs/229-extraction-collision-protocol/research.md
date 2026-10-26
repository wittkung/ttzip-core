# Research: Extraction Name-Collision Protocol

**Feature**: `229-extraction-collision-protocol` | **Date**: 2026-10-03

Every decision below cites the source consulted. Platform facts come from the XNU source, macOS man pages, Apple documentation and the UniFFI manual; codebase facts come from `git grep` inventories of this repository (file:line).

---

## R1. Atomic primitives for commit

**Decision**: Use `renameatx_np(src_dirfd, name, dst_dirfd, name, flags)` for every move into the destination.

| Situation | Flags | Result on conflict appearing concurrently |
|---|---|---|
| Free name (no conflict) | `RENAME_EXCL \| RENAME_NOFOLLOW_ANY` | `EEXIST` → becomes a `CommitRecheck` conflict, nothing overwritten |
| Replace, same type (file/symlink over file/symlink) | `RENAME_NOFOLLOW_ANY` | Kernel replaces the name atomically; original unlinked |
| Replace, type mismatch (dir ↔ non-dir) | `RENAME_SWAP \| RENAME_NOFOLLOW_ANY` | Atomic exchange; original lands in `displaced/` |
| Keep Both | `RENAME_EXCL \| RENAME_NOFOLLOW_ANY` to `<stem> N<.ext>` | `EEXIST` → N+1 |

**Rationale**:
- `RENAME_EXCL` returns `EEXIST` if the destination exists, closing the check-then-rename TOCTOU window; `RENAME_SWAP` atomically exchanges source and target and "source and target need not be of the same type, i.e. it is possible to swap a file with a directory"; the two flags together return `EINVAL`. Source: `rename(2)`, https://keith.github.io/xcode-man-pages/rename.2.html; XNU `bsd/vfs/vfs_syscalls.c` `renameatx_np` / `renameat_internal`, https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/vfs/vfs_syscalls.c.
- `RENAME_NOFOLLOW_ANY` makes path resolution fail with `ELOOP` if any component is a symlink (macOS 11+, below our macOS 14 floor). Combined with dirfds opened `O_DIRECTORY | O_NOFOLLOW`, no symlink at the destination can redirect a write (FR-009). Same sources.
- Cross-volume renames fail with `EXDEV`; staging therefore lives inside `destination_root` (data-model §3.1). Source: XNU `v_mount` check in `renameat_internal`.
- Unsupported flags return `ENOTSUP` on filesystems lacking `VOL_CAP_INT_RENAME_SWAP` / `VOL_CAP_INT_RENAME_EXCL`. Source: `rename(2)` `[ENOTSUP]`; `getattrlist(2)`.

**Fallback for volumes without `RENAME_EXCL`** (exFAT, FAT32, some SMB/NFS; revised 2026-10-17 after audit):
- `linkat(2)` fails with `EEXIST` if the new name exists and never replaces it, placing the complete staged file inode at the destination in one atomic step before the staged link is unlinked; used for regular files when `RENAME_EXCL` is unavailable and the volume supports hard links (FR-022). Source: `link(2)`, https://keith.github.io/xcode-man-pages/link.2.html.
- When no single-step atomic exclusive primitive exists for the item kind (directories and symlinks when `RENAME_EXCL` is unavailable, or regular files when `linkat` also fails with `ENOTSUP` / `EOPNOTSUPP` / `ENOSYS` / `EPERM` / `EMLINK`), `place_exclusive` returns `ENOTSUP` (`FailureReason::NotSupportedByVolume`), leaving both the existing destination (including any empty directory) and the staged source untouched.
- `ENOSYS` / `EOPNOTSUPP` are treated like `ENOTSUP`. A failed capability probe is treated as "unsupported" (`VolCaps::default()`), never as "plain rename allowed".

**Alternatives rejected**:
- Swift-side "check then extract": two full passes for 1M entries, TOCTOU window, and Keep Both would need a second temporary directory (rejected in Phase 11 review).
- `link()` + `unlink()` as the primary placement: does not work for directories and costs two syscalls; kept only as the fallback for regular files on volumes without `RENAME_EXCL`.
- `fstatat` + plain `renameat` as fallback: not exclusive; a lookup is not an exclusivity guarantee and plain `rename` silently replaces an existing file or empty directory.
- `openat(O_CREAT | O_EXCL)` / `mkdirat` reservation + `(st_dev, st_ino)` check + plain `renameat`: rejected because (1) a crash between reservation creation and `renameat` leaves a 0-byte file or empty directory at the destination, violating FR-011 ("original or complete new item"), and (2) plain `renameat` cannot atomically verify target inode identity, leaving a race window where a replaced target or existing empty directory can be overwritten.
- Commit journal for crash recovery: unnecessary, because each step above leaves the path original-or-new; recovery is "delete staging" (data-model §3.1).

## R2. Name comparison rules (FR-008)

**Decision**: Delegate case and Unicode-normalization equivalence to the filesystem by probing with `fstatat(dirfd, name, AT_SYMLINK_NOFOLLOW)`. Never compare names in user space. Obtain the on-disk spelling of an existing item with `getattrlistat(dirfd, name, ATTR_CMN_NAME, FSOPT_NOFOLLOW)` only for conflicts (not for every entry).

**Rationale**: APFS "is normalization-insensitive in both the case-insensitive and case-sensitive variants … a filename can be found with any of its normalization variants", and the default macOS variant is case-insensitive. A lookup of `README.md` therefore finds `Readme.md` on the default volume and does not on a case-sensitive one, which is exactly the volume's rule. Source: Apple File System Guide FAQ, https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/APFS_Guide/FAQ/FAQ.html. `ATTR_CMN_NAME` returns the stored UTF-8 name. Source: `getattrlist(2)`, https://keith.github.io/xcode-man-pages/getattrlist.2.html.

**Alternatives rejected**: user-space case folding / NFC comparison (must replicate per-volume Unicode version — APFS uses Unicode 9.0 tables, HFS+ 3.2 — and still races with the filesystem); `fcntl(F_GETPATH)` for the on-disk name (requires opening the item; research notes it is less robust than `ATTR_CMN_NAME` under cache races).

## R3. Keep Both naming (FR-006)

**Decision**: `<stem> N<.ext>` with N starting at 2, extension = text after the last `.` when the item is a file and the stem is non-empty (`.bashrc` has no extension); directories never split an extension. The stem is the incoming name as-is, so an incoming `name 2.txt` clashing becomes `name 2 2.txt`. Existence is decided by the exclusive rename itself (`EEXIST` → N+1), not by a prior lookup.

**Rationale**: Finder "adds a number to the name of the duplicate (for example, 'filename 2')"; Archive Utility never overwrites on extraction and applies the same numbering without prompting. Source: macOS User Guide, https://support.apple.com/guide/mac-help/mchlp1204/mac. The `name 2 2.txt` behavior for already-numbered names is reported by the research agent from community sources (https://discussions.apple.com/thread/254471928) and is marked **low-confidence**; it is also the rule that needs no parsing, so it is adopted independently of Finder parity.

## R4. UniFFI shape of the session

**Decision**: `#[derive(uniffi::Object)] ExtractionSession` with `#[uniffi::constructor] fn new(...) -> Result<Arc<Self>, TTZipError>`, synchronous methods taking `&self`, interior state in `Mutex<SessionSlot>`. Conflicts are paged (`conflict_page(offset, limit ≤ 1024)`); no method returns an unbounded `Vec`.

**Rationale**:
- Objects must be `Send + Sync`, methods take `&self`, mutation via `Mutex`/atomics; constructors may return `Result<Arc<Self>, E>`. Sources: https://mozilla.github.io/uniffi-rs/latest/internals/object_references.html, https://mozilla.github.io/uniffi-rs/latest/proc_macro/interfaces.html.
- `sequence<T>` crosses the boundary as a serialized `RustBuffer` (i32 count + serialized items): O(N) serialize + deserialize + transient copy. Re-extracting an archive into the same folder makes every entry a conflict, so 1M conflicts is a normal case; paging bounds each crossing to ≤ 1024 records. Source: https://mozilla.github.io/uniffi-rs/latest/internals/lifting_and_lowering.html.
- Synchronous exports block the calling thread; Swift calls `stage` from the extraction queue, which matches today's `NativeComputeDispatcher` serial queue usage.
- Progress keeps the existing `UniFFIProgressCallback` (a `callback_interface`). UniFFI marks callback interfaces soft-deprecated in favor of foreign traits; migrating progress is out of scope and tracked separately, because it changes every extraction and compression signature.

**Alternatives rejected**: a blocking "on_conflict" callback from a Rust worker thread into Swift (Rust thread parks while the main actor shows UI; cancellation and app termination leave the worker blocked; deadlock if the callback is invoked while the main actor waits on the extraction); UniFFI `async fn` (adds an executor dependency for work that is already scheduled by Swift).

## R5. Codebase facts that shape the design

| Fact | Evidence | Consequence |
|---|---|---|
| Full extraction stages into `.ttzip_staging_*` beside the destination, then `commit()` → `promote_dir_contents` removes any existing destination file before rename | `crates/ttzip-archive/src/fs/staging_sandbox.rs:67-139` (`fs::remove_file` at :107, :119, :129) | Replace `StagingSandbox` with the session's staging + commit walker; delete `promote_dir_contents` |
| A file at the destination where the archive has a directory makes `create_dir_all` fail mid-promotion, leaving a half-promoted tree | `staging_sandbox.rs:113-116` | Type mismatch becomes an explicit conflict |
| Selected-entry extraction writes directly into the destination with `truncate(true)` | `crates/ttzip-archive/src/archive/unified/extract_single.rs:178-335` | Selected entries stage like full extraction |
| `extract_single_entry_to_path` removes the destination then renames | `ttzip-engine/src/uniffi_api/extraction.rs:598-601` | Function deleted (no callers, below) |
| `extract_single_entry_to_path` and `extract_archive_stream_advanced` have no callers outside generated bindings | call-site inventory (Swift, Python SDK, C++/C# bindings) | Both exports deleted |
| `overwrite_existing` is constructed at 40+ sites; `TTZipExtractOptions` is `#[repr(C)]` with `abi_version` | `crates/ttzip-types/src/options/mod.rs:75-98`, inventory | Same-size field swap `overwrite_existing: bool` → `collision_policy: u8`, ABI version 3; v2 callers rejected |
| No Swift→Rust app-support path exists | `AppDelegate.swift:36-38`, `TTZipEngineFacading.swift:233-235` | Stale-staging registry lives in Swift; engine exposes `sweep_stale_staging` (data-model §3.2) |
| Finder "Extract Here" reaches the app via `ttzip://action?type=extract_here` → `AppIntentDispatcher` → `quickExtractArchive` | `FinderSync.swift:152`, `AppIntentParser.swift:68`, `AppIntentDispatcher.swift:89` | Dispatcher passes `.policy(.keepBoth)` (US3) |
| Drag-out, FileProvider, in-place edit, QuickLook use fresh temp dirs or memory | `ArchiveFilePromiseProvider.swift:14`, `TTZipFileProviderExtension.swift:310`, `InPlaceArchiveMutationEngine.swift:58,349`, `ArchiveExtractor.swift:379` | `.policy(.keepBoth)`; never prompt |
| `ForensicCollisionPanel` offers six actions incl. `keepNewer`, `keepLarger`, `mergeDirectory`; `renameIncoming` documents a ` (1)` suffix | `infra/ttkit/Sources/TTKitUI/Omnibar/ForensicCollisionPanel.swift:12-25` | Actions reduced to Replace / Keep Both / Skip / Stop; directory merge is automatic (FR-007); Return = Keep Both (FR-005) |
| `OverlayState` stores the prompt as four independent optionals (`showForensicCollision`, `collisionSource`, `collisionTarget`, `collisionResolver`) | `AppSubStates.swift` (Phase 10) | Replaced by one `collisionPrompt: CollisionPrompt?` so "shown without data" is unrepresentable |
| Settings use `@AppStorage` in `SettingsView` and the App Group store for cross-process keys | `SettingsView.swift:46-49`, `TTZipPreferencesStore.swift:10-23` | `ttzip_collision_default` in `@AppStorage` (main app only reads it) |
| Tests asserting silent overwrite | `staging_sandbox.rs:157`, `secure_extract_tests.rs:195`, `AppViewStateArchiveOperationsTests:114`, `tests_lifecycle.rs`, `UniFFIBindingsTests:122`, `safe_extract.rs:625` | Rewritten to the new semantics in the same change |
