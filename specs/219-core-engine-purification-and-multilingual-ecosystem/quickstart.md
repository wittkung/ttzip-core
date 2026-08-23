# Quickstart: Core Engine & Multi-Language Ecosystem

**Feature**: `219-core-engine-purification-and-multilingual-ecosystem`  

---

## 1. Rust Pure Library (`ttzip-engine`)

```toml
[dependencies]
ttzip-engine = "1.0.0"
```

```rust
use ttzip_engine::codecs::deflate::deflate_compress;
```

---

## 2. CMake C/C++ Integration

```cmake
find_package(TTZip REQUIRED)
target_link_libraries(my_app PRIVATE TTZip::Core)
```

---

## 3. Node.js Integration

```javascript
const ttzip = require('ttzip');

const entries = ttzip.inspect('archive.zip');
console.log('Entries:', entries);
```
