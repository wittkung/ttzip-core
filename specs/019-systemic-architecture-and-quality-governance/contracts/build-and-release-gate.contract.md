# Contract: Build System, Compiler Diagnostics & Release Engineering Gates

- **Specification**: `specs/019-systemic-architecture-and-quality-governance`
- **Domain**: Release Automation, Hardened Runtime, Code Signing, SPM Flags, CI Gates
- **Standard**: Zero-Warning, Release-by-Default

---

## 1. Compiler Flags & Concurrency Enforcements

### 1.1 `apple/Package.swift`
```swift
let swiftSettings: [SwiftSetting] = [
    .enableUpcomingFeature("StrictConcurrency"),
    .enableUpcomingFeature("ExistentialAny"),
    .unsafeFlags(["-warnings-as-errors"])
]
```

### 1.2 `core/Package.swift`
```swift
// Forbids `-no-whole-module-optimization`
let coreSwiftSettings: [SwiftSetting] = [
    .enableUpcomingFeature("StrictConcurrency"),
    .enableUpcomingFeature("ExistentialAny")
]
```

---

## 2. Release Engineering Invariants

1. **Release-by-Default**: `bundle_app.sh`, `重新构建并启动TTZip.command`, and all installation scripts MUST default to `BUILD_CONFIG="release"`.
2. **Binary Optimization**: All release binaries MUST undergo:
   - Whole Module Optimization (`-c release`)
   - Link-Time Optimization (`-Xlinker -dead_strip`)
   - Symbol Stripping (`strip -x`)
3. **Deterministic Repository Hygiene Gate**: `scripts/lint_repo_hygiene.sh` MUST pass with 0 errors before release bundling:
   - 0 rogue HTML files in `core/` root.
   - 0 orphaned source folders in `core/Sources/`.
   - 0 `.unsafeFlags` disabling release optimization.
   - 0 unignored `.DS_Store` or `._*` files.
