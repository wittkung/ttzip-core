# Implementation Plan: Deep Code Quality, Memory Safety, and Invariant Hardening

**Branch**: `157-deep-code-quality-and-memory-safety-hardening` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/157-deep-code-quality-and-memory-safety-hardening/spec.md`

---

## Summary

This plan outlines the systematic remediation of critical C bridge memory safety vulnerabilities (double free, OOM heap overwrites, silent I/O disk-full corruptions, file descriptor leaks), cryptographic secret scrubbing with `ttzip_secure_zero`, and UI runloop offloading for heavy PBKDF2 operations, concluding with full regression test verification.

---

## Technical Context

- **Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`), C11 (`-O3 -Wall -Wextra -Wvla -Wformat=2`).
- **Primary Dependencies**: Native static libraries in `Vendor/` (`libTTZipVendor.a`, `TTZipVendor.xcframework`).
- **Testing**: SPM `swift test` (525+ automated unit and integration tests).
- **Target Platform**: macOS 14.0+ (ARM64 Apple Silicon & x86_64).
- **Performance Goals**: Zero main thread blocking (>0ms lag) on user interaction; 100% throughput floor satisfaction.

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Invariant / Gate | Requirement | Status | Verification Method |
| :--- | :--- | :--- | :--- |
| **1. Stream-First** | Zero unconstrained heap allocations. | PASS | Bounded dynamic array resizing in 7z parser. |
| **2. Invariant-First** | POSIX return verification, overflow-safe arithmetic. | PASS | Guard `pwrite_all` return codes in `CTTZipBridge_ZipWriterCore.c`. |
| **3. Bounds-First** | Sensitive memory wiping, safe pointer nullification. | PASS | `ttzip_secure_zero` on password and key buffers; nullify freed pointers. |
| **4. Oracle-First** | Golden corpus and differential tests pass cleanly. | PASS | Execute full test suite via `swift test`. |
| **5. Logging Discipline** | Zero bare print/printf in production code. | PASS | All errors routed via `TTLogger` or structured return codes. |
| **6. Frozen Files Rule** | Core ZIP parallel freeze observed. | PASS | Modifications limited to non-frozen wrappers and bridge helpers. |

---

## Phase 0: Research Items

- R001 [SUBAGENT:research] 《C Bridge Memory Safety & Defect Remediation》: Investigate double-free in `CTTZipBridge_Archive.c`, OOM heap overwrite in `ttzip_7z_header_parser.c`, unchecked `pwrite_all` returns in `CTTZipBridge_ZipWriterCore.c`, and descriptor leaks.
- R002 [SUBAGENT:research] 《Cryptographic Secret Scrubbing & Entropy Validation》: Investigate lingering secrets on stack/heap and static IV fallbacks in `CTTZipBridge_7z.c`, `CTTZipBridge_Crypto.c`, and `ttzip_7z_kdf_arm64.c`.
- R003 [SUBAGENT:research] 《UI Runloop Offloading & Concurrency Responsiveness》: Investigate PBKDF2 blocking in `PasswordVaultViewModel.swift` and tree race conditions in `ArchiveTreeStore.swift`.

---

## Phase 1: Artifacts & Contracts

- **Data Model**: `specs/157-deep-code-quality-and-memory-safety-hardening/data-model.md`
- **Contracts**:
  - `specs/157-deep-code-quality-and-memory-safety-hardening/contracts/memory-safety-contract.json` [SUBAGENT:research]
  - `specs/157-deep-code-quality-and-memory-safety-hardening/contracts/vault-unlock-contract.json` [SUBAGENT:research]
- **Quickstart Guide**: `specs/157-deep-code-quality-and-memory-safety-hardening/quickstart.md`

---

## Project Structure & Planned Modifications

### Source Code Modifications by Component

#### 1. C Bridge Memory Safety & Descriptor Lifecycle (`Sources/CTTZipBridge/`)
- `Sources/CTTZipBridge/CTTZipBridge_Archive.c`:
  - Nullify `file_mem = NULL;` after `free(file_mem)` at line 132 to prevent double-free on error fallback.
- `Sources/CTTZipBridge/ttzip_7z_header_parser.c`:
  - Refactor capacity updates at lines 218, 253, 393, 409: only commit new capacity after verifying `realloc` return is non-null.
- `Sources/CTTZipBridge/CTTZipBridge_ZipWriterCore.c`:
  - Guard all `pwrite_all` return codes; fail immediately and close `out_fd` if `pwrite_all < 0`.
- `Sources/CTTZipBridge/ttzip_7z_header_writer.c` & `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c`:
  - Add `close(out_fd)` to error return paths.
- `Sources/CTTZipBridge/ttzip_threadpool.c`:
  - Null-check chunk allocations in `ttzip_parallel_for` and provide fallback.

#### 2. Cryptographic Memory Scrubbing (`Sources/CTTZipBridge/`)
- `Sources/CTTZipBridge/CTTZipBridge_7z.c`:
  - Scrub `pass_arg` with `ttzip_secure_zero` upon function completion.
- `Sources/CTTZipBridge/CTTZipBridge_Crypto.c`:
  - Wipe `w` in `ttzip_aes256_expand_keys` with `ttzip_secure_zero`.
- `Sources/CTTZipBridge/CTTZipExtract.c`:
  - Wipe `decrypted_buf` with `ttzip_secure_zero` before freeing on decryption failure.
- `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c`:
  - Return `TTZIP_ERR_GENERIC` on `SecRandomCopyBytes` failure instead of falling back to static IV.

#### 3. Concurrency & UI Responsiveness (`Sources/TTZipApp/` & `Sources/TTZipCore/`)
- `Sources/TTZipApp/ViewModels/PasswordVaultViewModel.swift`:
  - Offload `repository.unlock(masterPassword:)` to a detached background task.
- `Sources/TTZipApp/ViewModels/ArchiveTreeStore.swift`:
  - Introduce monotonic `generationID: UInt64` to prevent stale out-of-order state overwrites.

---

## Complexity Tracking

*No constitutional violations identified. Zero complexity exceptions.*
