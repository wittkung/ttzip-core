# Tasks: Consolidate Single-Core Deflate Engine on libdeflate and Modernize Architecture

**Feature Branch**: `137-libdeflate-single-core-consolidation`

**Date**: 2026-08-20

## Phase 1: Setup & Codec Baseline (Foundational)

- [x] T001 [P] [US1] Verify libdeflate C symbol linking and thread-local caching in Sources/CTTZipBridge/CTTZipStreamCoder.c
- [x] T002 [P] [US1] Verify Swift 6 LibdeflateCAdapter interface and MemoryPageFlyweightPool in Sources/TTZipCore/Adapters/LibdeflateCAdapter.swift

---

## Phase 2: Engine Routing & Pipeline Verification

- [x] T003 [P] [US1] Verify ZipBlockParallelCompressor chunk routing to LibdeflateCAdapter in Sources/TTZipCore/Zip/ZipBlockParallelCompressor.swift
- [x] T004 [P] [US2] Verify ZipBlockParallelDecompressor routing to LibdeflateCAdapter in Sources/TTZipCore/Zip/ZipBlockParallelDecompressor.swift
- [x] T005 [P] [US3] Verify DeflateStreamEngine zlib-ng streaming pipeline isolation in Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift

---

## Phase 3: Architecture Documentation & Subsystem Governance

- [x] T006 [P] [US4] Update Section 2.5 of ARCHITECTURE.md with dual-tier Deflate topology and single-core consolidation rationale in ARCHITECTURE.md
- [x] T007 [P] [US4] Update doc comments and research oracle notes in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c

---

## Phase 4: Quality & Regression Verification

- [x] T008 [US1] Execute LibdeflateCAdapterTests to verify whole-buffer and chunked roundtrips in Tests/TTZipTests/LibdeflateCAdapterTests.swift
- [x] T009 [US2] Execute ZipBlockParallelTests and full suite regression verification via swift test in Tests/TTZipTests/
