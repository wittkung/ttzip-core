# Feature Specification: 193-purge-dead-c-headers-dead-facades-and-linker-cleanup

## 1. Executive Summary & Strategic Motivation
During forensic codebase inspection, 3 categories of dead legacy code were identified:
1. **Dead C Headers**: 20+ obsolete C headers (7,500+ LOC) in `Sources/CTTZipBridge/include/`.
2. **Dead Sub-Facades**: 4 unused facade files (~414 LOC) in `Sources/TTZipCore/Facades/`.
3. **Obsolete Linker Settings**: Unused linker libraries (`xml2`, `expat`) in `Package.swift`.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Clean Bridge Layer
- **Given** inspecting `Sources/CTTZipBridge/include/`
- **When** checking files
- **Then** only `ttzip_rust_glue.h`, `CTTZipBridge.h`, and `module.modulemap` are present.

### User Scenario 2: Zero Regression & Fast Builds
- **Given** compiling and testing
- **When** running `swift build && swift test`
- **Then** builds succeed with zero warnings and 100% pass rate.

---

## 3. Success Metrics
1. Delete 20+ dead C header files.
2. Delete 4 unused Facade files.
3. Clean up `Package.swift` linker flags.
4. Pass `./scripts/run_local_ci_gate.sh` with 0 failures.
