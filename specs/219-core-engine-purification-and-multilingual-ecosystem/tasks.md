# Tasks: Core Engine Purification & Multi-Language Ecosystem

**Feature**: `219-core-engine-purification-and-multilingual-ecosystem`  
**Directory**: `specs/219-core-engine-purification-and-multilingual-ecosystem`  

---

## Phase 1: Pure Rust Microkernel (`rust/ttzip-engine`)

- [ ] T001 [P] [US1] Create `rust/ttzip-engine/Cargo.toml` with workspace inheritance and dependencies.
- [ ] T002 [US1] Extract `codecs`, `zip`, `sevenz`, `fs`, `runtime`, `security`, `crypto`, `types`, `benchmark` into `rust/ttzip-engine/src/`.
- [ ] T003 [US1] Update `rust/ttzip-glue/Cargo.toml` to depend on `ttzip-engine` and delegate FFI implementations.
- [ ] T004 [US1] Update `rust/Cargo.toml` to register `ttzip-engine` in workspace members.

---

## Phase 2: Python SDK & TUI Direct Alignment

- [ ] T005 [P] [US2] Update `rust/ttzip-python` to use `ttzip_engine` directly.
- [ ] T006 [US2] Run `PYTHONPATH=python python3 -m unittest discover -s python/tests` ensuring 100% green pass.

---

## Phase 3: C/C++ CMake & pkg-config Tooling

- [ ] T007 [P] [US3] Create `cmake/FindTTZip.cmake` and `cmake/TTZipConfig.cmake`.
- [ ] T008 [P] [US3] Create `ttzip.pc.in` and `scripts/generate_pkg_config.sh`.

---

## Phase 4: Node.js / TypeScript Native N-API SDK

- [ ] T009 [P] [US4] Create `rust/ttzip-node/Cargo.toml` and `rust/ttzip-node/src/lib.rs` implementing N-API bindings.
- [ ] T010 [P] [US4] Create `node/package.json`, `node/index.d.ts`, `node/index.js`, and `node/test.js`.

---

## Phase 5: Verification & Zero-Cloud CI Gate

- [ ] T011 [P] [US5] Execute `scripts/lint_loc_gate.sh` ensuring all files $\le 800\text{ LOC}$.
- [ ] T012 [US5] Execute `scripts/run_local_ci_gate.sh` across all components.
- [ ] T013 [US5] Synchronize all updates to `../ttzip-core` and push to GitHub `wittkung/ttzip-core`.
