# Quickstart: 019 Systemic Architecture & Quality Governance Hardening

- **Feature Directory**: `specs/019-systemic-architecture-and-quality-governance`
- **Purpose**: Step-by-step verification guide for Intent Routing, KeepAlive Tab State, Zero Warnings, and Release Engineering.

---

## 1. Prerequisites

- macOS 14.0+ (Apple Silicon or Intel x86_64)
- Xcode 16.0+ Command Line Tools (`swift --version` >= 6.0)

---

## 2. Verification Scenarios

### Scenario 1: Validate Repository Hygiene Gate
Run the deterministic repository hygiene linter:
```bash
./scripts/lint_repo_hygiene.sh
```
*Expected Output*:
```
======================================================================
          TTZip Deterministic Repository Hygiene Linter Gate          
======================================================================
--> [1/5] Checking for rogue root-level web artifacts in core/...
--> [2/5] Checking for unreferenced GUI targets inside core/Sources/...
--> [3/5] Checking for unoptimized compiler flags (.unsafeFlags)...
--> [4/5] Checking for forbidden macOS clutter (.DS_Store, ._ files)...
--> [5/5] Checking for unignored build artifacts in repository roots...
======================================================================
✅ Repository Hygiene Gate Passed: 0 violations detected.
```

---

### Scenario 2: Run Full State Transition & Intent Integration Tests
Execute the newly delivered test suites in `TTZipAppTests`:
```bash
cd apple && swift test --filter AppNavigationStateFlowTests
swift test --filter FinderSyncIntentMappingTests
```
*Expected Output*:
```
Test Suite 'AppNavigationStateFlowTests' passed
Test Suite 'FinderSyncIntentMappingTests' passed
Executed 10 tests, with 0 failures (0 unexpected)
```

---

### Scenario 3: Verify Zero-Warning Release Compilation & Bundling
Compile and package the release application:
```bash
./apple/scripts/bundle_app.sh --release
```
*Expected Output*:
```
🍎 Building and Bundling TTZip.app [release mode]
   Target Channel  : direct
   Signing Identity: -
--> [1/4] Compiling TTZipApp via Swift Package Manager in release mode...
Building for production...
Build of product 'TTZipApp' complete! (0 warnings, 0 errors)
--> [2/4] Assembling .app bundle directory structure...
--> [3/4] Performing code signing with Hardened Runtime...
--> [4/4] Verifying .app bundle integrity...
✅ Successfully bundled [direct - release]: /Users/kevintung/Documents/dev/products/ttzip/apple/dist/TTZip.app
```

---

### Scenario 4: Interactive Manual Verification
1. Open `dist/TTZip.app`.
2. Right-click any folder in Finder and choose **TTZip > New Archive** -> verify folder is instantly loaded into `CompressModalView`.
3. Switch between **Presets**, **Benchmark**, and **Home** -> verify keyboard arrow keys in non-Home tabs are not hijacked by Miller columns.
4. On the Home screen, press `Cmd+N` -> verify Compression Workspace opens.
