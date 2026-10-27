# Feature Specification: EmbeddingGemma 2 (`llama.cpp` GGUF) End-to-End Semantic Search

**Feature Branch**: `232-embedding-semantic-search`  
**Created**: 2026-04-17  
**Updated**: 2026-10-18  
**Status**: Phase 7 In Progress (Real GGUF + `llama.cpp` + Rust + UniFFI + Swift Verified; Process Phase Memory Sampled; Build-Stage Test RPATH Active; W1 Production Onboarding Aligned with `diskPath` & `targetKind`; 16th Test Pending Formal Target Re-Run)  
**Canonical Twin**: [`products/ttzip/docs/twin/semantic-search.md`](../../../docs/twin/semantic-search.md)

---

## 0. Five-Part Delivery & Verification Status

| # | Deliverable | Current Status | Verification Scope |
| :--- | :--- | :--- | :--- |
| **1** | **Deterministic Test-Double Control-Flow, Bundled-Only Dylib Policy & Real Engine Missing-Model Degradation** | **PASS (10/10 Rust Tests)** | Verified via `bazel test //products/ttzip/core/rust/crates/ttzip-semantic:all`. Proves `LlamaDylibLoadPolicy::BundledOnly` rejects Homebrew, bare library names, and env overrides; verifies scope isolation, chunking, incremental delete, cancellation, `.testdbl.ttvec` isolation, and real engine missing-model degradation (`NotInstalled` + `DegradedLexicalOnly`). |
| **2** | **Reference Vector Truncation & Quantization Math** | **PASS (Math Only)** | Verified via declared Bazel test fixture [`digup_reference_vectors_v1.json`](../../rust/crates/ttzip-semantic/tests/fixtures/digup_reference_vectors_v1.json). Proves MRL-128 truncation + symmetric INT8 quantization preserves $\ge 0.999$ cosine similarity against FP32 MRL-128 on official `google/embeddinggemma-2` reference vectors. |
| **3** | **Real Model Loading, Upstream `llama.h` Binding, `llama.cpp` Inference & Process Phase-Sampled Memory Measurement** | **PASS (Real GGUF Inference)** | [`csrc/ttzip_llama_bridge.c`](../../rust/crates/ttzip-semantic/csrc/ttzip_llama_bridge.c) directly `#include "llama.h"` (`bound_to_upstream_llama_header = true`), loads `embeddinggemma-2-Q8_0.gguf` (`309,855,456` B, SHA-256 `2188ac1deca4b77dffefd603c2776a9d76d9d74ec01841392982ebb840b09135`) on Metal (`MTL0`) and CPU, verifies `min_reference_cosine = 0.9999992 >= 0.999` across all 5 prompt-formatted reference vectors. Records test process phase samples: highest observed ~421 MiB, after unload ~273 MiB (not signed application peak; residual cause and leak status require verification, not directly attributed to driver cache). |
| **4** | **Rust → UniFFI → Swift Formal `OmniSearchTests`, Unconditional `.ready` Real-Model Suite & Production Onboarding Helpers** | **15/15 PASS; 16th Test Pending Formal Target Re-Run** | Verified via `bazel test //products/ttzip/apple:OmniSearchTests --test_filter=SemanticSearchTests` with build-stage RPATH `linkopts` (zero runtime `.xctest/Contents/Frameworks` symlink mutation). Includes `testRealGGUFModelEndToEndBilingualSemanticRetrievalViaUniFFI` (unconditional `guard case let .ready(desc) = state else { XCTFail(...); return }`). The 16th unit test `testProductionServiceOnboardingHelpersAndGlobalSearchResultItemMapping` (asserting `diskPath` and `targetKind`) remains pending formal target re-run acceptance. |
| **5** | **Window/Archive Opening (`W1`) & Signed `.app` Bundle Verification (`H`)** | **PENDING W1 / H** | • **Open File Boundary**: Q5's `buildOpenRequest(for:)` and `hit.toGlobalSearchResultItem()` construct the verified `SemanticOpenRequest` and `GlobalSearchResultItem` (`targetKind` yields `.diskItem(path:)` for directory hits and `.archiveEntry(archivePath:entrySubpath:)` for archive hits); panel invocation and window routing are owned by **W1** per [`contracts/handoff_patches.md`](./contracts/handoff_patches.md).<br>• **Body Line Number Boundary**: `lineStart` / `lineEnd` / `lineNumber` are formatted into `GlobalSearchResultItem.subtitle` and carried in `SemanticOpenRequest.lineNumber` as metadata only; `GlobalSearchResultItem`, `BrowserWindowRouter`, and `AppViewState` **do not yet have a consumer for body line positioning**.<br>• **Signed `.app` Bundle**: Embedding and signing `libllamacpp.dylib` inside `TTZip.app/Contents/Frameworks` is owned by **H**. |

