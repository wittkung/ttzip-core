# Implementation Plan: 145-pure-c-container-framing-and-cli-engine

## Technical Context
- **Language & Runtime**: Pure C11 (Core & CLI) + Swift 6.0 (GUI Wrapper).
- **Core Dependencies**: `ttzip_threadpool`, `ttzip_fs`, `libdeflate`, `fast-lzma2`, `libzstd`, `lzfse`, `snappy`, `zopfli`.
- **Target Deliverables**:
  1. `Sources/CTTZipBridge/include/ttzip_archive.h` + `Sources/CTTZipBridge/ttzip_archive.c`
  2. `cli/main.c` -> `build/ttzip-cli`
  3. `Sources/CTTZipBridge/ttzip_platform_detect.c` (x86_64 CPUID activation)
  4. Updated `CMakeLists.txt` with `add_executable(ttzip-cli cli/main.c)`
  5. Updated `scripts/local-ci.sh` verifying CLI builds and runs.

## Constitution Check
- Zero-cost abstractions on hot paths maintained.
- Zero GCD dependencies in C or Swift core engines.
- Zero GPL-3 dependencies.
- Dual-ISA hardware vector acceleration for ARM64 and x86_64.

## Phase 0: Research Items
- - R001 [SUBAGENT:research] 《C CLI Architecture》: Argument parsing and stdout progress formatting without third-party libraries.
- - R002 [SUBAGENT:research] 《CPUID Dispatch》: Safe cross-compiler x86_64 CPUID intrinsic detection on MSVC, GCC, and Clang.

## Phase 1: Artifacts & Contracts
- `contracts/ttzip-cli-contract.json`
- `quickstart.md`
