# Tasks: LZ4 Upstream Partial Decompression Guard

- [x] T001 [P] [US1] Expand `tests/decompress-partial.c` to cover 1..N byte prefix extraction matrix
- [x] T002 [US1] Apply short-circuit guard in `lib/lz4.c` safe decode loop shortcut
- [x] T003 [US1] Verify GNU Make build and full test suite (`test-lz4`, `test-frametest`)
- [x] T004 [US1] Verify CMake build and warning-free compilation
- [x] T005 [US1] Perform 18-item Pre-Flight audit and draft PR Description
