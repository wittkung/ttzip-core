#!/usr/bin/env python3
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

"""Minimal Smoke Verification Adapter for TTZip Engine Historical & Current Interfaces.

Validates that C-ABI extract & create entry points are directly callable with
exact struct layouts (V2 vs V3). Executes real multi-file archive extraction,
runs independent system tool cross-verification, and enforces non-zero exit on
any integrity mismatch.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

REPO_ROOT = Path(__file__).resolve().parents[5]

# Default dylib search paths
DEFAULT_CURRENT_DYLIB = Path(
    "/private/var/tmp/_bazel_kevintung/bc7b818d367659311eafb4e72b48b678"
    "/execroot/_main/bazel-out/darwin_arm64-opt/bin"
    "/products/ttzip/core/rust/ttzip-engine/libttzip_engine.dylib"
)

# Canonical Collision Policies from crates/ttzip-types/src/options/mod.rs:16-19
TTZIP_COLLISION_POLICY_KEEP_BOTH = 0
TTZIP_COLLISION_POLICY_REPLACE = 1
TTZIP_COLLISION_POLICY_SKIP = 2
TTZIP_COLLISION_POLICY_STOP = 3

# ---------------------------------------------------------------------------
# Struct Definitions
# ---------------------------------------------------------------------------

class CurrentExtractOptionsV3(ctypes.Structure):
    """C-ABI layout matching TTZipExtractOptions (V3) in crates/ttzip-types/src/options/mod.rs."""
    _fields_ = [
        ("struct_size", ctypes.c_uint32),
        ("abi_version", ctypes.c_uint32),
        ("destination_path", ctypes.c_char_p),
        ("password", ctypes.c_char_p),
        ("thread_budget", ctypes.c_uint32),
        ("collision_policy", ctypes.c_uint8),  # 0: KEEP_BOTH, 1: REPLACE, 2: SKIP, 3: STOP
        ("preserve_permissions", ctypes.c_bool),
        ("dry_run", ctypes.c_bool),
        ("progress_callback", ctypes.c_void_p),
        ("user_data", ctypes.c_void_p),
    ]

class HistoricalExtractOptionsV2(ctypes.Structure):
    """C-ABI layout matching TTZipExtractOptions (V2) in commit 06348231e~1."""
    _fields_ = [
        ("struct_size", ctypes.c_uint32),
        ("abi_version", ctypes.c_uint32),
        ("destination_path", ctypes.c_char_p),
        ("password", ctypes.c_char_p),
        ("thread_budget", ctypes.c_uint32),
        ("overwrite_existing", ctypes.c_bool),
        ("preserve_permissions", ctypes.c_bool),
        ("dry_run", ctypes.c_bool),
        ("progress_callback", ctypes.c_void_p),
        ("user_data", ctypes.c_void_p),
    ]

class CurrentCreateOptionsV3(ctypes.Structure):
    """C-ABI layout matching TTZipCreateOptions (V3) in crates/ttzip-types/src/options/mod.rs."""
    _fields_ = [
        ("struct_size", ctypes.c_uint32),
        ("abi_version", ctypes.c_uint32),
        ("format", ctypes.c_int32),
        ("level", ctypes.c_int32),
        ("encryption", ctypes.c_int32),
        ("password", ctypes.c_char_p),
        ("thread_budget", ctypes.c_uint32),
        ("solid_block_size_mb", ctypes.c_uint32),
        ("progress_callback", ctypes.c_void_p),
        ("user_data", ctypes.c_void_p),
    ]

# ---------------------------------------------------------------------------
# Integrity & Environment Helpers
# ---------------------------------------------------------------------------

def sha256_bytes(data: bytes) -> str:
    h = hashlib.sha256()
    h.update(data)
    return h.hexdigest()

def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()

def get_git_info(repo: Path) -> Dict[str, Any]:
    try:
        commit = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=repo, text=True, stderr=subprocess.DEVNULL
        ).strip()
        status = subprocess.check_output(
            ["git", "status", "--porcelain"], cwd=repo, text=True, stderr=subprocess.DEVNULL
        )
        has_dirty = len(status.strip()) > 0
        return {
            "git_head_commit": commit,
            "working_tree_dirty": has_dirty,
            "source_snapshot": f"{commit}{'+dirty' if has_dirty else ''}",
        }
    except Exception:
        return {
            "git_head_commit": "UNRECORDED",
            "working_tree_dirty": None,
            "source_snapshot": "UNRECORDED",
        }

def tree_manifest_sha256(root_dir: Path) -> Tuple[int, int, str]:
    manifest_entries: List[str] = []
    total_bytes = 0
    file_count = 0
    for dirpath, _, filenames in sorted(os.walk(root_dir)):
        for fn in sorted(filenames):
            fp = Path(dirpath) / fn
            if fp.is_file():
                rel = fp.relative_to(root_dir).as_posix()
                content = fp.read_bytes()
                f_hash = sha256_bytes(content)
                manifest_entries.append(f"{rel}:{len(content)}:{f_hash}")
                total_bytes += len(content)
                file_count += 1
    full_manifest = "\n".join(manifest_entries).encode("utf-8")
    return file_count, total_bytes, sha256_bytes(full_manifest)

# ---------------------------------------------------------------------------
# Runner Logic: Current Engine
# ---------------------------------------------------------------------------

def run_current_engine_smoke(dylib_path: Path, work_dir: Path) -> Tuple[Dict[str, Any], Path, int, int, str]:
    if not dylib_path.exists():
        raise FileNotFoundError(f"Current dylib not found at: {dylib_path}")

    git_info = get_git_info(REPO_ROOT)
    dylib_hash = sha256_file(dylib_path)

    lib = ctypes.CDLL(str(dylib_path))

    # Signature: fn(archive_path, destination_path, options, out_extracted_bytes, out_error) -> u32
    lib.ttzip_rust_archive_extract_unified_v2.argtypes = [
        ctypes.c_char_p,
        ctypes.c_char_p,
        ctypes.POINTER(CurrentExtractOptionsV3),
        ctypes.POINTER(ctypes.c_uint64),
        ctypes.c_void_p,
    ]
    lib.ttzip_rust_archive_extract_unified_v2.restype = ctypes.c_uint32

    # Signature: fn(src_paths, count, dest_path, options, split_size) -> u32
    lib.ttzip_rust_archive_create_unified.argtypes = [
        ctypes.POINTER(ctypes.c_char_p),
        ctypes.c_size_t,
        ctypes.c_char_p,
        ctypes.POINTER(CurrentCreateOptionsV3),
        ctypes.c_uint64,
    ]
    lib.ttzip_rust_archive_create_unified.restype = ctypes.c_uint32

    # Build source corpus
    src_dir = work_dir / "smoke_src"
    src_dir.mkdir(parents=True, exist_ok=True)
    (src_dir / "sub1").mkdir(exist_ok=True)
    (src_dir / "sub2").mkdir(exist_ok=True)
    (src_dir / "hello.txt").write_text("Hello MetastudyLine TTZip Smoke Test\n", encoding="utf-8")
    (src_dir / "sub1" / "nested.dat").write_bytes(b"\x01\x02\x03\x04" * 1024)
    (src_dir / "sub2" / "log.txt").write_text("Smoke log line 1\nSmoke log line 2\n", encoding="utf-8")

    src_files, src_bytes, src_manifest = tree_manifest_sha256(src_dir)

    # 1. Create TAR.GZ via C-ABI
    arc_path = work_dir / "smoke_output.tar.gz"
    if arc_path.exists():
        arc_path.unlink()

    entries = sorted(p for p in src_dir.iterdir())
    Arr = ctypes.c_char_p * len(entries)
    arr = Arr(*[str(p).encode("utf-8") for p in entries])

    c_opts = CurrentCreateOptionsV3()
    c_opts.struct_size = ctypes.sizeof(CurrentCreateOptionsV3)
    c_opts.abi_version = 3
    c_opts.format = 63  # FMT_TAR_GZ
    c_opts.level = 6     # LVL_NORMAL
    c_opts.encryption = 0
    c_opts.password = None
    c_opts.thread_budget = 0
    c_opts.solid_block_size_mb = 0
    c_opts.progress_callback = None
    c_opts.user_data = None

    t0_c = time.perf_counter_ns()
    rc_c = lib.ttzip_rust_archive_create_unified(
        arr, len(entries), str(arc_path).encode("utf-8"), ctypes.byref(c_opts), 0
    )
    t1_c = time.perf_counter_ns()
    if rc_c != 0:
        raise RuntimeError(f"Current engine create failed with status code: {rc_c}")

    arc_size = arc_path.stat().st_size
    if arc_size == 0:
        raise RuntimeError("Current engine created an empty archive file.")

    # 2. Extract via C-ABI (5 arguments)
    ext_dir = work_dir / "smoke_extracted_ttzip"
    if ext_dir.exists():
        shutil.rmtree(ext_dir)
    ext_dir.mkdir(parents=True, exist_ok=True)

    e_opts = CurrentExtractOptionsV3()
    e_opts.struct_size = ctypes.sizeof(CurrentExtractOptionsV3)
    e_opts.abi_version = 3
    e_opts.destination_path = str(ext_dir).encode("utf-8")
    e_opts.password = None
    e_opts.thread_budget = 0
    # Explicit mapping: 1 is REPLACE (0 is KEEP_BOTH)
    e_opts.collision_policy = TTZIP_COLLISION_POLICY_REPLACE
    e_opts.preserve_permissions = True
    e_opts.dry_run = False
    e_opts.progress_callback = None
    e_opts.user_data = None

    out_bytes = ctypes.c_uint64(0)
    t0_e = time.perf_counter_ns()
    rc_e = lib.ttzip_rust_archive_extract_unified_v2(
        str(arc_path).encode("utf-8"),
        str(ext_dir).encode("utf-8"),
        ctypes.byref(e_opts),
        ctypes.byref(out_bytes),
        None,
    )
    t1_e = time.perf_counter_ns()
    if rc_e != 0:
        raise RuntimeError(f"Current engine extract failed with status code: {rc_e}")

    ext_files, ext_bytes, ext_manifest = tree_manifest_sha256(ext_dir)
    engine_extract_passed = (
        ext_files == src_files and ext_bytes == src_bytes and ext_manifest == src_manifest
    )

    # 3. Independent Cross-Verification via Python tarfile
    cross_dir = work_dir / "smoke_extracted_cross_tar"
    if cross_dir.exists():
        shutil.rmtree(cross_dir)
    cross_dir.mkdir(parents=True, exist_ok=True)

    with tarfile.open(arc_path, "r:gz") as tar:
        tar.extractall(cross_dir)

    cross_files, cross_bytes, cross_manifest = tree_manifest_sha256(cross_dir)
    cross_verify_passed = (
        cross_files == src_files and cross_bytes == src_bytes and cross_manifest == src_manifest
    )

    if not engine_extract_passed:
        raise AssertionError(
            f"TTZip extract manifest mismatch! expected={src_manifest}, actual={ext_manifest}"
        )
    if not cross_verify_passed:
        raise AssertionError(
            f"Independent tar cross-extract manifest mismatch! expected={src_manifest}, actual={cross_manifest}"
        )

    res = {
        "engine_label": "Current Engine",
        "git_commit": git_info["git_head_commit"],
        "working_tree_dirty": git_info["working_tree_dirty"],
        "source_snapshot": git_info["source_snapshot"],
        "dylib_path": str(dylib_path),
        "dylib_sha256": dylib_hash,
        "c_abi_extract_function": "ttzip_rust_archive_extract_unified_v2",
        "extract_argument_count": 5,
        "options_struct_version": "V3 (collision_policy: u8 = REPLACE(1))",
        "source_corpus_files": src_files,
        "source_corpus_bytes": src_bytes,
        "source_manifest_sha256": src_manifest,
        "created_archive_bytes": arc_size,
        "smoke_create_elapsed_ms_observation_only": round((t1_c - t0_c) / 1e6, 3),
        "extracted_corpus_files": ext_files,
        "extracted_corpus_bytes": ext_bytes,
        "extracted_manifest_sha256": ext_manifest,
        "extract_reported_bytes": out_bytes.value,
        "smoke_extract_elapsed_ms_observation_only": round((t1_e - t0_e) / 1e6, 3),
        "ttzip_extract_integrity_verified": engine_extract_passed,
        "independent_tool_cross_verify_verified": cross_verify_passed,
        "all_integrity_verified": engine_extract_passed and cross_verify_passed,
    }
    return res, arc_path, src_files, src_bytes, src_manifest

# ---------------------------------------------------------------------------
# Runner Logic: Historical Engine (Commit 06348231e~1)
# ---------------------------------------------------------------------------

def run_historical_engine_smoke(
    dylib_path: Path,
    work_dir: Path,
    archive_path: Path,
    expected_files: int,
    expected_bytes: int,
    expected_manifest: str,
) -> Dict[str, Any]:
    if not dylib_path.exists():
        raise FileNotFoundError(f"Historical dylib not found at: {dylib_path}")

    dylib_hash = sha256_file(dylib_path)
    lib = ctypes.CDLL(str(dylib_path))

    # Historical signature: fn(archive_path, destination_path, options) -> u32
    # Exactly 3 arguments
    lib.ttzip_rust_extract_archive.argtypes = [
        ctypes.c_char_p,
        ctypes.c_char_p,
        ctypes.POINTER(HistoricalExtractOptionsV2),
    ]
    lib.ttzip_rust_extract_archive.restype = ctypes.c_uint32

    ext_dir = work_dir / "smoke_extracted_historical"
    if ext_dir.exists():
        shutil.rmtree(ext_dir)
    ext_dir.mkdir(parents=True, exist_ok=True)

    h_opts = HistoricalExtractOptionsV2()
    h_opts.struct_size = ctypes.sizeof(HistoricalExtractOptionsV2)
    h_opts.abi_version = 2
    h_opts.destination_path = str(ext_dir).encode("utf-8")
    h_opts.password = None
    h_opts.thread_budget = 0
    h_opts.overwrite_existing = True
    h_opts.preserve_permissions = True
    h_opts.dry_run = False
    h_opts.progress_callback = None
    h_opts.user_data = None

    t0_h = time.perf_counter_ns()
    rc_h = lib.ttzip_rust_extract_archive(
        str(archive_path).encode("utf-8"),
        str(ext_dir).encode("utf-8"),
        ctypes.byref(h_opts),
    )
    t1_h = time.perf_counter_ns()
    if rc_h != 0:
        raise RuntimeError(f"Historical engine extract failed with status code: {rc_h}")

    ext_files, ext_bytes, ext_manifest = tree_manifest_sha256(ext_dir)
    hist_extract_passed = (
        ext_files == expected_files
        and ext_bytes == expected_bytes
        and ext_manifest == expected_manifest
    )

    if not hist_extract_passed:
        raise AssertionError(
            f"Historical engine extract manifest mismatch! "
            f"expected={expected_manifest}, actual={ext_manifest}"
        )

    return {
        "engine_label": "Historical Engine (commit 06348231e~1)",
        "status": "VERIFIED_PASSED",
        "adapter_state": "VERIFIED_DUAL_SIDED",
        "c_abi_extract_function": "ttzip_rust_extract_archive",
        "extract_argument_count": 3,
        "options_struct_version": "V2 (overwrite_existing: bool = true)",
        "dylib_path": str(dylib_path),
        "dylib_sha256": dylib_hash,
        "extracted_corpus_files": ext_files,
        "extracted_corpus_bytes": ext_bytes,
        "extracted_manifest_sha256": ext_manifest,
        "smoke_extract_elapsed_ms_observation_only": round((t1_h - t0_h) / 1e6, 3),
        "historical_extract_integrity_verified": hist_extract_passed,
        "all_integrity_verified": hist_extract_passed,
    }

# ---------------------------------------------------------------------------
# CLI Entry Point
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Smoke verification adapter for TTZip engine.")
    parser.add_argument(
        "--current-dylib",
        type=Path,
        default=DEFAULT_CURRENT_DYLIB,
        help="Path to current libttzip_engine.dylib",
    )
    parser.add_argument(
        "--historical-dylib",
        type=Path,
        default=None,
        help="Path to historical libttzip_engine.dylib (if built)",
    )
    parser.add_argument(
        "--work-dir",
        type=Path,
        default=Path("/tmp/ttzip_smoke_validation"),
        help="Temporary directory for smoke files",
    )
    args = parser.parse_args()

    work_dir = args.work_dir
    if work_dir.exists():
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    print("=== TTZip Q4 Historical & Current Engine Smoke Validation ===")

    # 1. Run Current Engine Smoke Test
    current_res: Optional[Dict[str, Any]] = None
    arc_path: Optional[Path] = None
    try:
        current_res, arc_path, src_files, src_bytes, src_manifest = run_current_engine_smoke(
            args.current_dylib, work_dir
        )
        print("\n[Current Engine Smoke Results]:")
        print(json.dumps(current_res, indent=2))
    except Exception as exc:
        print(f"\n[FATAL ERROR] Current engine smoke validation failed: {exc}", file=sys.stderr)
        sys.exit(1)

    # 2. Run Historical Engine Smoke Test (if dylib specified)
    historical_status: Dict[str, Any]
    if args.historical_dylib is not None:
        if not args.historical_dylib.exists():
            print(
                f"\n[FATAL ERROR] Specified historical dylib does not exist: {args.historical_dylib}",
                file=sys.stderr,
            )
            sys.exit(1)
        try:
            assert arc_path is not None
            historical_status = run_historical_engine_smoke(
                args.historical_dylib,
                work_dir,
                arc_path,
                src_files,
                src_bytes,
                src_manifest,
            )
            print("\n[Historical Engine Smoke Results]:")
            print(json.dumps(historical_status, indent=2))
        except Exception as exc:
            print(f"\n[FATAL ERROR] Historical engine smoke validation failed: {exc}", file=sys.stderr)
            sys.exit(1)
    else:
        historical_status = {
            "engine_label": "Historical Engine (commit 06348231e~1)",
            "status": "PENDING_DYLIB",
            "adapter_state": "STRUCT_AND_CALLER_IMPLEMENTED_PENDING_DYLIB",
            "c_abi_extract_function": "ttzip_rust_extract_archive",
            "extract_argument_count": 3,
            "options_struct_version": "V2 (overwrite_existing: bool)",
            "dylib_path": "NOT_PROVIDED",
            "dylib_sha256": "UNRECORDED",
            "notes": (
                "Historical adapter C-ABI caller (3 args) and V2 struct are fully implemented. "
                "Pass --historical-dylib <path> to run dual-version smoke verification."
            ),
        }

    report = {
        "schema": "ttzip.q4.smoke_verification.v4",
        "timestamp_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "current_engine": current_res,
        "historical_engine": historical_status,
    }

    out_log = REPO_ROOT / "products/ttzip/docs/acceptance/q4-history/smoke-verification-log.json"
    out_log.parent.mkdir(parents=True, exist_ok=True)
    out_log.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    # Final assertion before declaring success
    if not current_res or not current_res.get("all_integrity_verified"):
        print("\n[FAILURE] Current engine integrity assertions failed! Exiting non-zero.", file=sys.stderr)
        sys.exit(1)

    if args.historical_dylib is not None:
        if not historical_status.get("all_integrity_verified"):
            print("\n[FAILURE] Historical engine integrity assertions failed! Exiting non-zero.", file=sys.stderr)
            sys.exit(1)

    print(f"\n[OK] Smoke test passed! All integrity assertions 100% verified.")
    print(f"Log recorded to: {out_log}")
    sys.exit(0)

if __name__ == "__main__":
    main()
