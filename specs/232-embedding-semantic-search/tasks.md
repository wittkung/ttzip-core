# Tasks: EmbeddingGemma 2 (`llama.cpp` GGUF) Semantic Search Remediation

**Input**: [`spec.md`](./spec.md), [`plan.md`](./plan.md), [`docs/twin/semantic-search.md`](../../../docs/twin/semantic-search.md)

---

## Status Overview (5 Deliverables)

1. **Deterministic Test-Double Control-Flow, Bundled-Only Dylib Policy & Real Engine Missing-Model Degradation**: `[x] PASS (10/10 Rust Tests)`
2. **Reference Vector Truncation & Quantization Math Tests**: `[x] PASS (Math Only)`
3. **Real Model Loading, Upstream `llama.h` Binding, `llama.cpp` Inference & Process Phase-Sampled Memory Measurement**: `[x] PASS (Real GGUF Inference on Metal MTL0 & CPU, min_reference_cosine = 0.9999992 >= 0.999; Test process phase samples: highest observed ~421 MiB, after unload ~273 MiB. Not signed app peak; residual cause and leak status require verification, not directly attributed to driver cache)`
4. **Rust → UniFFI → Swift Formal `OmniSearchTests` & Production Onboarding Helpers**: `[ ] 15/15 PASS; 16th Test Pending Formal Target Re-Run (Build-Stage RPATH Active, Unconditional .ready Suite Passed; 16th Test with diskPath & targetKind Assertions Implemented & Awaiting Re-Run Acceptance)`
5. **Host Window/Archive Opening (`W1`) & Signed `.app` Bundle Verification (`H`)**: `[ ] PENDING W1 / H (Q5 Service & Contract Ready; Line Number Has No Host Consumer Yet)`

---

## Phase 1–6: Completed Foundation & Direct Upstream `llama.h` Binding

- [x] **T001–T017** Remove `DeterministicEmbeddingProvider` from production constructors; isolate `TEST_DOUBLE_MODEL_FINGERPRINT = 0x5445_5354_4442_4C01`; declare `digup_reference_vectors_v1.json` in `BUILD.bazel`; enforce `LlamaDylibLoadPolicy::BundledOnly`; implement 11-method Swift $\to$ UniFFI forwarding and `.engineUnlinked` state in `UniFFIRustSemanticCoreBridge`; bind `csrc/ttzip_llama_bridge.c` directly to upstream `llama.h`.

---

## Phase 7: Closure Verification & Production Integration Readiness (Q5)

- [x] **T018 [Owner H]** Mount `//third_party/llama_cpp` (`b11461` / `4d756bc72bf00a4aacf410ae15a2d315f3db400d`) headers and `libllamacpp.dylib` on `ttzip_llama_bridge`, regenerate `TTZipCoreBindings` (`ttzip_engine.swift`), and enable `-DTTZIP_UNIFFI_SEMANTIC_SEARCH_LINKED` on `TTZipAppServices` / `OmniSearchTests`.
- [x] **T020 [Q5]** Split `testRealGGUFModelEndToEndBilingualSemanticRetrievalViaUniFFI` from `testProductionSemanticSearchEngineConstructorMatchesCompileTimeLinkState` with unconditional `guard case let .ready(desc) = state else { XCTFail(...); return }`; remove runtime `.xctest/Contents/Frameworks` symlink creation and configure build-stage RPATH `linkopts` on `ttzip_llama_bridge`; sample 5-stage `TASK_VM_INFO.phys_footprint` during test execution (highest observed ~421 MiB, after unload ~273 MiB; record as test process phase samples, not signed app peak, residual cause and leak status pending verification).
- [x] **T021 [Q5]** Align directory hits with `diskPath` in `SemanticSearchContracts.swift` (`hit.toGlobalSearchResultItem()`), retaining `archivePath` + `entrySubpath` for archive hits, and add `testProductionServiceOnboardingHelpersAndGlobalSearchResultItemMapping` in `SemanticSearchTests.swift` asserting `dirItem.targetKind == .diskItem(path:)` and `archItem.targetKind == .archiveEntry(archivePath:entrySubpath:)`; keep this 16th test in pending-acceptance status until formal test target re-run; update `contracts/handoff_patches.md` with verified `RootFolderAccessManager` / `GlobalSearchPanelController` / `BrowserWindowRegistry` / `BrowserWindowRouter` / `AppViewState` interfaces and explicitly record that body line numbers (`lineStart` / `lineEnd` / `lineNumber`) currently have no host line-jump consumer.
- [ ] **T019 [Owner W1 / Owner H]** W1 wires `SemanticSearchEngine.shared` into `GlobalSearchPanelController.swift`, `RootFolderAccessManager.swift`, and `BrowserWindowRouter.swift` (`routeGlobalSearchArchiveEntry` / `routeGlobalSearchDiskItem`); H verifies `libllamacpp.dylib` codesigning and model loading inside the packaged `TTZip.app` bundle.
