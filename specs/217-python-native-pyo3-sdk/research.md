# Research & Decision Matrix: High-Performance Python Native Binding via PyO3

**Feature**: `217-python-native-pyo3-sdk`  
**Status**: `COMPLETED`  

---

## 1. Binding Architecture: PyO3 vs. CFFI vs. ctypes

### Decision
Use **PyO3 0.22+ with `abi3-py310`** and `maturin`.

### Decision Matrix

| Metric / Dimension | PyO3 (Chosen) | CFFI | ctypes |
| :--- | :--- | :--- | :--- |
| **Call Overhead** | **$< 20\text{ ns}$** (Direct C-API / PyMethodDef) | $\sim 150\text{ ns}$ | $\sim 300\text{ ns}$ |
| **GIL Management** | **Native `py.allow_threads`** | Manual `ffi.release()` | Requires `PyEval_SaveThread` |
| **Type Safety** | **Rust Compile-time typed conversions** | Runtime C-type conversions | Runtime C-type conversions |
| **Packaging & Wheels** | **Automated via `maturin`** | Manual distutils / setuptools | Manual `.so` / `.dylib` bundling |
| **ABI Compatibility** | **`abi3` (One wheel works across Python 3.10~3.14)** | Python version specific | Dynamic loading |

---

## 2. GIL Release Concurrency Model

### Pattern
During heavy operations (archive creation, decompression, buffer codec compression), the GIL must be explicitly released:

```rust
let result = py.allow_threads(move || {
    // Heavy Rust multi-core Rayon workload runs here without GIL contention
    ttzip_glue::archive::extract_archive(...)
});
```

This guarantees that multiple Python threads running `ThreadPoolExecutor` can decompress different archives in parallel at full hardware memory bandwidth.
