#!/usr/bin/env python3
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

"""Static AST and source purity verification for TTZipArchiveContract."""

import ast
import re
import sys
import tempfile
from pathlib import Path

CONTRACT_DIR = Path(__file__).resolve().parent.parent
BUILD_FILE = CONTRACT_DIR / "BUILD.bazel"
SOURCES_DIR = CONTRACT_DIR / "Sources" / "TTZipArchiveContract"

EXPECTED_SOURCES = {"ArchiveEntry.swift", "ArchiveMimeMapper.swift"}
FORBIDDEN_TOKENS = (
    "FormatterCaches",
    "ByteCountFormatter",
    "ByteCountFormatterFlyweight",
    "DateFormatterCache",
    "formattedSize",
    "TTZipCore",
    "TTZipCoreTypes",
    "TTZipCoreBindings",
    "TTZipEngineFFI",
    "TTLog",
)


def check_sources(sources_dir: Path = SOURCES_DIR) -> list[str]:
    errors: list[str] = []
    if not sources_dir.is_dir():
        return [f"Missing contract sources directory: {sources_dir}"]
    actual_files = {p.name for p in sources_dir.glob("*.swift")}
    if actual_files != EXPECTED_SOURCES:
        errors.append(f"Expected {EXPECTED_SOURCES}, found {actual_files}")
    for swift_file in sorted(sources_dir.glob("*.swift")):
        text = swift_file.read_text(encoding="utf-8")
        for line in text.splitlines():
            stripped = line.strip()
            if stripped.startswith("import ") or stripped.startswith("@_exported import "):
                if stripped != "import Foundation":
                    errors.append(f"{swift_file.name}: forbidden import '{stripped}'")
        for token in FORBIDDEN_TOKENS:
            if re.search(rf"\b{re.escape(token)}\b", text):
                errors.append(f"{swift_file.name}: forbidden token '{token}'")
        if swift_file.name == "ArchiveEntry.swift":
            if re.search(r"ArchiveEntry\s*:[^{]*\bHashable\b", text):
                errors.append("ArchiveEntry.swift: Hashable conformance is forbidden in M1")
    return errors


def check_build_file(build_file: Path = BUILD_FILE) -> list[str]:
    errors: list[str] = []
    if not build_file.is_file():
        return [f"Missing BUILD.bazel: {build_file}"]
    tree = ast.parse(build_file.read_text(encoding="utf-8"), filename=str(build_file))
    found_contract = False
    found_pkg_private = False
    for node in ast.walk(tree):
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name):
            if node.func.id == "package":
                for kw in node.keywords:
                    if kw.arg == "default_visibility" and isinstance(kw.value, ast.List):
                        vals = [e.value for e in kw.value.elts if isinstance(e, ast.Constant)]
                        if vals == ["//visibility:private"]:
                            found_pkg_private = True
            elif node.func.id == "swift_library":
                name_val = None
                deps_val = None
                srcs_val = None
                copts_val = None
                for kw in node.keywords:
                    if kw.arg == "name" and isinstance(kw.value, ast.Constant):
                        name_val = kw.value.value
                    elif kw.arg == "deps" and isinstance(kw.value, ast.List):
                        deps_val = [e.value for e in kw.value.elts if isinstance(e, ast.Constant)]
                    elif kw.arg == "srcs" and isinstance(kw.value, ast.List):
                        srcs_val = [e.value for e in kw.value.elts if isinstance(e, ast.Constant)]
                    elif kw.arg == "copts" and isinstance(kw.value, ast.List):
                        copts_val = [e.value for e in kw.value.elts if isinstance(e, ast.Constant)]
                if name_val == "TTZipArchiveContract":
                    found_contract = True
                    if deps_val != []:
                        errors.append(f"TTZipArchiveContract.deps must be [], got {deps_val}")
                    if copts_val and "-import-module" in copts_val:
                        errors.append("TTZipArchiveContract.copts must not inject -import-module")
                    expected_srcs = [
                        "Sources/TTZipArchiveContract/ArchiveEntry.swift",
                        "Sources/TTZipArchiveContract/ArchiveMimeMapper.swift",
                    ]
                    if srcs_val != expected_srcs:
                        errors.append(f"TTZipArchiveContract.srcs must be {expected_srcs}, got {srcs_val}")
                    for s in srcs_val or []:
                        if ".." in s or s.startswith("//"):
                            errors.append(f"Cross-package or upward path forbidden in srcs: {s}")
    if not found_pkg_private:
        errors.append("BUILD.bazel missing package(default_visibility = ['//visibility:private'])")
    if not found_contract:
        errors.append("BUILD.bazel missing swift_library(name = 'TTZipArchiveContract')")
    return errors


def verify_negative_fixtures() -> list[str]:
    """Verify that forbidden imports, Hashable, and illegal BUILD deps are rejected."""
    with tempfile.TemporaryDirectory() as tmp:
        tmp_dir = Path(tmp)
        bad_srcs = tmp_dir / "Sources" / "TTZipArchiveContract"
        bad_srcs.mkdir(parents=True)
        (bad_srcs / "ArchiveEntry.swift").write_text(
            "import Foundation\nimport TTZipCoreBindings\npublic struct ArchiveEntry: Hashable {}\n",
            encoding="utf-8",
        )
        (bad_srcs / "ArchiveMimeMapper.swift").write_text(
            "import Foundation\npublic enum ArchiveMimeMapper {}\n",
            encoding="utf-8",
        )
        src_errs = check_sources(bad_srcs)
        if not any("forbidden import" in e for e in src_errs) or not any("Hashable" in e for e in src_errs):
            return [f"Negative source fixture failed to trigger expected errors: {src_errs}"]

        bad_build = tmp_dir / "BUILD.bazel"
        bad_build.write_text(
            'package(default_visibility = ["//visibility:public"])\n'
            'swift_library(name = "TTZipArchiveContract", srcs = ["../ArchiveEntry.swift"], '
            'copts = ["-Xfrontend", "-import-module", "-Xfrontend", "TTZipCoreBindings"], '
            'deps = ["//products/ttzip/core:TTZipCoreBindings"])\n',
            encoding="utf-8",
        )
        build_errs = check_build_file(bad_build)
        if len(build_errs) < 3:
            return [f"Negative BUILD fixture failed to trigger expected errors: {build_errs}"]
    return []


def main() -> int:
    errs = check_sources() + check_build_file() + verify_negative_fixtures()
    if errs:
        for e in errs:
            print(f"FAIL: {e}", file=sys.stderr)
        return 1
    print("PASS: TTZipArchiveContract canonical layout, imports, symbols, BUILD.bazel purity, and negative fixtures verified.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
