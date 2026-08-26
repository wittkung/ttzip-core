# Feature Specification: libarchive POSIX / Darwin Disk Space Pre-allocation (`ARCHIVE_EXTRACT_PREALLOCATE`)

**Feature Branch**: `034-libarchive-disk-preallocation`

**Created**: 2026-08-16

**Status**: Draft

**Input**: User description: "二、 Tier 1：最高优先级 / 极高合并概率的上游贡献（第 2 顺位）。2. 磁盘预分配 ARCHIVE_EXTRACT_PREALLOCATE。消除大文件解压写入碎片与 99% 晚期磁盘耗尽崩溃。解压写入吞吐提升 20% ~ 40%，具备早停机制。增加独立开关标志，标准 POSIX/Darwin 接口。"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Early Failure on Insufficient Disk Space (Priority: P1)

When a user extracts a large archive entry onto a disk volume that does not have enough remaining free space, the extraction engine immediately detects the capacity exhaustion at header creation time, safely aborts the operation with `ENOSPC`, and prevents partial corrupted file creation or wasting minutes of decompression I/O.

**Why this priority**: Eliminates the severe user frustration of long-running extractions crashing at 99% completion with unrecoverable leftover partial state.

**Independent Test**: Extract a 5GB uncompressed entry to a test disk volume with only 1GB free space; verify extraction immediately fails during `archive_write_header()` before writing data payload.

**Acceptance Scenarios**:

1. **Given** `ARCHIVE_EXTRACT_PREALLOCATE` flag is enabled and target filesystem has insufficient space for the entry size, **When** `archive_write_header()` is invoked for a regular file with known size, **Then** the call fails gracefully with `ARCHIVE_FAILED` or `ARCHIVE_FATAL` and `errno == ENOSPC`, without writing zeroed garbage to disk.
2. **Given** `ARCHIVE_EXTRACT_PREALLOCATE` flag is NOT enabled, **When** extracting to an almost-full disk, **Then** existing behavior is preserved (writes fail mid-stream during `archive_write_data_block()`).

---

### User Story 2 - Zero-Fragmentation High-Throughput Large File Extraction (Priority: P2)

When extracting large files (e.g. 100MB ~ 100GB disk images, databases, multimedia assets) on modern filesystems (APFS on macOS, ext4 / XFS on Linux), pre-allocating disk extents guarantees continuous physical block allocation and prevents severe fragmentation, significantly accelerating sequential write throughput.

**Why this priority**: Delivers 20% ~ 40% real-world extraction throughput gain on solid-state NVMe drives and APFS/ext4 filesystems.

**Independent Test**: Extract a 10GB archive on an APFS / ext4 volume with and without `ARCHIVE_EXTRACT_PREALLOCATE`, measure total elapsed time and check extent fragmentation count via filesystem tools (`fsck` / `filefrag`).

**Acceptance Scenarios**:

1. **Given** `ARCHIVE_EXTRACT_PREALLOCATE` is set on macOS (Darwin), **When** extracting a regular file with size > 0, **Then** the engine requests contiguous allocation via `fcntl(F_PREALLOCATE)` and falls back to non-contiguous pre-allocation if contiguous space is fragmented.
2. **Given** `ARCHIVE_EXTRACT_PREALLOCATE` is set on Linux/POSIX systems, **When** extracting a regular file with size > 0, **Then** the engine invokes `posix_fallocate()` or `fallocate()` to reserve unwritten extents without blocking on synchronous zero-fill where supported.

---

### User Story 3 - Graceful Fallback on Unsupported Filesystems and Non-Regular Entities (Priority: P3)

When extracting files to network filesystems (NFS, SMB), virtual filesystems (FUSE, procfs), or when extracting special entries (directories, symlinks, FIFOs, sparse files, zero-length files), the pre-allocation routine gracefully succeeds without throwing fatal errors on unsupported filesystem calls.

**Why this priority**: Guarantees zero regression and universal portability across diverse Unix/POSIX operating systems and storage backends.

**Independent Test**: Extract archives containing mixed entities (directories, 0-byte files, symlinks, sparse files) to NFS mounts and memory filesystems with `ARCHIVE_EXTRACT_PREALLOCATE` enabled.

**Acceptance Scenarios**:

