# Feature Specification: 149-cross-platform-cmake-and-c-sdk-quickstart

## 1. Executive Summary & User Scenarios

### User Scenario 1 (US1): Linux & Cross-Platform CMake Native Integration
As a C/C++ or Rust backend engineer on Linux/Windows, I want `CMakeLists.txt` to seamlessly resolve standard Unix libraries (`pthread`, `m`, `dl`, `z`, `bz2`) and export `TTZipTargets.cmake`, allowing `find_package(TTZip REQUIRED)` or `FetchContent` to work with zero configuration.

### User Scenario 2 (US2): Developer Standalone C SDK Quickstart Example
As a third-party developer embedding `libttzip` into a database, media server, or desktop app, I want a 1-minute runnable example in `examples/quickstart.c` demonstrating archive creation, in-memory inspection, and instant memory-preview extraction using the unified `ttzip_api.h` public ABI.

---

## 2. Functional Requirements

- **FR-001**: `CMakeLists.txt` must support Linux (`UNIX AND NOT APPLE`) linking `pthread`, `m`, `dl`, and system `zlib`/`bzip2` gracefully.
- **FR-002**: Create `examples/quickstart.c` demonstrating 5 core API use-cases (version check, archive creation, format sniffing, natural sort, in-memory preview).
- **FR-003**: Add `ttzip-quickstart` executable target to `CMakeLists.txt`.
- **FR-004**: Update `scripts/local-ci.sh` to compile and execute `ttzip-quickstart`.
- **FR-005**: All Swift tests and CMake builds must pass 100% green.

---

## 3. Success Criteria

1. `examples/quickstart.c` builds cleanly without warnings and executes all 5 API demonstrations.
2. CMake configuration is fully compatible with macOS, Linux, and Windows.
3. 100% local CI green.