---

## 1. User Scenarios & Testing *(mandatory)*

### User Story 1 - Authorized Content & Path Hybrid Search (Priority: P1)

As a macOS user searching across user-authorized folders and open archives in TTZip, I want natural-language queries in Chinese, English, or cross-lingual phrasing to retrieve relevant files even when the filename does not match the query tokens, and display the matching text snippet.

**Acceptance Scenarios**:

1. **Given** an authorized directory containing `notes_01.md` whose body discusses `滑动窗口限流与令牌桶退避策略`, **When** the user queries `"rate limiting backoff"` with real `embeddinggemma-2-Q8_0.gguf` loaded, **Then** `notes_01.md` ranks above unrelated files, `hit.toGlobalSearchResultItem()` produces `diskPath == resolvedOuterFilePath`, `archivePath == nil`, and `targetKind == .diskItem(path: fullPath)`, routing to `routeGlobalSearchDiskItem`.
2. **Given** an open archive containing `docs/runbook.txt` discussing `数据库连接池泄漏排查`, **When** the user queries `"connection pool exhaustion"`, **Then** `hit.toGlobalSearchResultItem()` produces `archivePath == containerOrRootPath`, `entrySubpath == "docs/runbook.txt"`, `diskPath == nil`, and `targetKind == .archiveEntry(archivePath:entrySubpath:)`, routing to `routeGlobalSearchArchiveEntry`.
3. **Given** PDF, Office, image, or audio/video files inside an authorized directory, **When** the index is built in Phase 1, **Then** those files are indexed as `PathOnly` (`supported_content = false`), and content-semantic matching is restricted to UTF-8 plain text and source code files.

---

### User Story 2 - Honest Unlinked & Degraded Status Reporting (Priority: P1)

As a privacy-conscious macOS user and developer, I want TTZip to run 100% locally, never substitute a fake keyword/hash encoder in production, and clearly distinguish between an unlinked UniFFI engine (`engineUnlinked`), a component-level Swift adapter test (`RecordingUniFFISemanticSearchEngine(missingModel: true)`), and a real Rust engine running in lexical-only fallback mode (`degradedLexicalOnly`).

**Acceptance Scenarios**:

1. **Given** `UniFFIRustSemanticCoreBridge` is initialized with `uniffiEngine: nil` (unlinked), **When** `modelState()`, `startDirectoryIndexing`, or `search` is called, **Then** it returns `.engineUnlinked(reason:)`, `phase == .engineUnlinked`, `isEngineLinked == false`, and `degradedLexicalOnly == false`.
2. **Given** `SemanticSearchCoordinator::new_production` is pointed at a missing model path, **When** `modelState()`, `start_directory_indexing`, or `search` is called, **Then** it returns `NotInstalled`, `phase == DegradedLexicalOnly`, and `degraded_lexical_only == true`.
3. **Given** a `.testdbl.ttvec` or corrupted `.ttvec` file whose header fingerprint does not equal `PINNED_MODEL_FINGERPRINT` (`0x914F_7F80_B114_6101`), **When** the production engine loads the scope index, **Then** it rejects the file and marks the scope for clean re-indexing.

---

## 2. Requirements *(mandatory)*

### Functional Requirements

- **FR-001 (Pinned Model & Runtime Contract)**: Production semantic encoding MUST use `ggml-org/embeddinggemma-2-GGUF` (`embeddinggemma-2-Q8_0.gguf`, `309,855,456` bytes, SHA-256 `2188ac1deca4b77dffefd603c2776a9d76d9d74ec01841392982ebb840b09135`, upstream revision `914f7f8`, non-optional immutable download URL `PinnedSemanticModelConstants.pinnedDownloadURLString`) via `llama.cpp` C-ABI (`b11461` / `4d756bc72bf00a4aacf410ae15a2d315f3db400d`).
- **FR-002 (Upstream Header Binding, Bundled-Only Dylib Loading & Build-Stage Test RPATH)**:
  - `csrc/ttzip_llama_bridge.h` MUST expose only opaque handles and stable interfaces without hand-written upstream `llama.cpp` struct definitions.
  - `csrc/ttzip_llama_bridge.c` MUST directly `#include "llama.h"` and derive all `_Static_assert`s and layout snapshot values from the real upstream header.
  - Production dylib loading (`LlamaDylibLoadPolicy::BundledOnly`) MUST only load bundled runtime paths (`@executable_path/../Frameworks/libllamacpp.dylib`, `@loader_path/../Frameworks/libllamacpp.dylib`, `@rpath/libllamacpp.dylib`). Test targets MUST resolve `@rpath/libllamacpp.dylib` via build-stage `linkopts` RPATHs rather than mutating `.xctest/Contents/Frameworks` at runtime.
