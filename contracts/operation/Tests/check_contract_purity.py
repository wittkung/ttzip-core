#!/usr/bin/env python3
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

"""Static AST and source purity verification for TTZipOperationContract."""

import ast
import re
import sys
import tempfile
from pathlib import Path

CONTRACT_DIR = Path(__file__).resolve().parent.parent
BUILD_FILE = CONTRACT_DIR / "BUILD.bazel"
SOURCES_DIR = CONTRACT_DIR / "Sources" / "TTZipOperationContract"

EXPECTED_SOURCES = {
    "ArchiveCompressionFormat.swift",
    "ArchiveCompressionOptions.swift",
    "ArchiveFilter.swift",
    "ArchiveOperationModels.swift",
    "HashType.swift",
    "PrototypeCopyable.swift",
}
ALLOWED_IMPORTS = {"import Foundation", "import TTZipArchiveContract"}
EXPECTED_DEPS = ["//products/ttzip/core/contracts/archive:TTZipArchiveContract"]
EXPECTED_VISIBILITY = [
    "//products/ttzip/core:__pkg__",
    "//products/ttzip/apple:__pkg__",
]
FORBIDDEN_TOKENS = (
    "TTZipCore",
    "TTZipCoreTypes",
    "TTZipCoreBindings",
    "TTZipEngineFFI",
    "UniFfi",
    "TtZipError",
    "CollisionPolicy",
    "CancellationToken",
    "ExtractionOutcome",
    "AppleSiliconTuner",
    "TTLog",
    "ArchiveComponentProtocol",
)


def check_sources(sources_dir: Path = SOURCES_DIR) -> list[str]:
    errors: list[str] = []
    if not sources_dir.is_dir():
        return [f"Missing contract sources directory: {sources_dir}"]
    actual_files = {p.name for p in sources_dir.glob("*.swift")}
    if actual_files != EXPECTED_SOURCES:
        errors.append(f"Expected {sorted(EXPECTED_SOURCES)}, found {sorted(actual_files)}")
    for swift_file in sorted(sources_dir.glob("*.swift")):
        text = swift_file.read_text(encoding="utf-8")
        for line in text.splitlines():
            stripped = line.strip()
            if stripped.startswith("import ") or stripped.startswith("@_exported import "):
                if stripped not in ALLOWED_IMPORTS:
                    errors.append(f"{swift_file.name}: forbidden import '{stripped}'")
        for token in FORBIDDEN_TOKENS:
            matched = ("UniFfi" in text) if token == "UniFfi" else bool(re.search(rf"\b{re.escape(token)}\b", text))
            if matched:
                errors.append(f"{swift_file.name}: forbidden token '{token}'")
    return errors


def _extract_str_list(node: ast.AST) -> list[str] | None:
    if not isinstance(node, ast.List):
        return None
    return [elt.value for elt in node.elts if isinstance(elt, ast.Constant) and isinstance(elt.value, str)]


def _validate_srcs_expr(node: ast.AST) -> list[str]:
    if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == "glob":
        if len(node.args) == 1:
            patterns = _extract_str_list(node.args[0])
            if patterns == ["Sources/TTZipOperationContract/**/*.swift"]:
                return []
        return ["TTZipOperationContract.srcs glob must equal ['Sources/TTZipOperationContract/**/*.swift']"]
    explicit = _extract_str_list(node)
    if explicit is None:
        return ["TTZipOperationContract.srcs must be glob(['Sources/TTZipOperationContract/**/*.swift'])"]
    errs = [f"Cross-package or upward path forbidden in srcs: {s}" for s in explicit if ".." in s or s.startswith("//")]
    expected = [f"Sources/TTZipOperationContract/{name}" for name in sorted(EXPECTED_SOURCES)]
    if sorted(explicit) != expected:
        errs.append(f"TTZipOperationContract.srcs must match {expected}, got {explicit}")
    return errs


