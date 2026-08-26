# Research: Core Engine Purification & Multi-Language Ecosystem

**Feature**: `219-core-engine-purification-and-multilingual-ecosystem`  

---

## 1. Rust Workspace Layering Pattern

### Standard Rust Systems Architecture
```
rust/
├── ttzip-engine/       # Layer 0: Pure algorithm crate, zero unsafe, rlib
├── ttzip-glue/         # Layer 1: C-ABI FFI wrapper, staticlib/cdylib
├── ttzip-python/       # Layer 2: PyO3 extension, cdylib (depends on ttzip-engine)
├── ttzip-node/         # Layer 2: N-API extension, cdylib (depends on ttzip-engine)
└── ttzip-tui/          # Layer 2: CLI/TUI binary (depends on ttzip-engine)
```

**Benefits**:
- Zero duplicate compilation of codec engines.
- `ttzip-engine` can be consumed by pure Rust crates without linking libc or exporting C symbols.
- PyO3 directly calls Safe Rust functions returning `Result<T, EngineError>`, avoiding C pointer marshaling and `ttzip_rust_` FFI boilerplate.

---

## 2. CMake and pkg-config Integration

### `FindTTZip.cmake`
- Searches `ttzip.h` in standard include paths (`/usr/local/include`, `/opt/homebrew/include`).
- Searches `libttzip.a` / `libttzip.dylib` in library paths.
- Creates imported target `TTZip::Core` with `INTERFACE_INCLUDE_DIRECTORIES` and `INTERFACE_LINK_LIBRARIES`.

---

## 3. Node.js N-API Pattern (`napi-rs`)
- High-performance asynchronous worker threads releasing V8 event loop.
- Buffer exchange without copying via `napi::bindgen_prelude::Buffer`.
