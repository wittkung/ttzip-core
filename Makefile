# SPDX-License-Identifier: LicenseRef-TTZip-Source-Available-1.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for macOS.

.PHONY: all build release reinstall install app cli test test-all-sdk test-interop test-security test-bench test-sanitizers clean help

all: reinstall

help:
	@./scripts/reinstall.sh --help

build:
	@swift build

release:
	@swift build -c release

reinstall:
	@./scripts/reinstall.sh

install:
	@./scripts/reinstall.sh

app:
	@./scripts/reinstall.sh --app

cli:
	@./scripts/reinstall.sh --cli

mas:
	@./scripts/reinstall.sh --mas

test:
	@swift test

test-all-sdk:
	@./scripts/run_all_sdk_tests.sh

test-interop:
	@python3 tests/interop/test_interop_matrix.py

test-security:
	@PYTHONPATH=tests/security:python python3 -m unittest discover -s tests/security

test-bench:
	@./scripts/run_sdk_benchmarks.sh

test-sanitizers:
	@./scripts/run_sanitizers.sh
	@./scripts/run_race_detector.sh

clean:
	@rm -rf .build dist
