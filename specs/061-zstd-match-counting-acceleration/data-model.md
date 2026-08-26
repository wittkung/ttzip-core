# Data Model: Zstandard Match Counting Acceleration & Double-Fast Engine Alignment

**Feature**: `061-zstd-match-counting-acceleration`
**Date**: 2026-08-17
**Status**: Completed

---

## 1. Core Structures & Memory Entities

### 1.1 `DoubleFastTable` (Dual-Hash Direct Index Table)

Directly indexed dual-table structure for $O(1)$ short (4-byte) and long (8-byte) match index lookup:

| Field Name | Type | Nullable | Required | Description |
| :--- | :--- | :--- | :--- | :--- |
| `table_small` | `UnsafeMutablePointer<UInt32>` | No | Yes | 64K entries (256 KB) direct-indexed by 4-byte hash |
| `table_long` | `UnsafeMutablePointer<UInt32>` | No | Yes | 64K entries (256 KB) direct-indexed by 8-byte hash |
| `hash_mask_small` | `UInt32` | No | Yes | Bitmask for small table index (`(1 << 16) - 1`) |
| `hash_mask_long` | `UInt32` | No | Yes | Bitmask for long table index (`(1 << 16) - 1`) |
| `workspace_size` | `UInt64` | No | Yes | Total contiguous workspace buffer size in bytes (524,288 B) |

---

### 1.2 `MatchCandidate` (Candidate Match Evaluation Structure)

Represents candidate match discovery and lookahead verification:

| Field Name | Type | Nullable | Required | Description |
| :--- | :--- | :--- | :--- | :--- |
| `length` | `UInt32` | No | Yes | Length of matched common prefix in bytes (2..273) |
| `distance` | `UInt32` | No | Yes | 0-based backward distance offset in bytes (1..dict_size) |
| `source_pos` | `UInt32` | No | Yes | 0-based position in input buffer where match originates |
| `is_long_match` | `Bool` | No | Yes | True if match originated from 8-byte long table probe |

---

### 1.3 `MatchFinderContext` (Fast Match Finder Execution Context)

Encapsulates match finder state and streaming buffer boundaries:

| Field Name | Type | Nullable | Required | Description |
| :--- | :--- | :--- | :--- | :--- |
| `buffer` | `UnsafePointer<UInt8>` | No | Yes | Input buffer memory pointer |
| `buffer_size` | `UInt64` | No | Yes | Total input buffer size in bytes |
| `pos` | `UInt64` | No | Yes | Current cursor offset within input buffer |
| `dict_size` | `UInt32` | No | Yes | Sliding window dictionary size in bytes (e.g. 262,144 B) |
| `nice_len` | `UInt32` | No | Yes | Target nice length for greedy termination (e.g. 32 B) |
| `cut_value` | `UInt32` | No | Yes | Maximum probe lookahead iterations |
| `tables` | `DoubleFastTable` | No | Yes | Embedded dual-hash direct index tables |

---

### 1.4 `UpstreamPatchArtifact` (Upstream Contribution Metadata)

Defines structure and patch metadata for upstream contributions to `facebook/zstd`:

| Field Name | Type | Nullable | Required | Description |
| :--- | :--- | :--- | :--- | :--- |
| `pr_id` | `String` | No | Yes | PR branch identifier (e.g. `feat/arm64-neon-zstd-count`) |
| `target_repo` | `String` | No | Yes | Target repository (`facebook/zstd`) |
| `target_branch` | `String` | No | Yes | Target branch (`dev`) |
| `worktree_path` | `String` | No | Yes | Local worktree workspace directory path |
| `commit_series` | `Array<String>` | No | Yes | Ordered list of commit summaries (`infra` -> `feat` -> `test`) |
| `validation_status` | `String` | No | Yes | Compilation and test result status (`PASS` / `FAIL`) |
