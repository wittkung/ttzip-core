# Tasks: Deep Code Quality, Memory Safety, and Invariant Hardening

**Feature Branch**: `157-deep-code-quality-and-memory-safety-hardening` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

---

## Dependencies & Execution Graph

```
[Phase 1: Setup] ──► [Phase 2: Foundational]
                             │
                             ▼
              [Phase 3: User Story 1 (P1: Memory Safety & C Bridge)]
                             │
                             ▼
              [Phase 4: User Story 2 (P2: Cryptographic Scrubbing)]
                             │
                             ▼
              [Phase 5: User Story 3 (P3: Concurrency & UI Runloop)]
                             │
                             ▼
              [Phase 6: Polish & Full Verification]
```

---

## Phase 1: Setup & Environment Baseline

- [x] T001 Verify compiler flags and dependency targets in `Package.swift`

---

## Phase 2: Foundational Prerequisites

- [x] T002 Verify `ttzip_secure_zero` and `ttzip_mul_overflow` header declarations in `Sources/CTTZipBridge/include/CTTZipCommon.h`

---

## Phase 3: User Story 1 - Elimination of Critical C Bridge Memory & Lifecycle Vulnerabilities (Priority: P1)

**Goal**: Eliminate double-free vulnerabilities, OOM heap overwrites, silent I/O disk-full corruptions, and descriptor leaks.

**Independent Test**: Build all targets and run archive inspection and writer error branch tests.

- [x] T003 [P] [US1] Fix double-free vulnerability in `Sources/CTTZipBridge/CTTZipBridge_Archive.c` by setting `file_mem = NULL;` after free
- [x] T004 [P] [US1] Fix out-of-bounds heap write on `realloc` failure in `Sources/CTTZipBridge/ttzip_7z_header_parser.c`
- [x] T005 [P] [US1] Guard `pwrite_all` returns and fail with `TTZIP_ERR_IO` on error in `Sources/CTTZipBridge/CTTZipBridge_ZipWriterCore.c`
- [x] T006 [P] [US1] Add descriptor closure `close(out_fd)` on error paths in `Sources/CTTZipBridge/ttzip_7z_header_writer.c` and `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c`
- [x] T007 [P] [US1] Add null check and synchronous fallback for chunk allocations in `Sources/CTTZipBridge/ttzip_threadpool.c`

---

## Phase 4: User Story 2 - Cryptographic Memory Scrubbing & Secret Defense (Priority: P2)

**Goal**: Ensure stack passwords, round keys, and decryption buffers are wiped with `ttzip_secure_zero`, and eliminate static IV fallbacks.

**Independent Test**: Verify memory scrubbing calls in crypto session and decompression code paths.

- [x] T008 [P] [US2] Wipe stack password arguments with `ttzip_secure_zero` in `Sources/CTTZipBridge/CTTZipBridge_7z.c`
- [x] T009 [P] [US2] Wipe expanded AES round keys with `ttzip_secure_zero` in `Sources/CTTZipBridge/CTTZipBridge_Crypto.c`
- [x] T010 [P] [US2] Wipe `decrypted_buf` with `ttzip_secure_zero` on decryption failure in `Sources/CTTZipBridge/CTTZipExtract.c`
- [x] T011 [P] [US2] Remove static IV fallback and return `TTZIP_ERR_GENERIC` on CSPRNG failure in `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c`

---

## Phase 5: User Story 3 - Concurrency Safety & Main UI Runloop Responsiveness (Priority: P3)

**Goal**: Offload heavy PBKDF2 operations to background workers and prevent asynchronous race conditions.

**Independent Test**: Verify non-blocking vault unlock and monotonic generation handling in tree stores.

- [x] T012 [P] [US3] Offload PBKDF2 vault unlocking to background task in `Sources/TTZipApp/ViewModels/PasswordVaultViewModel.swift`
- [x] T013 [P] [US3] Introduce monotonic `generationID` in `Sources/TTZipApp/ViewModels/ArchiveTreeStore.swift` to prevent out-of-order state overwrites
- [x] T014 [P] [US3] Fix potential task registration race condition in `Sources/TTZipCore/ConcurrencyPatterns/ArchiveWorkerPool.swift`

---

## Phase 6: Polish & Verification

**Goal**: Full compilation, static analysis, and regression suite execution.

- [x] T015 Run full compilation and regression test suite (`swift build --build-tests` and `swift test`)
- [x] T016 Perform cross-artifact convergence and consistency analysis
