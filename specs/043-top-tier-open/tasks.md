# Tasks: 043-top-tier-open (TTZip 顶尖开源工程对标与架构硬化改造)

**Feature Directory**: `specs/043-top-tier-open`  
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/043-top-tier-open/spec.md)  
**Plan**: [plan.md](file:///Users/kevintung/Documents/dev/TTZip/specs/043-top-tier-open/plan.md)  
**Status**: Ready for Implementation  

---

## Phase 1: Setup & Foundational Infrastructure

**Purpose**: 验证 Schema 契约完备性与工程基础环境

- [x] T001 [Setup] Assert Schema and Contract integrity for feature 043 in `specs/043-top-tier-open/contracts/`
- [x] T002 [P] [Setup] Configure `.swiftlint.yml` strict code quality rules in `.swiftlint.yml`

---

## Phase 2: User Story 1 - SPM 模块化与零 UnsafeFlags 依赖分发 (Priority: P1) 🎯 MVP

**Goal**: 清除 `Package.swift` 中所有绝对路径计算与 `.unsafeFlags`，使 `TTZipCore` 具备作为独立 Swift Package 远端依赖发布的标准能力。

- [x] T003 [US1] Refactor `Package.swift` to eliminate all `.unsafeFlags` and `#filePath` absolute path lookups in `Package.swift`
- [x] T004 [P] [US1] Verify clean standard build without compiler warnings in `Package.swift`

---

## Phase 3: User Story 2 - 内存安全确界与 Swift 6 并发模型硬化 (Priority: P1)

**Goal**: 实现基于 ARC/RAII 的不可变只读 `MmapBufferHandle: Sendable`，彻底消除裸指针跨线程访问与 `@unchecked Sendable` 逃逸。

- [x] T005 [P] [US2] Implement ARC/RAII `MmapBufferHandle: Sendable` with bounds-checked slice access in `Sources/TTZipCore/Adapters/MmapBufferHandle.swift`
- [x] T006 [P] [US2] Create unit tests for `MmapBufferHandle` lifecycle and error paths in `Tests/TTZipTests/MmapBufferHandleTests.swift`
- [x] T007 [US2] Integrate `MmapBufferHandle` into `ZipParallelExtractor` to eliminate raw pointer dereferences and `defer munmap` races in `Sources/TTZipCore/Zip/ZipParallelExtractor.swift`


---

## Phase 4: User Story 3 - 工业级 CI/CD 全量流水线与 Sanitizer 矩阵 (Priority: P2)

**Goal**: 重构 GitHub Actions CI/CD 流水线，添加 PR 触发、SwiftLint 门禁、全量 95+ 测试套件并行运行及 AddressSanitizer/ThreadSanitizer 动态矩阵。

- [x] T008 [US3] Refactor `.github/workflows/ci-cd.yml` with PR triggers, concurrency cancel-in-progress, and full test matrix in `.github/workflows/ci-cd.yml`
- [x] T009 [P] [US3] Configure AddressSanitizer and ThreadSanitizer matrix jobs in `.github/workflows/ci-cd.yml`


---

## Phase 5: User Story 4 - 代码库卫生清道与 libarchive 黄金语料库深度对齐 (Priority: P2)

**Goal**: 清理根目录 ad-hoc 测试垃圾文件，实现纯内存 `LibarchiveUUDecoder`，并接入 30+ 官方黄金语料样本与原语断言库。

- [x] T010 [P] [US4] Clean up all ad-hoc root test files and update `.gitignore` in `.gitignore`
- [x] T011 [P] [US4] Implement in-memory `LibarchiveUUDecoder` for pure memory fixture extraction in `Tests/TTZipTests/LibarchiveUUDecoder.swift`
- [x] T012 [P] [US4] Implement POSIX filesystem and memory assertion primitives `TTZipAssertions` in `Tests/TTZipTests/TTZipAssertions.swift`
- [x] T013 [US4] Implement `LibarchiveGoldenCorpusTests` covering 30+ representative `.uu` golden fixtures in `Tests/TTZipTests/LibarchiveGoldenCorpusTests.swift`
- [x] T014 [P] [US4] Upgrade `ArchiveMutationFuzzTests` to in-memory crash-first fuzzing runner in `Tests/TTZipTests/ArchiveMutationFuzzTests.swift`


---

## Phase 6: Polish, Verification & Performance Gate (Priority: P1)

**Goal**: 执行全量回归、门禁验证与零倒退审查。

- [x] T015 Run full parallel test suite regression (`swift test --parallel`) across all 95+ test files
- [x] T016 Verify zero-regression against 13 hard performance gates in `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`
