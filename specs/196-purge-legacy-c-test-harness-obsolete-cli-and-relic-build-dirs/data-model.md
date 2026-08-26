# Data Model: 196-purge-legacy-c-test-harness-obsolete-cli-and-relic-build-dirs

## 1. Unified Clean Testing Architecture
```
Tests/
  ├── TTZipAppTests/      # GUI App Unit & Snapshot Tests
  └── TTZipTests/         # High-Level Swift Core & CLI Integration Tests

rust/
  └── ttzip-glue/
      └── tests/          # Property, Fuzz, Differential, and FFI Tests
```
