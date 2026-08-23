# Feature Specification: 185-total-rust-microkernel-migration-and-c-swift-pruning

## 1. Executive Summary & Strategic Motivation
This is the milestone feature for TTZip's complete architectural purification:
1. **Purge all legacy C/C++ source trees from Swift package (`Sources/CTTZipBridge/`)**:
   - Strip `zopfli/`, `fast-lzma2/`, `lzfse/`, `snappy/`, `CTTZipBridge.c`, `CTTZipBridge_Archive.c` from `Sources/CTTZipBridge/`.
   - Ensure `rust/ttzip-glue` encapsulates all native codec compilation and exports a single, unified, strongly-typed C-ABI header (`ttzip_rust_glue.h`).
2. **Sink Container Parsing & Multi-threaded Password Attack Engines to Rust**:
   - Container header parsing, Zip64 overflow handling, solid block stream seeking, and multi-threaded dictionary/brute-force password recovery run 100% in Rust (`rust/ttzip-glue/src/crypto/password_recovery.rs` & `rust/ttzip-glue/src/archive/`).
3. **Prune redundant Swift implementations & thin out TTZipCore**:
   - Swift layers (`Zip/`, `SevenZip/`, `Tar/`, `Split/`, `Crypto/`, `VFS/`) become ultra-thin facades delegating directly to Safe Rust.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Zero C Source in Swift Package
- **Given** building TTZip on macOS via Swift Package Manager
- **When** compiling `Sources/CTTZipBridge`
- **Then** SPM compiles only 0-1 tiny bridge header connecting directly to the precompiled `libTTZipVendor.a`, eliminating 78 C source files from the build graph.

### User Scenario 2: High-Speed Multi-Core Password Recovery
- **Given** password-protected archives (ZIP, 7z)
- **When** running dictionary recovery in CLI/TUI or App
- **Then** recovery executes directly in Rust Rayon worker pools with zero Swift overhead and $>500,000\text{ passwords/sec}$.

---

## 3. Success Metrics
1. **C Code Purged**: 100% of legacy C/C++ directories (`zopfli`, `fast-lzma2`, `lzfse`, `snappy`) removed from `Sources/CTTZipBridge/`.
2. **Rust as Single Source of Truth**: 100% of container, encryption, compression, recovery, and VFS logic residing in `rust/ttzip-glue`.
3. **Zero Regression**: 100% pass rate across 200+ Rust tests, 897+ Swift tests, and 7/7 local CI stages.
