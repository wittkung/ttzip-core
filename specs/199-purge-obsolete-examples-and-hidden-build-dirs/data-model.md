# Data Model: 199-purge-obsolete-examples-and-hidden-build-dirs

## 1. Clean Pristine Repository Structure
```
Repo Root:
  ├── Sources/             # Swift 6 Targets (TTZipApp, TTZipCLI, TTZipCore, TTZipBench, CTTZipBridge)
  ├── Tests/               # Swift Tests (TTZipTests, TTZipAppTests)
  ├── rust/                # Safe Rust Workspace (ttzip-glue, ttzip-tui)
  ├── Vendor/              # XCFramework (TTZipVendor.xcframework) & upstream submodules
  └── scripts/             # Local CI/CD Automation & Maintenance Scripts
```
