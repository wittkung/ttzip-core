# Data Model: Core Engine Purification & Multi-Language Ecosystem

**Feature**: `219-core-engine-purification-and-multilingual-ecosystem`  

---

## 1. Rust Workspace Dependency Graph

```text
       [ttzip-engine] (Safe Pure Rust, rlib)
        ▲      ▲      ▲      ▲
        │      │      │      │
[ttzip-glue] [ttzip-python] [ttzip-node] [ttzip-tui]
(C-ABI FFI)    (PyO3)        (N-API)      (CLI Tool)
```

---

## 2. Package Ecosystem Export Specifications

```text
+-------------+----------------------+--------------------+-------------------------------+
| Language    | Package ID           | Binding Mechanism  | Primary Type Definitions      |
+-------------+----------------------+--------------------+-------------------------------+
| Rust        | ttzip-engine         | Direct Native      | Engine, ArchiveOptions, Codec |
| Swift       | TTZipCore            | C-ABI Bridging     | TTZipEngine, ArchiveFormat    |
| Python      | ttzip                | PyO3 ABI3          | EntryMetadata, ProgressInfo   |
| Node/TS     | ttzip                | N-API (napi-rs)    | EntryMetadata, CompressionOpt |
| C / C++     | ttzip (pkg-config)   | ttzip.h Header     | TTZipStatus, TTZipCreateOpt   |
+-------------+----------------------+--------------------+-------------------------------+
```
