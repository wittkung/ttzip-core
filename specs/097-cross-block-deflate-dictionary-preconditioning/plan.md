# Implementation Plan: 097-cross-block-deflate-dictionary-preconditioning

## Technical Context
- **Layer**: `Sources/CTTZipBridge/CTTZipStreamCoder.c`, `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`.
- **Target OS**: macOS 14.0+ on Apple Silicon & Intel.
- **RFC Standard**: RFC 1951 (DEFLATE Compressed Data Format Specification).

---

## Phase 0 & 1 Index
- Phase 0: `research.md` (R001: 32KB Sliding Window Injection, R002: TLS stream pool).
- Phase 1: `data-model.md`, `contracts/cross-block-dict-schema.json`, `quickstart.md`.

---

## Phase 2: Implementation & Verification
1. `Tests/TTZipTests/CrossBlockDeflateDictionaryTests.swift`: Unit & ratio gain tests with system oracle consensus.
2. Full regression and performance gate verification.