1. **Given** target entry is a directory, symlink, hardlink, or 0-byte file, **When** header is processed with `ARCHIVE_EXTRACT_PREALLOCATE`, **Then** pre-allocation is silently skipped and standard extraction proceeds normally.
2. **Given** target filesystem returns `ENOTSUP`, `EOPNOTSUPP`, or `EINVAL` (e.g. NFS / FAT32), **When** pre-allocation fails due to lack of filesystem support, **Then** the engine logs a debug warning and smoothly falls back to standard streaming write without aborting extraction.

---

### Edge Cases

- **Sparse Files (`archive_entry_sparse`)**: When an entry contains sparse map blocks, pre-allocating the entire virtual size would destroy sparsity and cause unnecessary disk exhaustion. Pre-allocation MUST be skipped for entries with active sparse block maps.
- **Growing / Unknown File Sizes**: When an entry size is negative, undefined, or streaming without size in header, pre-allocation MUST be skipped.
- **Permissions and Read-Only Targets**: If file descriptor cannot be opened with write permissions, header creation returns error before pre-allocation.
- **Disk Full during partial write**: If pre-allocation is unsupported and falls back, traditional mid-stream write error handling remains completely intact.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST define a new public extraction flag `ARCHIVE_EXTRACT_PREALLOCATE` in `archive.h` (using the next available bitmask value, e.g. `0x0080` or `0x00010000`).
- **FR-002**: `archive_write_disk_set_options()` MUST accept and parse `ARCHIVE_EXTRACT_PREALLOCATE`.
- **FR-003**: On macOS / Darwin, `archive_write_disk_posix.c` MUST utilize `fcntl(fd, F_PREALLOCATE, &fst)` with `F_ALLOCATECONTIG | F_ALLOCATEALL`, falling back to `F_ALLOCATEALL` if contiguous allocation fails, and ensure the logical file size is synchronized via `ftruncate(fd, size)` if necessary.
- **FR-004**: On Linux and standard POSIX systems supporting `posix_fallocate()` or `fallocate()`, `archive_write_disk_posix.c` MUST invoke the appropriate OS system call to reserve blocks.
- **FR-005**: If pre-allocation returns `ENOSPC`, the extraction of the current entry MUST fail immediately with informative error message `No space left on device` and return `ARCHIVE_FAILED`.
- **FR-006**: If pre-allocation returns filesystem incompatibility errors (`EINVAL`, `ENOTSUP`, `EOPNOTSUPP`, `ENOSYS`), the engine MUST ignore the error, clear errno, and fall back to normal unallocated streaming write.
- **FR-007**: Pre-allocation MUST be skipped for zero-size files, directories, symlinks, special devices, and entries with sparse block structures (`archive_entry_sparse_count(entry) > 0`).
- **FR-008**: Build configuration systems (CMake `CMakeLists.txt` and Autotools `configure.ac`) MUST probe for `fpreallocate`, `posix_fallocate`, and `fallocate` headers and library symbols (`HAVE_POSIX_FALLOCATE`, `HAVE_F_PREALLOCATE`, `HAVE_FALLOCATE`).

### Key Entities

- **`archive_write_disk`**: The POSIX disk writer instance managing filesystem object creation, metadata restoration, and block stream writes.
- **`archive_entry`**: The metadata descriptor containing entry type (regular file, directory, symlink), size in bytes, and sparse block records.
- **`fstore_t` (Darwin)**: The filesystem extent allocation descriptor specifying allocation flags, offset, length, and returned allocated bytes.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% early detection of `ENOSPC` for regular files when target volume free space is less than the entry size, aborting within < 1 millisecond.
- **SC-002**: Sequential extraction write throughput on APFS / ext4 for files >= 1GB improves by at least 15% ~ 35% under concurrent extraction loads.
- **SC-003**: 100% pass rate across libarchive test suite (`libarchive_test`) and zero regression on all non-regular file extractions (symlinks, hardlinks, sparse files, zero-byte files).
- **SC-004**: Full cross-platform portability: compiles and functions seamlessly on macOS (Sonoma/Sequoia), Linux (glibc/musl), FreeBSD, OpenBSD, Windows (MSVC/MinGW fallback).

## Assumptions

- Target operating systems provide either Darwin `F_PREALLOCATE`, POSIX `posix_fallocate`, Linux `fallocate`, or none (safe no-op fallback).
- Enabling `ARCHIVE_EXTRACT_PREALLOCATE` is opt-in for callers through `archive_write_disk_set_options()`, preventing unexpected behavioral shifts for existing consumers.
- BSD KNF coding style and libarchive single-responsibility commit discipline apply to all modifications.