def _validate_contract_rule(node: ast.Call) -> tuple[bool, list[str]]:
    kw_map = {kw.arg: kw.value for kw in node.keywords if kw.arg}
    name_node = kw_map.get("name")
    if not (isinstance(name_node, ast.Constant) and name_node.value == "TTZipOperationContract"):
        return False, []
    errors: list[str] = []
    deps_val = _extract_str_list(kw_map.get("deps", ast.Constant(None)))
    if deps_val != EXPECTED_DEPS:
        errors.append(f"TTZipOperationContract.deps must be {EXPECTED_DEPS}, got {deps_val}")
    vis_val = _extract_str_list(kw_map.get("visibility", ast.Constant(None)))
    if vis_val != EXPECTED_VISIBILITY:
        errors.append(f"TTZipOperationContract.visibility must be {EXPECTED_VISIBILITY}, got {vis_val}")
    copts_val = _extract_str_list(kw_map.get("copts", ast.List(elts=[]))) or []
    if "-import-module" in copts_val:
        errors.append("TTZipOperationContract.copts must not inject -import-module")
    srcs_node = kw_map.get("srcs")
    errors.extend(_validate_srcs_expr(srcs_node) if srcs_node is not None else ["Missing srcs in TTZipOperationContract"])
    return True, errors


def check_build_file(build_file: Path = BUILD_FILE) -> list[str]:
    if not build_file.is_file():
        return [f"Missing BUILD.bazel: {build_file}"]
    tree = ast.parse(build_file.read_text(encoding="utf-8"), filename=str(build_file))
    errors: list[str] = []
    found_contract = False
    found_pkg_private = False
    for node in ast.walk(tree):
        if not (isinstance(node, ast.Call) and isinstance(node.func, ast.Name)):
            continue
        if node.func.id == "package":
            for kw in node.keywords:
                if kw.arg == "default_visibility" and _extract_str_list(kw.value) == ["//visibility:private"]:
                    found_pkg_private = True
        elif node.func.id == "swift_library":
            matched, rule_errs = _validate_contract_rule(node)
            if matched:
                found_contract = True
                errors.extend(rule_errs)
    if not found_pkg_private:
        errors.append("BUILD.bazel missing package(default_visibility = ['//visibility:private'])")
    if not found_contract:
        errors.append("BUILD.bazel missing swift_library(name = 'TTZipOperationContract')")
    return errors


def verify_negative_fixtures() -> list[str]:
    with tempfile.TemporaryDirectory() as tmp:
        tmp_dir = Path(tmp)
        bad_srcs = tmp_dir / "Sources" / "TTZipOperationContract"
        bad_srcs.mkdir(parents=True)
        for name in EXPECTED_SOURCES:
            (bad_srcs / name).write_text("import Foundation\n", encoding="utf-8")
        (bad_srcs / "HashType.swift").write_text(
            "import Foundation\nimport TTZipCore\npublic typealias Bad = AppleSiliconTuner\n",
            encoding="utf-8",
        )
        src_errs = check_sources(bad_srcs)
        if not any("forbidden import" in e for e in src_errs) or not any("AppleSiliconTuner" in e for e in src_errs):
            return [f"Negative source fixture failed to trigger expected errors: {src_errs}"]

        bad_build = tmp_dir / "BUILD.bazel"
        bad_build.write_text(
            'package(default_visibility = ["//visibility:public"])\n'
            'swift_library(name = "TTZipOperationContract", srcs = ["../HashType.swift"], '
            'copts = ["-Xfrontend", "-import-module", "-Xfrontend", "TTZipCoreTypes"], '
            'visibility = ["//visibility:public"], '
            'deps = ["//products/ttzip/core/contracts/archive:TTZipArchiveContract", "//products/ttzip/core:TTZipCoreTypes"])\n',
            encoding="utf-8",
        )
        build_errs = check_build_file(bad_build)
        if len(build_errs) < 4:
            return [f"Negative BUILD fixture failed to trigger expected errors: {build_errs}"]
    return []


def main() -> int:
    errs = check_sources() + check_build_file() + verify_negative_fixtures()
    if errs:
        for err in errs:
            print(f"FAIL: {err}", file=sys.stderr)
        return 1
    print("PASS: TTZipOperationContract canonical layout, imports, symbols, BUILD.bazel purity, and negative fixtures verified.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
