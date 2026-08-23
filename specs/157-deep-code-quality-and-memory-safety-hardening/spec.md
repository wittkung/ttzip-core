# Feature Specification: Deep Code Quality, Memory Safety, and Invariant Hardening

**Feature Branch**: `157-deep-code-quality-and-memory-safety-hardening`
**Created**: 2026-08-20
**Status**: Clarified
**Input**: User description: "全面审计和优化代码质量 /speckit-specify /goal"

---

## Clarifications

### Session 2026-08-20
- Q: What are the primary defect categories targeted in this deep hardening phase? → A: P0/P1 memory safety & lifecycle bugs (double free, OOM heap overwrite, deadlock in parallel streams, fd leaks, unchecked IO returns), P1 cryptographic secret scrubbing (`ttzip_secure_zero`), and P2/P3 UI runloop concurrency offloading.
- Q: How should OOM conditions be handled in C bridge parsers and threadpools? → A: Return explicit `TTZIP_ERR_OUT_OF_MEMORY` or `TTZIP_ERR_IO`, clean up all allocated memory without mutating capacity counters prematurely, and close open file descriptors.
- Q: How should PBKDF2 key derivation be handled in the GUI app? → A: Offload PBKDF2 computation to `Task.detached(priority: .userInitiated)` and publish state back to `@MainActor` to prevent main UI freezing.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Elimination of Critical C Bridge Memory & Lifecycle Vulnerabilities (Priority: P1)

As a security and systems engineer, I need all C bridge code (`CTTZipBridge`) to have zero double-free hazards, zero out-of-bounds heap overwrites on allocation failure, proper error returns on disk full, closed file descriptors on error branches, and null-checked thread pool dispatches, so that the core archiving engine never crashes or corrupts data even under extreme stress or hostile input.

**Why this priority**: Memory corruptions, double frees, and silent disk-full corruptions are fatal defects that violate TTZip Engineering Constitution invariants.

**Independent Test**: Audit and execute unit tests covering error branches, inspection of corrupt archives, and parallel stream teardown; verify clean error handling with zero crashes or leaks.

**Acceptance Scenarios**:
1. **Given** a corrupted Snappy/TAR archive during `ttzip_archive_inspect_entries_v2`, **When** extraction fails, **Then** memory is freed exactly once with zero double-free crashes.
2. **Given** an OOM condition during 7z header parsing in `ttzip_7z_parse_header_from_memory`, **When** `realloc` returns NULL, **Then** the parser aborts cleanly without writing past the original buffer capacity.
3. **Given** an I/O error or full disk during `ttzip_write_zip_archive_disk`, **When** `pwrite_all` fails, **Then** the engine immediately cleans up, closes descriptors, and returns `TTZIP_ERR_IO`.

---

### User Story 2 - Cryptographic Memory Scrubbing & Secret Defense (Priority: P2)

As a security auditor and user, I want all sensitive credentials (passwords, expanded AES-256 round keys, intermediate decryption plaintext buffers) to be deterministically scrubbed with `ttzip_secure_zero` upon function exit or error paths, and ensure CSPRNG failures immediately abort rather than falling back to static IVs.

**Why this priority**: Plaintext secrets in stack/heap memory or static IV fallbacks compromise cryptographic confidentiality.

**Independent Test**: Inspect crypto key expansion, session initialization, and entry decryption routines; verify `ttzip_secure_zero` is invoked on all exit branches.

**Acceptance Scenarios**:
1. **Given** command-line password arguments in `CTTZipBridge_7z.c`, **When** the subprocess spawns or fails, **Then** the stack password buffer is scrubbed with `ttzip_secure_zero`.
2. **Given** AES round key expansion in `CTTZipBridge_Crypto.c`, **When** key derivation completes, **Then** the expanded key array on stack is zeroed.
3. **Given** a CSPRNG failure in `ttzip_7z_crypto_session_init`, **When** `SecRandomCopyBytes` fails, **Then** the session initialization aborts with an error instead of using a static IV.

---

### User Story 3 - Concurrency Safety & Main UI Runloop Responsiveness (Priority: P3)

As an interactive desktop user, I want heavy cryptographic key derivations (PBKDF2) and file tree mutations to execute on background threads without blocking the `@MainActor` UI runloop, and ensure asynchronous tree updates never overwrite newer user states with stale data.

