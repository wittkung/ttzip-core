# Tasks: 148-frontend-c-wiring-and-swift-slimming

## Phase 1: Natural Sort & Magic Sniffing Wiring (US1 & US2)

- [x] T001 [US1] Update Sources/TTZipApp/Services/DiskItemSorter.swift to use NativeMicrokernelBridge.naturalCompare
- [x] T002 [US2] Update Sources/TTZipApp/Services/MediaPreviewFactory.swift to integrate NativeMicrokernelBridge.sniffMagic and in-memory data previews

## Phase 2: Tree Search & Full Verification (US3)

- [x] T003 [US3] Verify Sources/TTZipApp/ViewModels/ArchiveTreeStore.swift search filtering
- [x] T004 [US1] Run scripts/local-ci.sh to ensure all 76+ Swift tests pass 100% green
