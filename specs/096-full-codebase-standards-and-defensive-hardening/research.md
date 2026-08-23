# Research Document: 096-full-codebase-standards-and-defensive-hardening

## Phase 0: Research Synthesis

### R001 [SUBAGENT:research] Header Standardization & Hoare Triple Contracts
- **Decision**: Standardize all C bridge headers in `Sources/CTTZipBridge/include/` with explicit Doxygen Hoare Triple tags (`@brief`, `@param[in,out]`, `@return`, `@pre`, `@post`, `@invariant`, `@complexity`, `@threadsafe`).
- **Rationale**: Eliminates cognitive ambiguity for callers and AI agents regarding buffer ownership, validity, alignment constraints, and concurrency guarantees.
- **Alternatives Considered**: Minimal docstrings (`@param`/`@return` only) (rejected because preconditions and thread-safety constraints are essential for native C zero-copy bridges).
- **Source**: Linux Kernel `Documentation/doc-guide/kernel-doc.rst`, Apple Swift DocC specification.

### R002 [SUBAGENT:research] Compiler Warning Flags Zero-Tolerance
- **Decision**: Fix all latent compiler warnings across C bridge source files to achieve clean compilation under `-Wall -Wextra -Wmissing-prototypes -Wstrict-prototypes -Wvla -Wshadow -Wformat=2`.
- **Rationale**: Warnings like unused variables, sign compare mismatches, and unprototyped functions frequently conceal runtime bugs, stack overflows, and ABI mismatches.
- **Alternatives Considered**: Suppressing warnings via `#pragma clang diagnostic ignored` (rejected as mask rather than cure).
- **Source**: SEI CERT C Coding Standard, Linux Kernel VLA elimination initiative.

### R003 [SUBAGENT:research] Struct Magic Sentinel & Free-Poisoning Architecture
- **Decision**: Embed `uint32_t magic` in all C handle structs (`ttzip_stream_coder_t`, `ttzip_zip_chunked_stream_t`, `ttzip_tar_entry_info_t`, etc.), checking `ctx->magic == TTZIP_STRUCT_MAGIC` on API entry and overwriting with `TTZIP_POISON_FREE (0xDEADBEEFU)` prior to release.
- **Rationale**: Immediately converts silent UAF and double-free memory corruptions into deterministic assertion failures in release builds without ASan overhead.
- **Alternatives Considered**: ASan-only validation in CI (rejected because production release builds run without ASan).
- **Source**: SQLite `src/mem2.c` (`SQLITE_MAGIC`), Linux Kernel `include/linux/poison.h`.

### R004 [SUBAGENT:research] DSE-Immune Memory Eradication
- **Decision**: Apply `ttzip_secure_zero` (using `memset_s` / `explicit_bzero` + assembly memory barrier `__asm__ __volatile__("" : : "r"(ptr) : "memory")`) across all temporary crypto key schedules, PBKDF2 buffers, and password strings.
- **Rationale**: Clang `-O3` treats standard `memset()` as dead-code if the buffer goes out of scope immediately afterward, leaving secrets exposed in memory.
- **Alternatives Considered**: Standard `memset()` (rejected due to compiler dead-store elimination).
- **Source**: OpenSSL `crypto/mem_clr.c` (`OPENSSL_cleanse`), Linux Kernel `lib/string.c` (`memzero_explicit`).
