# Upstream Research & Community Investigation: LZ4 Partial Decompression Guard

## R001: Community Context & Downstream Invariants
- **Decision**: Introduce compile-time constant short-circuit guard `if (partialDecoding && (op >= oend)) break;` in `LZ4_decompress_generic` safe loop shortcut.
- **Rationale**: Downstream engines (EROFS, RocksDB, ClickHouse) rely heavily on `LZ4_decompress_safe_partial` for fast metadata probing. In CVE-2022-49078 and Issue #1172, maintainers established that once requested output size is satisfied, the decompressor must not probe subsequent input bytes.
- **Alternatives Considered**: Modifying `LZ4_FAST_DEC_LOOP` fastloop distance check (rejected: unsafe for small buffer targets < 64 bytes).
- **Source**: `Vendor/worktrees/lz4/partial-guard/lib/lz4.c`, GitHub issues #1172, #929.

## R002: Upstream Coding Style & C90 Constraints
- **Decision**: Follow vanilla C90 with `/* ... */` comment format, 4 spaces indent, zero C99 declarations, and `-Wc++-compat` compliance.
- **Rationale**: `CODING_STYLE` mandates strict C90 compliance for `lib/` directory so third-party C and C++ projects can drop in `lz4.c` without build changes.
- **Alternatives Considered**: Using `//` comments (rejected: violates C90 and fails automated CI linters).
- **Source**: `Vendor/worktrees/lz4/partial-guard/CODING_STYLE`, `.clang-format`.
