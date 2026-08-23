# Error Diagnostics Contract

**Interface**: `ttzip_rust_last_error_message` & `ttzip_rust_clear_last_error`  
**Language Boundary**: C-ABI (Rust `extern "C"` ↔ Swift CTTZipBridge)

## Contract Guarantees

1. **Thread-Local Isolation**: Each OS thread maintains an isolated `DiagnosticErrorContext`. Errors on Worker Thread A never overwrite or affect Thread B.
2. **Zero-Allocation Error Storage**: The diagnostic buffer is a fixed 512-byte inline stack/thread-local array. Generating an error message will NEVER trigger heap allocations (`malloc`), guaranteeing that error reporting succeeds even under `ErrOutOfMemory`.
3. **Null-Safety**:
   - When the previous operation on the calling thread succeeded (`status == TTZIP_STATUS_OK`), `ttzip_rust_last_error_message()` returns `NULL`.
   - When an error occurred (`status < 0`), it returns a non-null, null-terminated UTF-8 C string.
4. **Lifetime**: The returned `const char*` pointer is valid until the next FFI call on the same thread or until `ttzip_rust_clear_last_error()` is called. Callers must copy the string immediately (e.g. `String(cString:)` in Swift).
