# Phase 1 Data Model: libarchive POSIX / Darwin Disk Space Pre-allocation

> **Consistency Contract**: All entity field names, types, and required constraints defined here strictly match the corresponding JSON Schemas in `contracts/`.

---

## 1. Entity: `ArchiveExtractFlags`

Represents the extraction configuration bitmask options passed to `archive_write_disk_set_options()`.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `flags` | `integer` (uint32) | Yes | Bitwise OR of `ARCHIVE_EXTRACT_*` constants. |
| `preallocate_enabled` | `boolean` | Yes | True if `ARCHIVE_EXTRACT_PREALLOCATE (0x80000)` bit is set. |
| `sparse_enabled` | `boolean` | Yes | True if `ARCHIVE_EXTRACT_SPARSE (0x1000)` bit is set. |
| `safe_writes_enabled` | `boolean` | Yes | True if `ARCHIVE_EXTRACT_SAFE_WRITES (0x40000)` bit is set. |

---

## 2. Entity: `DiskPreallocateRequest`

Represents the input parameters evaluated before invoking OS-level extent pre-allocation on a file descriptor.

| Field Name | Type | Required | Constraints | Description |
| :--- | :--- | :--- | :--- | :--- |
| `fd` | `integer` (int32) | Yes | `>= 0` | Valid open POSIX file descriptor with write access. |
| `filesize` | `integer` (int64) | Yes | `> 0` | Expected logical entry file size in bytes from archive metadata. |
| `entry_type` | `string` | Yes | Enum: `"regular"`, `"directory"`, `"symlink"`, `"hardlink"`, `"fifo"`, `"device"` | File type format. Preallocation only executes for `"regular"`. |
| `has_sparse_map` | `boolean` | Yes | Boolean | True if `archive_entry_sparse_count(entry) > 0`. |
| `is_hfs_compression` | `boolean` | Yes | Boolean | True if macOS HFS+/APFS transparent stream compression is active. |

---

## 3. Entity: `DiskPreallocateResult`

Represents the structured execution outcome and telemetry of a space pre-allocation operation.

| Field Name | Type | Required | Constraints | Description |
| :--- | :--- | :--- | :--- | :--- |
| `status` | `string` | Yes | Enum: `"success"`, `"skipped"`, `"fallback"`, `"fatal_error"` | Outcome classification. |
| `system_call` | `string` | Yes | Enum: `"f_preallocate"`, `"posix_fallocate"`, `"none"` | The underlying OS pre-allocation interface invoked. |
| `contiguous` | `boolean` | Yes | Boolean | True if contiguous extent allocation succeeded on Darwin. |
| `error_code` | `integer` (int32) | Yes | `>= 0` | System error code (0 for success, or `errno` / `posix_fallocate` return value). |
| `error_message` | `string` | No | String | Descriptive error text set on `archive_set_error()` when status is `"fatal_error"`. |

---

## 4. Entity: `FStoreDescriptor` (Darwin Specific)

Represents the kernel `struct fstore` layout populated for `fcntl(fd, F_PREALLOCATE, &fst)`.

| Field Name | Type | Required | Constraints | Description |
| :--- | :--- | :--- | :--- | :--- |
| `fst_flags` | `integer` (uint32) | Yes | `F_ALLOCATECONTIG \| F_ALLOCATEALL` or `F_ALLOCATEALL` | Allocation strategy flags. |
| `fst_posmode` | `integer` (int32) | Yes | `F_PEOFPOSMODE (2)` | Position mode relative to physical EOF. |
| `fst_offset` | `integer` (int64) | Yes | `0` | Offset in bytes relative to position mode. |
| `fst_length` | `integer` (int64) | Yes | `> 0` | Number of bytes to allocate from storage pool. |
| `fst_bytesalloc` | `integer` (int64) | Yes | `>= 0` | Actual bytes allocated by the kernel upon return. |
