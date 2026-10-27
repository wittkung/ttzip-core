# Plan: Core Engine Purification & Multi-Language Ecosystem

**Feature**: `219-core-engine-purification-and-multilingual-ecosystem`  
**Classification**: `[Full SDD]`  

---

## 1. Technical Execution Phases

### Phase 1: `ttzip-engine` Pure Rust Crate Creation
1. Create `rust/ttzip-engine/Cargo.toml` (`crate-type = ["rlib"]`).
2. Move core algorithm modules (`codecs`, `zip`, `sevenz`, `fs`, `runtime`, `security`, `types`, `benchmark`) into `ttzip-engine`.
3. Set `#![forbid(unsafe_code)]` at top of `ttzip-engine/src/lib.rs`.

### Phase 2: `ttzip-glue` Thin C-ABI Refactoring
1. Make `ttzip-glue` depend on `ttzip-engine`.
2. Keep only `ffi/` modules in `ttzip-glue` and re-export C-ABI symbols.
3. Verify `libTTZipVendor.a` and Swift C-bridge compile with 0 warnings.

### Phase 3: Python SDK & TUI Direct Engine Alignment
1. Update `rust/ttzip-python/Cargo.toml` to depend on `ttzip-engine` (and `ttzip-glue` where needed).
2. Run pytest suite and benchmark suite to verify performance.

### Phase 4: C/C++ CMake & pkg-config Tooling
1. Create `cmake/FindTTZip.cmake` and `cmake/TTZipConfig.cmake`.
2. Create `ttzip.pc.in` and `scripts/generate_pkg_config.sh`.

### Phase 5: Node.js / TypeScript Native N-API SDK
1. Setup `rust/ttzip-node/` crate using N-API.
2. Provide `node/` package with `index.d.ts`, `index.js`, and test script.

### Phase 6: Zero-Cloud CI & LOC Defense Gate
1. Execute `scripts/lint_loc_gate.sh`.
2. Execute `scripts/run_local_ci_gate.sh`.
3. Synchronize to `/Users/kevintung/Documents/dev/ttzip-core/` and push.