**Why this priority**: Synchronous key derivation freezes the UI and degrades user experience.

**Independent Test**: Execute `PasswordVaultViewModel.unlockVault()` and verify main thread responsiveness, and verify generation-tagged updates in `ArchiveTreeStore`.

**Acceptance Scenarios**:
1. **Given** a master password entry in `PasswordVaultViewModel`, **When** `unlockVault()` is invoked, **Then** PBKDF2 runs in a background task and UI state updates smoothly on completion.
2. **Given** rapid successive tree updates in `ArchiveTreeStore`, **When** multiple background tasks complete out-of-order, **Then** only the latest generation updates `rootNodes`.

---

## Edge Cases

- How does the 7z header parser handle repeated capacity doublings when file streams are corrupt? Capacity is only updated after `realloc` succeeds; on failure, execution returns `TTZIP_ERR_OUT_OF_MEMORY`.
- What happens if disk space is exhausted during central directory writing in ZIP? `pwrite_all` failure is caught immediately, file descriptor closed, and `TTZIP_ERR_IO` returned.
- How does the worker pool handle a task completing before its handle is registered? Synchronous registration under lock prevents premature unregistration races.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST eliminate the double-free vulnerability in `Sources/CTTZipBridge/CTTZipBridge_Archive.c` by ensuring `file_mem` is nulled after release.
- **FR-002**: System MUST prevent out-of-bounds heap writes in `Sources/CTTZipBridge/ttzip_7z_header_parser.c` by only updating capacity variables after successful `realloc`.
- **FR-003**: System MUST verify return values for all `pwrite_all` calls in `Sources/CTTZipBridge/CTTZipBridge_ZipWriterCore.c` and fail with `TTZIP_ERR_IO` on write failure.
- **FR-004**: System MUST ensure file descriptors are closed on all error paths in `Sources/CTTZipBridge/ttzip_7z_header_writer.c` and `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c`.
- **FR-005**: System MUST null-check dynamic chunk allocations in `Sources/CTTZipBridge/ttzip_threadpool.c` and provide synchronous fallback.
- **FR-006**: System MUST wipe all sensitive password and key arrays with `ttzip_secure_zero` in `Sources/CTTZipBridge/CTTZipBridge_7z.c`, `Sources/CTTZipBridge/CTTZipBridge_Crypto.c`, and `Sources/CTTZipBridge/CTTZipExtract.c`.
- **FR-007**: System MUST reject static IV fallbacks in `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c` upon CSPRNG failure.
- **FR-008**: System MUST offload PBKDF2 vault unlocking in `Sources/TTZipApp/ViewModels/PasswordVaultViewModel.swift` to background tasks.
- **FR-009**: System MUST prevent stale out-of-order state overwrites in `Sources/TTZipApp/ViewModels/ArchiveTreeStore.swift` using monotonic generation counters.
- **FR-010**: System MUST pass all automated regression and performance test suites with 100% green status.

---

### Key Entities

- **CryptoSession**: Cryptographic state management entity with deterministic zeroing and CSPRNG entropy validation.
- **HeaderParserState**: 7z metadata parsing context with bounded dynamic array allocation and verified capacity invariants.
- **ZipWriterContext**: Core ZIP output state manager enforcing POSIX I/O return verification.
- **PasswordVaultViewModel**: UI view model managing secure master password validation offloaded from the main actor.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Zero double-free, out-of-bounds write, or memory safety defects detected across C bridge audit.
- **SC-002**: 100% of sensitive cryptographic buffers wiped with `ttzip_secure_zero` before stack/heap deallocation.
- **SC-003**: 0ms main thread blocking during master password PBKDF2 unlocking in `PasswordVaultViewModel`.
- **SC-004**: 100% pass rate across all automated unit and integration tests (`swift test`).
- **SC-005**: Zero compiler warnings across all 5 targets in Swift 6.0 and C11 builds.

---

## Assumptions

- Operating environment is macOS 14.0+ on Apple Silicon ARM64 and Intel x86_64.
- `ttzip_secure_zero` provides DSE (Dead-Store Elimination) immunity via `memset_s` and compiler memory barriers.
- All core ZIP parallel engine freeze rules remain observed.
