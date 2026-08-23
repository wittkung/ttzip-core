# Specification: Feature 219 — Core Engine Purification & Multilingual Ecosystem

**Feature Identifier**: `219-core-engine-purification-and-multilingual-ecosystem`  
**Classification**: `[Full SDD]`  
**Status**: `SPECIFIED`  

---

## 1. Problem Statement & Background

Currently in `ttzip-core`:
1. `ttzip-glue` combines both pure Safe Rust compression/VFS/crypto algorithms AND C-ABI FFI bindings (`#[no_mangle] extern "C"`). This prevents Rust developers from using TTZip as a pure, dependency-free crate with `#![forbid(unsafe_code)]`.
2. Python SDK (`ttzip-python`) and CLI (`ttzip-tui`) have to route through FFI helpers or duplicate logic rather than directly importing pure Rust structs and `Result<T, E>` types.
3. System C/C++ developers lack standardized `pkg-config` (`ttzip.pc`) and CMake integration (`FindTTZip.cmake`).
4. Web and Node.js backend developers lack an official native N-API module (`npm install ttzip`).

---

## 2. Requirements & User Stories

### User Story 1: Pure Rust Microkernel Separation (`ARCH-1`)
- **R1.1**: Create `rust/ttzip-engine/` as a pure Safe Rust crate with `#![forbid(unsafe_code)]`.
- **R1.2**: Move core algorithms (`codecs`, `zip`, `sevenz`, `fs`, `runtime`, `security`, `crypto`) into `ttzip-engine`.
- **R1.3**: Refactor `rust/ttzip-glue` as a lightweight C-ABI translation layer depending on `ttzip-engine`.

### User Story 2: Zero-FFI Python SDK & TUI Alignment
- **R2.1**: Update `rust/ttzip-python` to depend directly on `ttzip-engine`.
- **R2.2**: Ensure all 16-format tests and 60-point matrix benchmarks pass with identical or improved throughput.

### User Story 3: C/C++ Package Config & CMake Ecosystem Tooling
- **R3.1**: Create `cmake/FindTTZip.cmake` and `cmake/TTZipConfig.cmake` defining target `TTZip::Core`.
- **R3.2**: Create `scripts/generate_pkg_config.sh` and `ttzip.pc.in` template.

### User Story 4: Node.js / TypeScript Native N-API SDK (`npm install ttzip`)
- **R4.1**: Create `rust/ttzip-node/` N-API binding crate with `napi-rs` / `napi`.
- **R4.2**: Provide TypeScript type definitions and functional tests for buffer compression and archive extraction in JavaScript.

### User Story 5: Zero-Cloud CI & LOC Defense Gate
- **R5.1**: Ensure all newly created and refactored files pass the $\le 800\text{ LOC}$ gate.
- **R5.2**: Pass the 4-stage local CI gate across Swift, Rust, and Python suites.
