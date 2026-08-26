# Feature Specification: 181-sink-filter-dsl-inplace-edit-concurrency-and-manifest-verifier

## 1. Executive Summary & Strategic Motivation
This feature represents the sixth and final round of deep non-Rust code sinking and structural unification across TTZip. We sink the remaining 5 major algorithmic and architectural domains from pure Swift into **Safe Rust (`rust/ttzip-glue`)**:
1. **Archive Filter DSL Lexer, Parser & AST Evaluator (`rust/ttzip-glue/src/fs/filter_dsl.rs`)**:
   - High-throughput lexical scanning and AST evaluation for expressions like `name == "*.log" AND size > 1MB OR date < 2026-01-01`.
   - Replaces `ArchiveFilterDSLLexerParser.swift` (440 LOC) with zero-allocation Rust AST execution.
2. **In-Place Atomic Archive Modification Engine (`rust/ttzip-glue/src/archive/in_place_edit.rs`)**:
   - In-place entry append, replacement, and deletion inside ZIP, 7z, and TAR containers without re-compressing untouched blocks.
   - Replaces `InPlaceEditEngine.swift` (325 LOC) and `InPlaceEditSession.swift` (92 LOC).
3. **Differential Manifest Scanner & Golden Corpus Verifier (`rust/ttzip-glue/src/testing/differential.rs`)**:
   - Multi-threaded recursive hashing, POSIX permission/mtime diffing, and differential oracle comparison against system libarchive and unzip.
   - Replaces `DifferentialManifestScanner.swift` (278 LOC), `DifferentialManifestVerifier.swift` (277 LOC), and `LibarchiveGoldenCorpusVerifier.swift` (360 LOC).
4. **Zero-Copy Concurrency Pipelines & Chunk Splitters (`rust/ttzip-glue/src/concurrency/`)**:
   - Lock-free bounded channels, work-stealing thread pools, and cache-friendly chunk splitters replacing heavy Swift locks/semaphores (`DynamicParallelRingBuffer.swift`, `ArchiveWorkerPool.swift`, `BoundedProducerConsumerQueue.swift`, `DynamicChunkSplitter.swift`).
5. **Consolidation & Thinning of Object-Oriented Boilerplate**:
   - Streamlining `ArchiveBuilders.swift`, `ArchiveBatchFacade.swift`, `ArchiveOperationsFacade.swift`, `ArchiveStreamingFacade.swift`, `ConcreteVisitors.swift`, and `ConcreteRepositories.swift` to thin, lightweight C-ABI facades.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Complex Archive Filter DSL Queries
- **Given** an archive with 100,000 files and a complex filter DSL query
- **When** evaluating entries for selective extraction or exclusion
- **Then** the Rust AST engine evaluates all 100,000 entries in $<5\text{ms}$ with zero heap allocation per entry.

### User Scenario 2: In-Place Fast Archive Modification
- **Given** a 5GB ZIP or 7z archive with 500 entries
- **When** updating or removing a 10KB text file in-place
- **Then** the operation completes in $<50\text{ms}$ via in-place Central Directory rewrite without rewriting the untouched 5GB payload.

### User Scenario 3: Differential Oracle & Golden Corpus Verification
- **Given** test suites running differential system oracle checks
- **When** validating TTZip output against libarchive
- **Then** full manifest comparison runs multi-threaded at $>2\text{GB/s}$ in Rust.

---

## 3. Success Metrics
1. **Total Core Sinking**: 100% of Filter DSL, In-Place Editing, Concurrency Queues, and Manifest Verifiers reside in Safe Rust.
2. **SRP & LOC Budget**: 100% of first-party source files maintained at $< 350\sim 500\text{ LOC}$.
3. **Zero Regression**: 100% pass rate across 175+ Rust tests, 885+ Swift tests, and 7/7 local CI stages.

---

## 4. Clarifications
- **Q1: How are Swift Filter DSL types preserved?**
  - **Decision**: Public Swift AST structs (`FilterExpression`, `FilterToken`) remain available for Swift callers, but execution is delegated to `ttzip_rust_eval_filter_dsl`.
- **Q2: How are In-Place Editing transactions managed?**
  - **Decision**: Rust `InPlaceArchiveSession` manages atomic temporary copy-on-write staging and final atomic rename (`renameat2` / `replace_file`).
