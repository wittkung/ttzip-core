# Implementation Plan: High-Throughput Native Python SDK via PyO3

**Feature**: `217-python-native-pyo3-sdk`  
**Classification**: `[Full SDD]`  
**Status**: `PLANNED`  
**Spec Path**: `specs/217-python-native-pyo3-sdk/spec.md`  

---

## 1. Technical Context

- **Target Locations**:
  - `rust/ttzip-python/` (PyO3 C-extension crate)
  - `python/ttzip/` (Python wrapper, type stubs, and exception definitions)
  - `python/tests/` (pytest verification test suite)
- **Toolchains**: Rust 1.80+, PyO3 0.22, Maturin 1.5+, Python 3.10+, pytest.
- **Zero-Cloud CI**: Local pytest gate executed via `scripts/run_python_tests.sh`.

---

## 2. Phased Execution Roadmap

### Phase 1: PyO3 Native Extension Crate (`rust/ttzip-python`)
- [ ] Create `rust/ttzip-python/Cargo.toml` with `pyo3 = { version = "0.22", features = ["extension-module", "abi3-py310"] }`.
- [ ] Implement `rust/ttzip-python/src/lib.rs` exporting native bindings:
  - `_compress`, `_extract`, `_inspect`
  - `_compress_buffer`, `_decompress_buffer`
  - `_crc32`, `_crc64`, `_version`, `_is_hardware_accelerated`
- [ ] Implement explicit GIL release wrapping (`py.allow_threads`).

### Phase 2: Python Wrapper Package, Exceptions & Type Stubs
- [ ] Create `pyproject.toml` with Maturin configuration.
- [ ] Create `python/ttzip/exceptions.py` (TTZipError, AuthenticationError, CorruptArchiveError, SecurityError).
- [ ] Create `python/ttzip/models.py` (EntryMetadata, ProgressInfo).
- [ ] Create `python/ttzip/__init__.py` and `python/ttzip/py.typed` with PEP 561 type hints.
- [ ] Create `python/ttzip/__init__.pyi` type stubs.

### Phase 3: Build & Local Wheel Compilation
- [ ] Create `scripts/build_python.sh` running `maturin develop` or `pip install -e .`.
- [ ] Verify native extension loads cleanly in Python 3.10+.

### Phase 4: Python Test Suite & Concurrency Verification
- [ ] Create `python/tests/test_basic.py` (ZIP, 7z, TAR creation and extraction).
- [ ] Create `python/tests/test_buffers.py` (Deflate, Zstd in-memory roundtrip).
- [ ] Create `python/tests/test_concurrency.py` (Multi-threaded GIL-release verification).
- [ ] Create `python/tests/test_security.py` (Zip Slip exception containment).

### Phase 5: Verification & Zero-Cloud CI Hardening
- [ ] Run pytest suite ensuring 100% pass rate.
- [ ] Run `scripts/lint_loc_gate.sh` enforcing single-file $\le 800\text{ LOC}$ ceiling.
- [ ] Integrate into `scripts/run_local_ci_gate.sh`.
