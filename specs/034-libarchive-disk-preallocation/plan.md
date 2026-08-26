# Implementation Plan: libarchive POSIX / Darwin Disk Space Pre-allocation (`ARCHIVE_EXTRACT_PREALLOCATE`)

## Technical Context

- **Upstream Baseline**: `libarchive` master branch (`Vendor/libarchive-upstream/`).
- **Target Files**:
  - `libarchive/archive.h`: Define `ARCHIVE_EXTRACT_PREALLOCATE (0x80000)`.
  - `libarchive/archive_write_disk_posix.c`: Add `preallocate_file()` helper and inject in `_archive_write_disk_header()`.
  - `libarchive/test/test_write_disk_preallocate.c`: New unit test covering regular, sparse, 0-byte, and non-regular entries.
  - `libarchive/test/CMakeLists.txt` & `Makefile.am`: Register new test.
  - `CMakeLists.txt` & `configure.ac`: Add `HAVE_POSIX_FALLOCATE` and `HAVE_F_PREALLOCATE` detection.
- **Language**: C99 / C11 + POSIX.1-2001 APIs.
- **Coding Standard**: BSD KNF (Kernel Normal Form), 8-character hard tabs, K&R braces, zero extraneous whitespace.

---

## Constitution Check

- [x] **Zero-Cost Abstraction**: Pre-allocation runs exactly once per file at header creation time ($O(1)$ metadata extent reservation). Zero heap allocation or dynamic objects on data-streaming hot path (`write_data_block`).
- [x] **Fast-Path Bypass**: Sparse files (`ARCHIVE_EXTRACT_SPARSE` or `archive_entry_sparse_count > 0`), zero-length files, non-regular files, and HFS+ compressed streams bypass pre-allocation immediately.
- [x] **Subsystem Freeze**: No frozen files (`ZipParallel*`, `CTTZipExtract.c`, etc.) are touched.
- [x] **Logging & Clean Output**: Zero bare `printf`/`fprintf`. Errors propagate through `archive_set_error()`.

---

## Phase 0: Research & Investigation

- [x] - R001 [SUBAGENT:research] 《Darwin `fcntl(F_PREALLOCATE)` 与 APFS 空间预分配行为机制研究》: Complete (`research.md`). Two-tier cascade (`F_ALLOCATECONTIG | F_ALLOCATEALL` ➔ `F_ALLOCATEALL`) + logical size synchronization.
- [x] - R002 [SUBAGENT:research] 《POSIX `posix_fallocate` 与 Linux `fallocate` 语义与错误码处理机制研究》: Complete (`research.md`). Direct return code consumption, fatal `ENOSPC`/`EDQUOT`/`EFBIG` early abort, non-fatal `EINVAL`/`EOPNOTSUPP` fallback.
- [x] - R003 [SUBAGENT:research] 《libarchive `archive_write_disk_posix.c` 与 `archive.h` 扩展点与现有稀疏/截断逻辑研究》: Complete (`research.md`). Bitmask `0x80000`, injection in `_archive_write_disk_header()` after `restore_entry(a)`.

---

## Phase 1: Contracts & Data Model

- [x] **Data Model**: `specs/034-libarchive-disk-preallocation/data-model.md`
- [x] **Contracts**:
  - `[SUBAGENT:research]` `specs/034-libarchive-disk-preallocation/contracts/disk_preallocate_options.json`
  - `[SUBAGENT:research]` `specs/034-libarchive-disk-preallocation/contracts/disk_preallocate_result.json`
- [x] **Validation Quickstart**: `specs/034-libarchive-disk-preallocation/quickstart.md`

---

## Planned Modifications by Component

### Component 1: Public Header (`libarchive/archive.h`)
- Add `#define ARCHIVE_EXTRACT_PREALLOCATE (0x80000)` with BSD comment.

### Component 2: POSIX Disk Writer (`libarchive/archive_write_disk_posix.c`)
- Add `preallocate_file(struct archive_write_disk *a)` static function:
  - Guard against `a->fd < 0`, `a->filesize <= 0`, `(a->mode & AE_IFMT) != AE_IFREG`.
  - Guard against `(a->flags & ARCHIVE_EXTRACT_SPARSE) != 0` and `archive_entry_sparse_count(a->entry) > 0`.
  - On Darwin (`HAVE_F_PREALLOCATE`): `fcntl(F_PREALLOCATE, &fst)` two-tier cascade.
  - On POSIX (`HAVE_POSIX_FALLOCATE`): `posix_fallocate(a->fd, 0, (off_t)a->filesize)`. Early abort on `ENOSPC`/`EDQUOT`/`EFBIG`.
- Call `preallocate_file(a)` inside `_archive_write_disk_header()` after `restore_entry(a)`.

### Component 3: Unit Tests (`libarchive/test/test_write_disk_preallocate.c`)
- Add unit tests validating:
  1. Regular file pre-allocation succeeds and preserves file size.
  2. 0-byte file extraction succeeds without error.
  3. Sparse file extraction does not over-allocate disk extents.
  4. Directory and symlink entries ignore the flag cleanly.

### Component 4: Build System Probes
- `CMakeLists.txt`: `CHECK_FUNCTION_EXISTS_GLIBC(posix_fallocate HAVE_POSIX_FALLOCATE)` and `CHECK_SYMBOL_EXISTS(F_PREALLOCATE "fcntl.h" HAVE_F_PREALLOCATE)`.
- `configure.ac`: `AC_CHECK_FUNCS([posix_fallocate])` and `AC_CHECK_DECL([F_PREALLOCATE]...)`.
- `libarchive/test/CMakeLists.txt` & `Makefile.am`: Register `test_write_disk_preallocate.c`.
