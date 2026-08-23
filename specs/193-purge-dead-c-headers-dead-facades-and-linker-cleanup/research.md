# Phase 0 Research: 193-purge-dead-c-headers-dead-facades-and-linker-cleanup

## Research Item R001: Legacy C Headers Verification
- **Decision**: Delete all legacy vendor headers in `Sources/CTTZipBridge/include/` except `CTTZipBridge.h` and `ttzip_rust_glue.h`.
- **Rationale**: 
  - All codec implementations and bindings are provided by `rust/ttzip-glue` and statically linked into `Vendor/TTZipVendor.xcframework`.
  - No Swift code references any of the old C headers.
- **Alternatives Considered**: 
  - *Keep them as references*: Wastes disk space and causes linting noise.
- **Source**: 
  - `Sources/CTTZipBridge/include/module.modulemap`
  - `Sources/CTTZipBridge/include/ttzip_rust_glue.h`

---

## Research Item R002: Unused Facades & Linker Cleanup
- **Decision**: 
  - Delete `ArchiveOperationsFacade.swift`, `ArchiveSecurityFacade.swift`, `ArchiveStreamingFacade.swift`, `TTZipEngineFacade+TemplateAndProxies.swift`.
  - Remove `.linkedLibrary("xml2")` and `.linkedLibrary("expat")` from `Package.swift`.
- **Rationale**: 
  - TTZipEngineFacade is the unified facade; sub-facades have 0 callers across the repository.
- **Alternatives Considered**: 
  - *Keep sub-facades*: Unnecessary maintenance overhead.
- **Source**: 
  - `Sources/TTZipCore/Facades/`
  - `Package.swift`
