# Feature Specification: 199-purge-obsolete-examples-and-hidden-build-dirs

## 1. Executive Summary & Strategic Motivation
1. Purge obsolete `examples/` directory (`examples/quickstart.c` 89 LOC unbuildable legacy C API prototype).
2. Clean up hidden historical build directories in root (`.build_custom/`, `.build_di_test/`, `.build_tmp/`).
3. Ensure workspace is 100% pristine and free of unbuildable code or transient directories.
4. Pass all 4 stages of local CI gate in $< 10\text{s}$.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Clean Examples Domain
- **Given** inspecting the repository
- **When** checking for sample code
- **Then** modern guides live directly in `README.md` and `Sources/TTZipCLI`.
- **And** zero broken `.c` examples exist.

### User Scenario 2: Pristine Hidden Directories
- **Given** listing hidden directories in repository root
- **When** inspecting `.build*`
- **Then** only active SwiftPM `.build/` exists.

---

## 3. Success Metrics
1. Delete `examples/` directory (`quickstart.c`).
2. Delete `.build_custom/`, `.build_di_test/`, `.build_tmp/`.
3. Pass local CI/CD automated gate.
