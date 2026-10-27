# Research & Technology Decisions: Rust Core & Glue Layer Architectural Reconstruction

**Feature**: `223-rust-core-and-glue-architectural-reconstruction`  
**Date**: 2026-08-24  
**Spec Reference**: [`spec.md`](file:///Users/kevintung/Documents/dev/TTZip/core/specs/223-rust-core-and-glue-architectural-reconstruction/spec.md)

---

## 1. Unified ArchiveSource Abstraction & Fault-Tolerant Memory Mapping

### Problem Context
The current codebase has 6 ad-hoc calls to `fs::read(archive_path)` in:
- `core/rust/ttzip-engine/src/archive/unified/extract_single.rs:39` (ZIP)
- `core/rust/ttzip-engine/src/archive/unified/extract_single.rs:58` (7z)
- `core/rust/ttzip-engine/src/archive/unified/extract_single.rs:78` (TAR)
- `core/rust/ttzip-engine/src/archive/in_place_edit.rs:190` (ZIP in-place)
- `core/rust/ttzip-engine/src/archive/in_place_edit.rs:356` (7z in-place)
- `core/rust/ttzip-engine/src/zip/writer/types.rs:131` (`ZipInputItem.data`)

Calling `fs::read` loads the entire file into the heap ($O(N)$ memory), immediately causing OOM on 20GB+ archives. Simply replacing `fs::read` with unconditional `mmap` introduces `SIGBUS` panics on network drives (SMB/NFS) and removable media.

### Decision
Implement a dynamic `ArchiveSource` trait supporting:
1. `MmapSource` (via `memmap2` crate) for local NVMe/APFS drives with `madvise(MADV_WILLNEED)` for Central Directory parsing.
2. `StreamSource` (via `pread` on file descriptor with 64KB sliding ring-buffer) for remote SMB/NFS mounts, USB drives, or virtual filesystems.
3. Media detection via POSIX `statfs(path, &sfs)` inspecting `sfs.f_flags & MNT_LOCAL` and `sfs.f_fstypename`.

### Source & Evidence
- `memmap2` documentation & standard POSIX `statfs(2)` manual.
- SQLite VFS (`os_unix.c`) and ripgrep (`grep_searcher::MmapChoice`).

---

## 2. Streaming Parallel Multi-Core ZIP Engine

### Problem Context
The existing architecture had a dual-engine schism:
- `archive/unified/create.rs:69-127`: Routes ZIP compression from Swift/C-ABI to single-threaded `libarchive` (`archive_write_set_format_zip`).
- `zip/writer/parallel.rs:18-136`: Rayon-parallel ZIP compressor, but loads all input files entirely into `Vec<ZipInputItem>` and clones raw payloads (`chunk.to_vec()`), causing $M_{\text{peak}} \approx 3U + C$.
- `zip/writer/store_stream.rs:94-355`: Lock-free Store-mode multi-core ZIP stream writer with `pwrite` and APFS preallocation, but not exposed to FFI.

### Decision
Construct a unified `StreamingParallelZipWriter`:
1. Producer-consumer pipeline: Rayon compresses individual files into thread-local bounded chunks (using `libdeflate`).
2. Lock-free/Atomic sequential offset reservation: each compressed entry claims its `(lfh_offset, compressed_len)` atomically.
3. Positional disk writer (`pwrite` directly to target file descriptor) with `apfs_preallocate` / `fstore_t` preallocation.
4. Central Directory and Zip64 End of Central Directory emitted in a single sequential tail stream.
5. Peak memory bounded by `thread_count * max_chunk_size` ($< 64\text{MB}$ total RSS).

### Source & Evidence
- `core/rust/ttzip-engine/src/zip/writer/store_stream.rs:94-355`
- Info-ZIP Appnote & PKWare Zip64 specification.

---

## 3. Cross-Language Error Diagnostics Pipeline

### Problem Context
All FFI endpoints in `core/rust/ttzip-engine/src/ffi/` return a flat integer enum `TTZipStatus` (`ErrOpenFailed = -7`, `ErrInvalidParam = -1`). When operations fail, all error context (line number, errno, entry path, byte offset, missing permissions) is discarded, leaving the Swift UI layer unable to provide actionable feedback to users.

### Decision
1. Implement a thread-local `DiagnosticErrorContext` in `types.rs` storing:
   - `status: TTZipStatus`
   - `message: [c_char; 512]` (zero-heap, stack-formatted C-string)
   - `entry_path: [c_char; 256]`
   - `offset: u64`
2. Export C-ABI functions:
   - `ttzip_rust_last_error_message() -> *const c_char`
   - `ttzip_rust_clear_last_error()`
3. In Swift, `ArchiveEngineBridge` checks `status < 0`, reads `ttzip_rust_last_error_message()`, and wraps it into a typed `TTZipError.engineFailure(status, details)`.

---

## 4. VFS Session Lifecycle & Zero-Allocation Fuzzy Search

### Problem Context
In `core/Sources/TTZipCore/Bridge/RustVfsBridge.swift:64-93`:
- `fuzzySearch` calls `withTreeHandle`, executing `entries.map { strdup($0.path) }` and `ttzip_rust_vfs_tree_build` on every single keystroke.
- For 100,000 entries: 100k `strdup` + 100k `free` + 100k node tree build per character typed.
- In `core/rust/ttzip-engine/src/fs/vfs/search.rs:33`: `target.chars().collect::<Vec<char>>()` allocates two heap vectors per tree node.

### Decision
1. Promote tree handle to `RustVfsSession` lifetime: the Rust tree handle is built once when the archive is inspected, and freed when the tab/archive is closed.
2. Refactor `fuzzy_match` to use zero-allocation byte/char iterators over `&str` without collecting `Vec<char>`.
3. In FFI `ttzip_rust_vfs_fuzzy_search`, return matching entry indexes (`u32`) as a contiguous array buffer to Swift, eliminating all per-result `CString` allocations.

---

## 5. Dead Code Elimination & SPSC Safety Hardening

### Problem Context
- `core/rust/ttzip-engine/src/runtime/worker_pool/pool.rs`: 298 lines implementing `EventDrivenWorkerPool`, exported via 25 C-ABI functions in `ttzip_rust_glue.h:301-343`. Subagent verification confirmed 0 Swift references.
- `core/rust/ttzip-engine/src/runtime/ring_buffer/spsc.rs:114-136`: `SpscRingBuffer` exposed `push(&self)` and `pop(&self)` with `UnsafeCell` under `unsafe impl Sync`, permitting concurrent push from multiple threads (data race UB).

### Decision
1. Delete `runtime/worker_pool/pool.rs` and remove all 25 `ttzip_rust_worker_pool_*` / `ttzip_rust_*_ring_buffer_*` exports from `ttzip_rust_glue.h`.
2. Refactor `SpscRingBuffer` to remove `push/pop` on the shared struct, forcing callers to invoke `split() -> (SpscProducer<T>, SpscConsumer<T>)` where `SpscProducer` and `SpscConsumer` are `Send + !Sync`.

---

## 6. Password Vault & Cryptographic Hardening

### Problem Context
- `core/rust/ttzip-engine/src/crypto/vault.rs:106-132`: GHASH multiplication `mul_h` performs 128-bit secret-dependent conditional branching (`if ((x[byte_idx] >> bit_idx) & 1) != 0` and `if lsb != 0`), leaking the $H$ authentication subkey via cache/branch timing.
- macOS already provides `CryptoKit.AES.GCM` backed by hardware CoreCrypto and Secure Enclave.

### Decision
1. Delegate Password Vault credential caching directly to Swift `CryptoKit.AES.GCM` and macOS Keychain.
2. Remove the custom generic AES-256-GCM / GHASH from `crypto/vault.rs`.
3. For format-specific WinZip AES decryption in Rust, retain only the dedicated WinZip PBKDF2/CTR routines accelerated by NEON SIMD (`crypto/aes256/simd.rs`).
