# Research & Defect Analysis Report: Deep Code Quality, Memory Safety, and Invariant Hardening

**Feature Branch**: `157-deep-code-quality-and-memory-safety-hardening`
**Created**: 2026-08-20
**Spec**: [spec.md](./spec.md)

---

## Technical Context & Baseline

- **Language / Runtime**: Swift 6.0 (`swift-tools-version: 6.0`), C11 (`-O3 -Wall -Wextra -Wvla -Wformat=2`).
- **Target Platform**: macOS 14.0+ (ARM64 Apple Silicon NEON & PMULL + x86_64).
- **Core Architecture**: 100% in-process static C bindings (`CTTZipBridge`), Swift 6 strict concurrency (`TTZipCore`, `TTZipApp`, `TTZipCLI`).

---

## Research Items

### R001: C Bridge Memory Safety & Defect Remediation

#### Problem Statement
Analyze memory safety flaws in `Sources/CTTZipBridge/`, specifically the double-free in `CTTZipBridge_Archive.c`, out-of-bounds heap write on `realloc` failure in `ttzip_7z_header_parser.c`, unchecked `pwrite_all` returns in `CTTZipBridge_ZipWriterCore.c`, and descriptor leaks.

#### Findings
1. **Double Free in `CTTZipBridge_Archive.c:132, 162`**:
   - `ttzip_archive_inspect_entries_v2` frees `file_mem` at line 132 upon snappy decompression success. If subsequent archive reading fails, line 162 frees `file_mem` again.
   - Fix: Set `file_mem = NULL;` immediately after `free(file_mem);` at line 132.
2. **Out-of-Bounds Heap Write on `realloc` in `ttzip_7z_header_parser.c:218-225, 252-258, 392-399, 408-415`**:
   - Capacity variables are multiplied and reassigned before verifying `realloc` success. If `realloc` fails (returns NULL), the capacity counter has grown while the buffer remains at the old size, leading to buffer overflow.
   - Fix: Use temporary `new_cap` and `new_arr` variables; only update capacity and pointer if `new_arr != NULL`.
3. **Unchecked `pwrite_all` Returns in `CTTZipBridge_ZipWriterCore.c:245-393`**:
   - Return values of `pwrite_all` are discarded. Disk-full (`ENOSPC`) or I/O failure results in silent truncated write.
   - Fix: Check `if (pwrite_all(...) < 0) { close(out_fd); return TTZIP_ERR_IO; }`.
4. **File Descriptor Leaks in `ttzip_7z_header_writer.c:115` & `CTTZipBridge_7zSolid.c:208`**:
   - Memory allocation failures in metadata serialization leave open file descriptors.
   - Fix: Ensure `close(out_fd)` on all early return branches.

#### Decision
Implement direct nullification on freed pointers, transactional buffer resizing on `realloc`, comprehensive POSIX I/O return validation, and deterministic descriptor closure on all error paths.

#### Rationale
Guarantees strict compliance with the Four Systemic Engineering Invariants (Zero Memory Corruption, Invariant-First, Bounds-First).

#### Alternatives Considered
- *Alternative*: Ignore OOM branches on macOS assuming virtual memory overcommit always succeeds.
  - *Reason for Rejection*: Overcommit does not guarantee allocation success on constrained devices or under large archive header parsing; violating bounds safety is unacceptable.

#### Source
- `Sources/CTTZipBridge/CTTZipBridge_Archive.c:122-163`
- `Sources/CTTZipBridge/ttzip_7z_header_parser.c:218-420`
- `Sources/CTTZipBridge/CTTZipBridge_ZipWriterCore.c:240-400`
- `Sources/CTTZipBridge/ttzip_7z_header_writer.c:110-130`
- `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c:170-210`

---

### R002: Cryptographic Secret Scrubbing & Entropy Validation

#### Problem Statement
Investigate lingering secrets (stack passwords, AES round keys, plaintext decryption heap memory) and CSPRNG error fallbacks in `Sources/CTTZipBridge/`.

#### Findings
1. `CTTZipBridge_7z.c:314-325`: Stack buffer `char pass_arg[512]` holds plaintext password.
2. `CTTZipBridge_Crypto.c:50-68`: `ttzip_aes256_expand_keys` leaves `uint32_t w[60]` expanded round keys on the stack.
3. `CTTZipExtract.c:123-128`: AES decryption failure frees `decrypted_buf` without zeroing.
4. `ttzip_7z_kdf_arm64.c:138-140`: Falls back to static `0x5A` IV on `SecRandomCopyBytes` failure.

#### Decision
Apply `ttzip_secure_zero` to all temporary secret buffers upon exit, and return `TTZIP_ERR_GENERIC` on CSPRNG failures rather than falling back to static nonces.

#### Rationale
Prevents secret recovery from unscrubbed stack memory and ensures cryptographic security invariants are preserved.

#### Alternatives Considered
- *Alternative*: Standard `memset(buf, 0, len)`.
  - *Reason for Rejection*: Standard `memset` at function scope end is prone to compiler Dead-Store Elimination (DSE). `ttzip_secure_zero` uses `memset_s` and memory barrier intrinsics to guarantee physical clearing.

#### Source
- `Sources/CTTZipBridge/CTTZipBridge_7z.c:314-375`
- `Sources/CTTZipBridge/CTTZipBridge_Crypto.c:50-70`
- `Sources/CTTZipBridge/CTTZipExtract.c:120-130`
- `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c:135-145`

---

### R003: UI Runloop Offloading & Concurrency Responsiveness

#### Problem Statement
Audit UI view models for main thread blocking operations and asynchronous race conditions in state updates.

#### Findings
1. `PasswordVaultViewModel.swift:83-94`: PBKDF2 master key derivation (10,000 iterations) executes synchronously on `@MainActor`, blocking UI for ~100ms.
2. `ArchiveTreeStore.swift:54-65`: Rapid sequential tree updates can resume out of order, overwriting newer tree state with older data.

#### Decision
1. Offload `repository.unlock` in `PasswordVaultViewModel` to `Task.detached(priority: .userInitiated)` and publish results back to `@MainActor`.
2. Add a monotonic `generationID: UInt64` in `ArchiveTreeStore` to discard out-of-order background results.

#### Rationale
Ensures 60fps smooth UI responsiveness and deterministic state consistency under rapid user interaction.

#### Alternatives Considered
- *Alternative*: Reduce PBKDF2 iterations to 1,000 to minimize main thread latency.
  - *Reason for Rejection*: Weakens cryptographic defense against brute-force attacks. The correct architecture is offloading to background threads.

#### Source
- `Sources/TTZipApp/ViewModels/PasswordVaultViewModel.swift:80-100`
- `Sources/TTZipApp/ViewModels/ArchiveTreeStore.swift:50-70`
