# Tasks: 095-comprehensive-systems-engineering-evolution

## Phase 1: User Story 1 - Defensive C Infrastructure & Memory Safety (Priority: P1)

**Story Goal**: Implement struct magic sentinels, free-poisoning, overflow checking, volatile key zeroing, cacheline alignment, and strict compiler flags.
**Independent Test**: `swift build` and `swift test --filter AlgorithmicOptimizationBenchmarkTests`.

- [x] T001 [P] [US1] Add `ttzip_add_overflow`, `ttzip_mul_overflow`, `TTZIP_CACHELINE_ALIGNED`, `TTZIP_STRUCT_MAGIC`, and `TTZIP_POISON_FREE` macros in `Sources/CTTZipBridge/include/CTTZipCommon.h`
- [x] T002 [P] [US1] Add volatile memory zeroing `ttzip_secure_zero` on stack key schedules in `Sources/CTTZipBridge/CTTZipBridge_Crypto.c`
- [x] T003 [US1] Configure `-fvisibility=hidden`, `-Wall`, `-Wextra`, `-Wmissing-prototypes`, `-Wstrict-prototypes`, `-Wvla`, `-Wshadow`, and `-Wformat=2` in `Package.swift`

---

## Phase 2: User Story 2 - Documentation & Formal Mathematical Invariants (Priority: P1)

**Story Goal**: Embed formal mathematical derivations and Hoare Triple Design-by-Contract annotations into core algorithm files.
**Independent Test**: Code inspection and `swift build`.

- [x] T004 [P] [US2] Embed Barrett polynomial reduction proof and $\mu(x)$ derivation in `Sources/CTTZipBridge/ttzip_crc64.c`
- [x] T005 [P] [US2] Embed quadratic root derivation for $N_{\max} = 5552$ in `Sources/CTTZipBridge/CTTZipAdler32Neon.c`
- [x] T006 [P] [US2] Embed ARM64 26-bit branch offset sign-extension math in `Sources/CTTZipBridge/ttzip_bcj_arm64_neon.c`

---

## Phase 3: User Story 3 - Testing, Differential Oracles & Fuzzing (Priority: P2)

**Story Goal**: Upgrade differential test oracles and verify hostile mutation fuzzing vectors.
**Independent Test**: `swift test --filter DifferentialOracleTests,ArchiveMutationFuzzTests`.

- [x] T007 [P] [US3] Expand `DifferentialOracleTestHarness.swift` and `DifferentialOracleTests.swift` with multi-way consensus checks (`ditto`, `bsdtar`, `7z`, `zipinfo`)
- [x] T008 [P] [US3] Verify 6 hostile mutation vectors in `Tests/TTZipTests/ArchiveMutationFuzzTests.swift`

---

## Phase 4: User Story 4 - Microarchitectural Acceleration & Zero-Regression Gates (Priority: P2 & P3)

**Story Goal**: Implement vector candidate pre-filtering in search and pass all 13 constitutional performance gates.
**Independent Test**: `swift test --filter XCTestPerformanceMeasureTests` and full `swift test`.

- [x] T009 [P] [US4] Implement fast candidate prefix/suffix filtering in `Sources/TTZipCore/Search/ArchiveSearchIndex.swift`
- [x] T010 [US4] Execute full test suite regression and verify all 13 performance throughput floors
