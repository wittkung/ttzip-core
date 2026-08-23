# Tasks: Test Suite Architecture Governance, Target Isolation & Unified Corpus Infrastructure

**Feature Directory**: `specs/135-test-suite-governance-and-target-isolation`  
**Target Subject**: 语料基础设施统一收敛、动态二进制解析器与 CI 性能门禁  
**Status**: Completed  

---

## Phase 1: Setup & Unified Corpus Infrastructure (Priority: P1)

- [x] T001 [P] [US1] Add streaming disk-write extension `writeToDisk(filePath:totalBytes:chunkSize:)` in `Sources/TTZipCore/Benchmark/BenchmarkCorpusGenerator.swift`
- [x] T002 [P] [US1] Refactor `Sources/TTZipCore/Benchmark/EnwikFixtureCacheManager.swift` to delegate fallback generation to `BenchmarkCorpusType.text`
- [x] T003 [P] [US1] Clean up legacy independent generators in `Sources/TTZipCore/Benchmark/SyntheticXmlCorpusGenerator.swift` and `Sources/TTZipCore/Benchmark/MultiModalDatasetGenerator.swift`

---

## Phase 2: User Story 3 - Dynamic System Binary Resolver (Priority: P1)

- [x] T004 [P] [US3] Implement unified 5-tier `SystemBinaryResolver` in `Sources/TTZipCore/Platform/SystemBinaryResolver.swift` conforming to `contracts/system_binary_resolver_config.json`
- [x] T005 [P] [US3] Clean up hardcoded `/opt/homebrew/bin/zstd` in `Tests/TTZipTests/TarZstParetoFrontierPkTests.swift`
- [x] T006 [P] [US3] Clean up hardcoded `/opt/homebrew/bin/` paths in `Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift` and `Tests/TTZipTests/ComprehensiveCorpusBenchmarkPkTests.swift`

---

## Phase 3: User Story 2 & 4 - Benchmark Gating & CI Audit Gate Verification (Priority: P2)

- [x] T007 [P] [US2] [US4] Update `scripts/upstream_audit_gate.py` to assert the 50-point in-memory matrix with 3D latency metrics
- [x] T008 [US2] Run full test suite `swift test` and confirm total execution time $\le 2.5$ seconds with 0 failures

---

## Phase 4: Convergence & Final Sign-off

- [x] T009 Verify all 16 checklist requirements in `checklists/requirements.md`
- [x] T010 Run `@speckit-converge` and `@speckit-analyze` consistency checks