- **FR-003 (Zero Test Double in Production Path)**: Production constructors in Rust (`SemanticSearchCoordinator::new_production`, `UniFFISemanticSearchEngine::new`) and Swift (`SemanticSearchEngine.init(storageDirectory:)`) MUST NOT instantiate or fall back to `DeterministicEmbeddingProvider`.
- **FR-004 (Unconditional `.ready` Assertion in Dedicated Real-Model Test)**: `testRealGGUFModelEndToEndBilingualSemanticRetrievalViaUniFFI` MUST require `.ready` via `guard case let .ready(desc) = state else { XCTFail(...); return }` and never silently skip assertions when the model is missing or failed.
- **FR-005 (Production Service Onboarding & W1 Contract)**: `SemanticSearchEngine` MUST provide `shared`, `defaultStorageDirectory()`, `scopeId(forDirectoryURL:)`, `scopeId(forArchivePath:)`, `registerAuthorizedDirectory(url:isAuthorized:startIndexingImmediately:)`, `revokeAuthorizedDirectory(url:)`, and `SemanticSearchHit.toGlobalSearchResultItem()` populating `diskPath` for directory hits (`targetKind == .diskItem(path:)`) and `archivePath` + `entrySubpath` for archive hits (`targetKind == .archiveEntry(archivePath:entrySubpath:)`).
- **FR-006 (Production Instance Composition & Terminal Shutdown on Normal Exit)**:
  - **Shared Production Composition**: Settings (`SettingsView` / `LocalModelSettingsSectionView`), Global Search (`GlobalSearchPanelController`), Directory Authorization/Indexing (`RootFolderAccessManager`), and AppKit Termination (`AppDelegate`) MUST reference the same process-wide `SemanticSearchEngine` instance (`LocalModelManager.shared.engine === SemanticSearchEngine.shared`). Explicit dependency injection in tests MUST remain supported so isolated test instances can use independent temporary directories, recorder bridges, and download transports without mutating global singletons.
  - **Terminal Shutdown vs. Reset/Reload**: Normal application termination MUST execute an idempotent terminal `shutdown()` across `LocalModelManager` -> `SemanticSearchEngine` -> `RustSemanticCoreBridging` (`UniFFIRustSemanticCoreBridge`). Terminal shutdown MUST close entry points for new load/reload/search/index/download operations, cancel and drain all accepted in-flight operations (including download tasks that were cancelled via `cancelDownload()` before `shutdown()` began), and release the native `UniFFISemanticSearchEngine` binding (`uniffiEngine = nil`) so `LlamaSession::drop` -> `ttzip_llama_session_destroy` executes before C++ `atexit` (`__cxa_finalize_ranges`). Terminal shutdown MUST NOT invoke `engineFactory` to recreate a binding, re-register scopes, or query `modelState()` in a way that implicitly loads a GGUF file on disk.
  - **Accepted Download Task & Per-Request Staging Ownership**: `cancelDownload()` hides/stops the current UI download intent and signals `Task.cancel()`, but every accepted download task MUST remain owned by `LocalModelManager` until its execution body actually finishes. Each download generation MUST write to an independently owned staging path (`<filename>.part.<generation>`) and clean up only its own staging file upon exit or cancellation so a retiring transport can never overwrite or delete a newer retry's staging file.
  - **Non-Destructive Cleanup**: Normal exit shutdown releases only runtime handles and transient `.part*` staging files after all accepted download tasks have drained; it MUST NOT delete verified installed GGUF files, persistent `.ttvec`/`.ttidx` indices, or unrelated user files (`deleteModel()` remains an explicit user action).
  - **No Late Revival**: Late returns from cancelled or in-flight downloads, checksum verifications, runtime reloads, or searches after shutdown initiation MUST NOT republish `.ready`, recreate native bindings, or start new indexing work.
  - **AppKit `CmdQ` Termination Gate**: `AppDelegate.applicationShouldTerminate(_:)` MUST initiate a single owned async shutdown and return `.terminateLater`, calling `sender.reply(toApplicationShouldTerminate: true)` on `MainActor` only after shutdown and resource release complete (or `false` if resource release cannot be confirmed). Repeated `CmdQ` requests during shutdown MUST share the single in-flight cleanup without duplicate teardown or duplicate replies; unloaded exit MUST NOT load a model in order to terminate.
  - **Separate Verification Accounting**: Controlled unit/lifecycle tests, real `.app` loaded-GGUF -> `CmdQ` exit verification, and live network reachability/redistribution notice delivery for the non-optional `PinnedSemanticModelConstants.pinnedDownloadURLString` MUST be accounted for separately. Test-runner `exit(0)` and test-instance `unloadModelRuntime()` teardown do not substitute for real GUI `.app` loaded-GGUF -> `CmdQ` verification.
