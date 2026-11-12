#!/usr/bin/env python3
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

"""Contract tests for standalone rust_test input trimming in ttzip-engine BUILD.bazel."""

from __future__ import annotations

import glob as py_glob
import re
import unittest
from pathlib import Path
from typing import Any

ENGINE_DIR = Path(__file__).resolve().parent
BUILD_BAZEL_PATH = ENGINE_DIR / "BUILD.bazel"

EXPECTED_STANDALONE_SRCS: tuple[str, ...] = (
    "tests/xz_vli_tests.rs",
    "tests/zip_datetime_tests.rs",
)

EXPECTED_XZ_VLI_TESTS: tuple[str, ...] = (
    "test_ladder_boundaries_roundtrip",
    "test_vli_size_exhaustive_consistency",
    "test_vli_overflow_rejection",
    "test_non_canonical_encoding_rejection",
    "test_sequence_too_long_circuit_breaker",
    "test_unexpected_eof_on_truncated_buffers",
    "test_buffer_too_small_on_encode",
    "test_sequential_concatenated_vlis",
)

EXPECTED_ZIP_DATETIME_TESTS: tuple[str, ...] = (
    "test_leap_year_rules",
    "test_days_in_month_boundaries",
    "test_dos_datetime_min_boundary_1980",
    "test_dos_datetime_max_boundary_2107",
    "test_dos_datetime_current_timestamp_2026",
    "test_dos_datetime_all_zero_safe_fallback",
    "test_dos_datetime_invalid_values_rejection",
    "test_dos_datetime_epoch_clamping",
)

FORBIDDEN_HARNESS_PATTERNS: tuple[re.Pattern[str], ...] = (
    re.compile(r"\bmod\s+common\b"),
    re.compile(r"\bmod\s+corrupted_harness\b"),
    re.compile(r"\bmod\s+fixtures\b"),
    re.compile(r"\bmod\s+formats_matrix_harness\b"),
    re.compile(r"\bmod\s+fuzz_harness\b"),
    re.compile(r"tests/archives"),
    re.compile(r"tests/fixtures"),
    re.compile(r"\binclude_bytes!\b"),
    re.compile(r"\binclude_str!\b"),
    re.compile(r"\bCARGO_MANIFEST_DIR\b"),
)


def evaluate_build_bazel(build_path: Path) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    rust_tests: dict[str, dict[str, Any]] = {}

    def bazel_glob(include: list[str], exclude: list[str] | None = None) -> list[str]:
        excluded: set[str] = set()
        for pattern in exclude or []:
            for match in py_glob.glob(str(ENGINE_DIR / pattern), recursive=True):
                rel = Path(match).resolve().relative_to(ENGINE_DIR).as_posix()
                excluded.add(rel)
        results: set[str] = set()
        for pattern in include:
            for match in py_glob.glob(str(ENGINE_DIR / pattern), recursive=True):
                path_obj = Path(match).resolve()
                if path_obj.is_file():
                    rel = path_obj.relative_to(ENGINE_DIR).as_posix()
                    if rel not in excluded:
                        results.add(rel)
        return sorted(results)

    def record_rust_test(**kwargs: Any) -> None:
        rust_tests[kwargs["name"]] = kwargs

    env: dict[str, Any] = {
        "__builtins__": {
            "len": len,
            "True": True,
            "False": False,
            "None": None,
        },
        "load": lambda *_args, **_kwargs: None,
        "package": lambda **_kwargs: None,
        "exports_files": lambda *_args, **_kwargs: None,
        "glob": bazel_glob,
        "select": lambda mapping: mapping,
        "rust_library": lambda **_kwargs: None,
        "rust_static_library": lambda **_kwargs: None,
        "rust_shared_library": lambda **_kwargs: None,
        "rust_test": record_rust_test,
        "test_suite": lambda **_kwargs: None,
    }
    source = build_path.read_text(encoding="utf-8")
    exec(compile(source, str(build_path), "exec"), env)
    return env, rust_tests


