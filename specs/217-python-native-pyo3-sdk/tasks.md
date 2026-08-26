# Tasks: High-Throughput Native Python SDK via PyO3

**Feature**: `217-python-native-pyo3-sdk`  
**Directory**: `specs/217-python-native-pyo3-sdk`  
**Spec Path**: `specs/217-python-native-pyo3-sdk/spec.md`  
**Plan Path**: `specs/217-python-native-pyo3-sdk/plan.md`  

---

## Phase 1: PyO3 Native Extension Crate (`rust/ttzip-python`)

- [x] T001 [P] Create `rust/ttzip-python/Cargo.toml` with PyO3 0.22 extension-module and ABI3 compatibility.
- [x] T002 Implement `rust/ttzip-python/src/lib.rs` exporting native Rust archive operations, buffer codecs, SIMD checksums, and GIL release boundaries.
- [x] T003 Update `rust/Cargo.toml` to register `ttzip-python` in workspace members.

---

## Phase 2: Python Wrapper Package, Exceptions & Type Stubs

- [x] T004 [P] [US1] Create root `pyproject.toml` with Maturin build backend and package metadata.
- [x] T005 [P] [US1] Create `python/ttzip/exceptions.py` with full exception hierarchy.
- [x] T006 [P] [US1] Create `python/ttzip/models.py` with `EntryMetadata` and `ProgressInfo`.
- [x] T007 [US1] Create `python/ttzip/__init__.py`, `python/ttzip/__init__.pyi`, and `python/ttzip/py.typed`.

---

## Phase 3: Build & Wheel Compilation

- [x] T008 [P] [US2] Create `scripts/build_python.sh` to compile native wheels using `maturin build` and `maturin develop`.
- [x] T009 [US2] Verify `import ttzip` loads cleanly and `ttzip.version()` matches engine version.

---

## Phase 4: Test Suite & Concurrency Verification

- [x] T010 [P] [US3] Create `python/tests/test_basic.py` testing ZIP, 7z, and TAR creation/extraction.
- [x] T011 [P] [US3] Create `python/tests/test_buffers.py` testing in-memory Deflate/Zstd compression and SIMD CRC32.
- [x] T012 [P] [US3] Create `python/tests/test_concurrency.py` verifying true multi-threaded speedup with GIL release.
- [x] T013 [US3] Run `pytest python/tests` ensuring 100% of test cases pass.

---

## Phase 5: Verification & Zero-Cloud CI Hardening

- [x] T014 [P] [US4] Execute `scripts/lint_loc_gate.sh` ensuring all new Python and Rust files satisfy $\le 800\text{ LOC}$.
- [x] T015 [US4] Update `scripts/run_local_ci_gate.sh` to optionally execute python tests when python/pytest is installed.
