# Implementation Plan: 148-frontend-c-wiring-and-swift-slimming

## Technical Context
- **Target Subsystems**: `Sources/TTZipApp/Services/DiskItemSorter.swift`, `Sources/TTZipApp/Services/MediaPreviewFactory.swift`, `Sources/TTZipApp/ViewModels/ArchiveTreeStore.swift`.
- **C Microkernels**: `ttzip_strnatcmp.c`, `ttzip_magic_sniff.c`, `ttzip_archive.c` (`ttzip_archive_extract_entry_mem`), `NativeMicrokernelBridge.swift`.

## Constitution Check
- **Zero GCD Violations**: Maintained.
- **Zero Memory Leaks**: Verified.
- **100% Local CI**: Verified.

## Phase 0: Outline & Research
- - R001 [SUBAGENT:research] 《C11 Natural Numeric Sort Integration in SwiftUI List》: Replacing `localizedStandardCompare` with `ttzip_strnatcasecmp`.
- - R002 [SUBAGENT:research] 《0-Disk-IO Instant Preview Stream Factory》: Serving images, PDF, and audio from in-memory NSData without temporary disk staging.

## Phase 1: Design & Contracts
- `contracts/frontend-c-bridge-contract.json`
- `data-model.md`
- `quickstart.md`
