# Spec: Unified Client SDK & TTZipCore Professional Consolidation

**Feature**: `210-unified-client-sdk-consolidation`  
**Classification**: `[Lean SDD]` (Core codebase consolidation into high-cohesion Unified Client SDK)  
**Status**: `IN_PROGRESS`  

---

## 1. Context & Objectives

Following the 100% sinking of core compression algorithms, hardware vector acceleration, in-place edit sessions, and standalone CLI into Rust (`bin/ttzip` and `libttzip_glue.a`), `Sources/TTZipCore` contains historical fragmentation (131 scattered files with redundant OOP strategy hierarchies, fragmented extensions, and boilerplate forwarding).

This feature reorganizes `Sources/TTZipCore` into a modern **Unified Client SDK Architecture** (~25 high-cohesion files):
1. **Bridge**: Explicit C-ABI bindings and memory safety adapters.
2. **Client**: Unified `TTZipEngineFacade`, `ArchiveReader`, `ArchiveWriter`, `ArchiveExtractor`, `ArchiveTreeNode`, `InPlaceArchiveMutationEngine`.
3. **Models**: Compact data models (`ArchiveEntry`, `ArchiveCompressionFormat`, `ArchiveProgress`, `ArchiveFormatStandardRegistry`).
4. **Security & VFS**: `PasswordVaultManager`, `PasswordRecoveryEngine`, `SmartExtractResolver`, `VFSLz4CachePool`.
5. **System & Platform**: `AppleSiliconTuner`, `FinderSyncHelper`, `QuickLookPreviewEngine`, `DeepFileMetadataReader`, `CompetitorDetector`, `SystemUtilities`.
6. **Commands**: `ArchiveCommandProtocol`, `CommandHistoryManager`, concrete UI commands.
7. **Localization**: Manager, LocaleKeys, formatters, and localized catalogs.

---

## 2. Invariants & Zero Breaking Changes

- All symbols exported to `TTZipApp` and `Tests` remain 100% API-compatible.
- Single-file LOC defense gate ($\le 800\text{ LOC}$) must pass for all files.
- 4-stage local CI gate must pass 100% with zero regressions.
