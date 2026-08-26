# Implementation Plan: 193-purge-dead-c-headers-dead-facades-and-linker-cleanup

## Technical Context
- **Objective**: Purge 20+ legacy C headers from `Sources/CTTZipBridge/include/`, 4 dead Facades from `Sources/TTZipCore/Facades/`, and clean `Package.swift` linker settings.

---

## Constitution Check
- [x] **Zero Cloud Actions Quota**: 100% local testing.
- [x] **Single-Source of Truth**: Swift calls Rust C-ABI exclusively via `ttzip_rust_glue.h`.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《彻底清除 CTTZipBridge 废弃 C 头文件》: Completed.
- R002 [SUBAGENT:research] 《剥离空转 Facades 与精简 Package.swift 链接器参数》: Completed.

---

## Phase 1: Purge Dead C Headers
- Delete 20+ obsolete C headers from `Sources/CTTZipBridge/include/`.
- Ensure `module.modulemap` cleanly exports `CTTZipBridge.h` and `ttzip_rust_glue.h`.

## Phase 2: Purge Dead Facades
- Delete `ArchiveOperationsFacade.swift`, `ArchiveSecurityFacade.swift`, `ArchiveStreamingFacade.swift`, and `TTZipEngineFacade+TemplateAndProxies.swift`.

## Phase 3: Package.swift & Final CI Verification
- Clean up `Package.swift` linker flags.
- Run `swift build`, `swift test`, `cargo test --workspace`, and `./scripts/run_local_ci_gate.sh`.
