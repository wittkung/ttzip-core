# Data Model: Deep Code Quality, Memory Safety, and Invariant Hardening

**Feature Branch**: `157-deep-code-quality-and-memory-safety-hardening` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

---

## 1. Domain Entities & Type Definitions

### 1.1 `MemorySafetyAuditRecord`
Represents an audited C bridge memory operation, tracking buffer pointers, lifecycle states, and overflow validation markers.

- **`module_name`** (`string`, minLength: 1, maxLength: 64, required): Name of C source module (e.g., `"CTTZipBridge_Archive.c"`, `"ttzip_7z_header_parser.c"`).
- **`operation_type`** (`string`, enum: `["allocation", "reallocation", "deallocation", "zeroing", "fd_close"]`, required): Category of memory or resource lifecycle action.
- **`buffer_size_bytes`** (`integer`, minimum: 0, required): Size in bytes of allocated, resized, or scrubbed buffer.
- **`is_overflow_checked`** (`boolean`, required): Indicates whether arithmetic overflow (`ttzip_mul_overflow` / `ttzip_add_overflow`) was validated prior to allocation.
- **`is_secure_zeroed`** (`boolean`, required): Indicates whether memory was scrubbed using `ttzip_secure_zero` prior to release.
- **`status`** (`string`, enum: `["success", "oom_handled", "io_error", "corrupt_rejected"]`, required): Result status of the audited operation.

---

### 1.2 `VaultUnlockRequestPayload`
Represents a master password unlock request dispatched to the password vault repository from the UI.

- **`request_id`** (`string`, pattern: "^[0-9a-fA-F-]{36}$", required): Unique UUID v4 for the unlock request.
- **`timestamp_ms`** (`integer`, minimum: 0, required): Unix timestamp in milliseconds when request was initiated.
- **`iteration_count`** (`integer`, minimum: 1000, maximum: 100000, required): PBKDF2 key derivation iteration count.
- **`is_async_offloaded`** (`boolean`, required): Confirms whether execution is detached from the main UI thread.

---

### 1.3 `VaultUnlockResponsePayload`
Represents the result of an asynchronous vault unlock operation returned to the `@MainActor`.

- **`request_id`** (`string`, pattern: "^[0-9a-fA-F-]{36}$", required): UUID matching the corresponding request.
- **`success`** (`boolean`, required): `true` if master password unlocked the vault, `false` otherwise.
- **`duration_ms`** (`number`, minimum: 0.0, required): Execution duration in milliseconds of the background key derivation.
- **`error_code`** (`string`, enum: `["NONE", "INVALID_PASSWORD", "CORRUPTED_VAULT", "CANCELLED"]`, required): Failure classification code.

---

## 2. Invariants & Lifecycle State Transitions

### 2.1 Bounded Dynamic Array Resizing (7z Parser)
```
[Read New Element Count / Size]
              │
              ▼
[Calculate Temporary New Capacity: new_cap]
              │
              ▼
[Overflow Guard: ttzip_mul_overflow(sizeof(T), new_cap, &bytes)]
        ┌─────┴─────┐
     (Pass)       (Fail)
        ▼           ▼
[Call realloc()] [Abort: Return TTZIP_ERR_OUT_OF_MEMORY]
        ┌─────┴─────┐
  (Non-Null)     (Null)
        ▼           ▼
[Commit new_cap] [Preserve old cap: Return TTZIP_ERR_OUT_OF_MEMORY]
        │
        ▼
[Write Element at index++]
```

### 2.2 Async Vault Unlock Pipeline
```
[User Submits Master Password in UI]
              │
              ▼
[PasswordVaultViewModel.unlockVault()]
              │
              ▼
[Detach Task.detached(priority: .userInitiated)]
              │
              ▼
[PBKDF2 SHA-256 Key Derivation on Background Worker]
              │
              ▼
[Publish Result back to @MainActor runloop]
              │
              ▼
[Update isUnlocked & dismiss prompt smoothly (60 FPS)]
```
