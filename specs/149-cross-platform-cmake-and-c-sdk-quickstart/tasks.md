# Tasks: 149-cross-platform-cmake-and-c-sdk-quickstart

## Phase 1: Cross-Platform CMake Configuration (US1)

- [x] T001 [US1] Update CMakeLists.txt to add Linux POSIX library detection (pthread, m, dl, z, bz2) and remove duplicate destination
- [x] T002 [US1] Add ttzip-quickstart target in CMakeLists.txt

## Phase 2: C SDK Quickstart Example (US2)

- [x] T003 [US2] Create examples/quickstart.c demonstrating core API usage
- [x] T004 [US2] Compile and run build/ttzip-quickstart verifying all 4 demonstrations

## Phase 3: Local CI Pipeline Integration (US1 & US2)

- [x] T005 [US1] Update scripts/local-ci.sh to build and run ttzip-quickstart
- [x] T006 [US1] Execute ./scripts/local-ci.sh ensuring 100% green verification
