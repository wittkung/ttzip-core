# Implementation Plan: EmbeddingGemma 2 (`llama.cpp` GGUF) End-to-End Semantic Search

**Branch**: `232-embedding-semantic-search` | **Date**: 2026-10-18 | **Spec**: [`spec.md`](./spec.md)  
**Twin Document**: [`products/ttzip/docs/twin/semantic-search.md`](../../../docs/twin/semantic-search.md)

---

## 1. Architecture & Closure Summary

TTZip's semantic search follows the **GGUF + `llama.cpp` C-ABI Bridge + Rust (`ttzip-semantic`) + UniFFI + Swift thin adapter** architecture.
All Q5 verification and production integration readiness items are aligned:

1. **Direct Upstream `llama.h` Binding & Build-Stage Test RPATH**:
   - `csrc/ttzip_llama_bridge.h` exposes only the opaque session handle (`TtzipLlamaBridgeSession`), layout snapshot (`TtzipLlamaAbiLayoutSnapshot` with `bound_to_upstream_llama_header`), and 5 C bridge functions (including `ttzip_llama_current_phys_footprint_bytes`).
   - `csrc/ttzip_llama_bridge.c` directly includes `"llama.h"` (`//third_party/llama_cpp:llama_cpp`).
   - Runtime `.xctest/Contents/Frameworks/libllamacpp.dylib` symlink creation has been removed from `SemanticSearchTests.swift`; `cc_library(name = "ttzip_llama_bridge")` in `products/ttzip/core/rust/crates/ttzip-semantic/BUILD.bazel` propagates build-time macOS RPATH `linkopts` to resolve `@rpath/libllamacpp.dylib` from declared Bazel runfiles.
2. **Strict Separation of Status Tests, Component Adapter Tests, and Real-Model `.ready` Tests**:
   - `testProductionSemanticSearchEngineConstructorMatchesCompileTimeLinkState`: verifies default production constructor link state.
   - `testRealGGUFModelEndToEndBilingualSemanticRetrievalViaUniFFI`: unconditionally asserts `guard case let .ready(desc) = state else { XCTFail(...); return }` and verifies English/Chinese zero-filename-overlap body retrieval and `buildOpenRequest`.
   - `testLinkedBridgeWithMissingModelReportsNotInstalledAndDegradedLexicalOnly`: explicitly documented as a Swift adapter component test (`RecordingUniFFISemanticSearchEngine(missingModel: true)`), while real Rust missing-model degradation is verified in `test_production_coordinator_never_uses_test_double_and_degrades_honestly`.
   - `testProductionServiceOnboardingHelpersAndGlobalSearchResultItemMapping` (16th test): verifies onboarding helpers and asserts `dirItem.targetKind == .diskItem(path:)` (filling `diskPath`) and `archItem.targetKind == .archiveEntry(archivePath:entrySubpath:)` (filling `archivePath` + `entrySubpath`). **Remains pending acceptance until formal test target re-run**.
3. **Test Process Phase-Sampled Memory vs Compute Buffer**:
   - Sampled during test execution via `TASK_VM_INFO.phys_footprint`: `baseline = 8.09 MiB`, `after_load = 389.80 MiB`, `after_index = 421.38 MiB` (highest observed value), `after_query = 421.38 MiB`, `after_unload = 273.03 MiB`.
   - This represents phase samples of the test process, **not the signed application runtime peak**. The residual cause (~273 MiB) and whether any memory leak exists require further verification and must not be directly attributed to driver-level caching.
4. **Production Service Onboarding & Alignment with W1**:
   - Added `SemanticSearchEngine.shared`, `defaultStorageDirectory()`, `scopeId(forDirectoryURL:)`, `scopeId(forArchivePath:)`, `registerAuthorizedDirectory(url:isAuthorized:startIndexingImmediately:)`, `revokeAuthorizedDirectory(url:)`, `SemanticSearchHit.resolvedOuterFilePath`, and `SemanticSearchHit.toGlobalSearchResultItem()`.
   - `toGlobalSearchResultItem()` maps authorized directory hits to `diskPath = fullPath` (`targetKind == .diskItem(path:)`), avoiding accidental routing into `routeGlobalSearchArchive`. Open archive hits populate `archivePath` and `entrySubpath` (`targetKind == .archiveEntry(archivePath:entrySubpath:)`).
   - Documented exact lifecycle and field contracts with W1 in [`contracts/handoff_patches.md`](./contracts/handoff_patches.md) using only verified host types (`RootFolderAccessManager`, `GlobalSearchPanelController`, `GlobalSearchResultItem`, `BrowserWindowRegistry`, `BrowserWindowRouter`, `AppViewState`), and explicitly recorded that body line numbers (`lineStart` / `lineEnd` / `lineNumber`) currently have no line-jump consumer in host UI.

---

## 2. Ownership Boundaries

### Q5 Owned Files (Strictly Enforced)
- `products/ttzip/docs/twin/semantic-search.md`
- `products/ttzip/core/specs/232-embedding-semantic-search/**`
- `products/ttzip/core/rust/crates/ttzip-semantic/**`
- `products/ttzip/core/rust/ttzip-engine/src/uniffi_api/semantic_search.rs`
- `products/ttzip/apple/Sources/TTZipApp/Services/SemanticSearch/**`
- `products/ttzip/apple/Tests/TTZipAppTests/SemanticSearchTests.swift`

### W1 & H Owned Host Files (Not Modified by Q5)
- **W1**: `GlobalSearchPanelController.swift`, `RootFolderAccessManager.swift`, `BrowserWindowRouter.swift`, `BrowserWindowRegistry.swift`, `AppViewState.swift`, `AppViewState+ArchiveOperations.swift`, `TTZipApp.swift`
- **H**: `products/ttzip/apple/BUILD.bazel`, `third_party/llama_cpp/BUILD.bazel`, `apple/scripts/bundle_app.sh`
