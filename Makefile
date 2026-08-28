# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression microkernel & SDK matrix.

.PHONY: all help build release rust-build rust-test cli python-build test test-all-sdk test-interop test-security test-bench test-sanitizers test-out-of-tree-smoke clean

all: build

help:
	@echo "TTZip Core & SDK Matrix Build System"
	@echo ""
	@echo "Targets:"
	@echo "  build                  Build Swift 6 facade (Debug)"
	@echo "  release                Build Swift 6 facade (Release)"
	@echo "  rust-build             Build Rust microkernel & workspace crates (Release)"
	@echo "  rust-test              Run Rust workspace unit & integration tests"
	@echo "  cli                    Build ttzip CLI binary"
	@echo "  python-build           Build PyO3 Python C-extension module"
	@echo "  test                   Run Swift 6 test suite"
	@echo "  test-all-sdk           Run full multi-language SDK test matrix (9 ecosystems)"
	@echo "  test-interop           Run cross-language N x N interoperability matrix"
	@echo "  test-security          Run security test suite (Zip-Slip, bounds, memory)"
	@echo "  test-bench             Run full-pipeline micro-benchmarks"
	@echo "  test-sanitizers        Run AddressSanitizer & ThreadSanitizer checks"
	@echo "  test-out-of-tree-smoke Run zero-config out-of-tree SDK smoke tests"
	@echo "  clean                  Clean all build directories and caches"

build:
	@swift build

release:
	@swift build -c release

rust-build:
	@cargo build --release --workspace --manifest-path rust/Cargo.toml

rust-test:
	@cargo test --workspace --manifest-path rust/Cargo.toml

cli:
	@cargo build --release --manifest-path rust/Cargo.toml --bin ttzip

python-build:
	@./scripts/build_python.sh

test:
	@swift test

test-all-sdk:
	@./scripts/run_all_sdk_tests.sh

test-interop:
	@python3 tests/interop/test_interop_matrix.py

test-security:
	@PYTHONPATH=tests/security:sdk/python python3 -m unittest discover -s tests/security

test-bench:
	@./scripts/run_sdk_benchmarks.sh

test-sanitizers:
	@./scripts/run_sanitizers.sh
	@./scripts/run_race_detector.sh

test-out-of-tree-smoke:
	@./scripts/run_out_of_tree_smoke.sh

clean:
	@rm -rf .build dist rust/target
