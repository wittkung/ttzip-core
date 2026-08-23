# Data Model: Multi-Ecosystem Package Distribution

**Feature**: `218-multi-ecosystem-package-distribution`  

---

## 1. Distribution Artifacts Entity Matrix

```text
+-----------------------------------------------------------------------------------------------+
| Ecosystem   | Artifact Path                             | Build Engine | Distribution Channel |
+-------------+-------------------------------------------+--------------+----------------------+
| Homebrew    | Formula/ttzip.rb                          | Ruby / Cargo | wittkung/homebrew-tap|
| Crates.io   | rust/target/package/ttzip-*.crate         | cargo package| crates.io API        |
| PyPI        | dist/ttzip-1.0.0-cp310-abi3-*.whl         | maturin      | PyPI / pip           |
+-----------------------------------------------------------------------------------------------+
```

---

## 2. Channel Version Synchronization Model

All 3 package managers must synchronize version `1.0.0` from `rust/Cargo.toml` and `pyproject.toml`:
- `ttzip-core` SPM tag: `v1.0.0`
- Homebrew formula: `url "https://github.com/wittkung/ttzip-core/archive/refs/tags/v1.0.0.tar.gz"`
- Crates.io version: `version = "1.0.0"`
- Python PyPI version: `version = "1.0.0"`