def extract_rust_test_names(source_path: Path) -> tuple[str, ...]:
    text = source_path.read_text(encoding="utf-8")
    pattern = re.compile(r"#\[test\]\s*fn\s+([A-Za-z0-9_]+)\s*\(")
    return tuple(pattern.findall(text))


class BazelHarnessInputsTest(unittest.TestCase):
    def test_standalone_test_srcs_declared_and_trimmed(self) -> None:
        env, rust_tests = evaluate_build_bazel(BUILD_BAZEL_PATH)
        self.assertIn(
            "STANDALONE_TEST_SRCS",
            env,
            "BUILD.bazel must define STANDALONE_TEST_SRCS",
        )
        standalone_srcs = env["STANDALONE_TEST_SRCS"]
        self.assertIsInstance(standalone_srcs, list)
        self.assertEqual(len(standalone_srcs), len(set(standalone_srcs)))
        for expected in EXPECTED_STANDALONE_SRCS:
            self.assertIn(expected, standalone_srcs)

        integration_srcs = set(env["INTEGRATION_TEST_SRCS"])
        workspace_unit_srcs = set(env["WORKSPACE_UNIT_TEST_SRCS"])
        harness_srcs = set(env["TEST_HARNESS_SRCS"])
        data_files = set(env["TEST_DATA_FILES"])
        self.assertGreaterEqual(len(harness_srcs), 20)
        self.assertGreaterEqual(len(data_files), 85)

        for src in standalone_srcs:
            self.assertIn(src, integration_srcs)
            self.assertIn(src, workspace_unit_srcs)
            target_name = src[len("tests/") : -len(".rs")]
            self.assertIn(target_name, rust_tests)
            rule = rust_tests[target_name]
            self.assertEqual(
                rule["srcs"],
                [src],
                f"{target_name} must declare srcs = [{src!r}] without TEST_HARNESS_SRCS",
            )
            self.assertEqual(
                rule["compile_data"],
                ["Cargo.toml"],
                f"{target_name} must declare compile_data = ['Cargo.toml'] without TEST_DATA_FILES",
            )
            self.assertEqual(
                rule["data"],
                [],
                f"{target_name} must declare data = [] without TEST_DATA_FILES",
            )
            self.assertEqual(rule["tags"], ["workspace-unit"])

        harness_dependent_rule = rust_tests["deferred_fixup_and_sparse_tests"]
        self.assertTrue(harness_srcs.issubset(set(harness_dependent_rule["srcs"])))
        self.assertTrue(data_files.issubset(set(harness_dependent_rule["compile_data"])))
        self.assertTrue(data_files.issubset(set(harness_dependent_rule["data"])))

    def test_standalone_sources_do_not_reference_harnesses_or_fixtures(self) -> None:
        env, _ = evaluate_build_bazel(BUILD_BAZEL_PATH)
        standalone_srcs = env.get("STANDALONE_TEST_SRCS", list(EXPECTED_STANDALONE_SRCS))
        for rel_src in standalone_srcs:
            src_path = ENGINE_DIR / rel_src
            self.assertTrue(src_path.is_file(), f"Missing standalone source {src_path}")
            content = src_path.read_text(encoding="utf-8")
            for pattern in FORBIDDEN_HARNESS_PATTERNS:
                self.assertIsNone(
                    pattern.search(content),
                    f"{rel_src} matched forbidden harness/fixture pattern {pattern.pattern!r}",
                )

    def test_xz_vli_and_zip_datetime_preserve_all_16_tests(self) -> None:
        xz_tests = extract_rust_test_names(ENGINE_DIR / "tests/xz_vli_tests.rs")
        zip_tests = extract_rust_test_names(ENGINE_DIR / "tests/zip_datetime_tests.rs")
        self.assertEqual(xz_tests, EXPECTED_XZ_VLI_TESTS)
        self.assertEqual(zip_tests, EXPECTED_ZIP_DATETIME_TESTS)
        self.assertEqual(len(xz_tests) + len(zip_tests), 16)


if __name__ == "__main__":
    unittest.main(verbosity=2)
