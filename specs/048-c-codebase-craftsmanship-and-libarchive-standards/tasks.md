# Tasks: 048-c-codebase-craftsmanship-and-libarchive-standards

**Feature Branch**: `048-c-codebase-craftsmanship-and-libarchive-standards`  
**Input Documents**: [spec.md](./spec.md), [plan.md](./plan.md), [data-model.md](./data-model.md), [research.md](./research.md), [quickstart.md](./quickstart.md), [contracts/](./contracts/)  

---

## Phase 1: Setup & Contracts Verification

- [x] T001 [P] 验证并核对 JSON Schema 契约在 `specs/048-c-codebase-craftsmanship-and-libarchive-standards/contracts/` 下的零通配与字段一致性

---

## Phase 2: User Story 1 - C 头文件全量四维契约与 Doxygen 自解释规范化 (Priority: P1)

*Goal*: 对 `CTTZipCommon.h`, `CTTZipSysAlloc.h`, `CTTZipIO.h`, `CTTZipBridge_Archive.h` 等进行自解释注释重构。  
*Independent Test*: 编译 0 警告并通过静态分析。

- [x] T002 [P] [US1] 重构 `Sources/CTTZipBridge/include/CTTZipCommon.h` 与 `Sources/CTTZipBridge/include/CTTZipSysAlloc.h`
- [x] T003 [P] [US1] 重构 `Sources/CTTZipBridge/include/CTTZipIO.h` 与 `Sources/CTTZipBridge/include/CTTZipBridge_Archive.h`

---

## Phase 3: User Story 2 - C 实现文件内存所有权、Arena 析构与 64 位 Clamp 加固 (Priority: P1)

*Goal*: 修复 Arena 内部指针释放崩溃、I/O 缓冲区泄漏、重复 APFS 预分配与整型溢出。  
*Independent Test*: 运行 `swift test --filter TarNativeEngineTests,MmapBufferHandleTests` 全部通过。

- [x] T004 [P] [US2] 重构 `Sources/CTTZipBridge/CTTZipCommon.c`, `Sources/CTTZipBridge/CTTZipSysAlloc.c`, `Sources/CTTZipBridge/CTTZipIO.c`
- [x] T005 [P] [US2] 重构 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`，统一 Arena 安全析构

---

## Phase 4: User Story 3 - 并发管道死锁消除与原子错误传播 (Priority: P2)

*Goal*: 修复 `CTTZipBridge_GzParallel.c` 失败分支条件变量广播死锁，统一错误传播。  
*Independent Test*: 运行 `swift test --filter "(TarNativeEngineTests|SevenZipBridgeTests)"` 全部通过。

- [x] T006 [P] [US3] 重构 `Sources/CTTZipBridge/CTTZipBridge_GzParallel.c`

---

## Phase 5: Verification & Polish

- [x] T007 运行全量 589+ 单元测试与本地 CI 流水线 `./scripts/run_local_ci.sh --quick` 验证零回归
