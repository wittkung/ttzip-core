#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# TTZip: Cross-Language N x N Interoperability Matrix Test Orchestrator.
# Implements automated round-trip matrix validation across Tier-1 SDKs:
# Python (PyO3), C11 Native, Modern C++20, Go (CGO), Java 22+ (Panama FFM), Dart (FFI).

import os
import sys
import json
import time
import shutil
import hashlib
import tempfile
import unittest
import subprocess
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Any

# Root paths
SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent.parent
RUST_RELEASE = REPO_ROOT / "rust" / "target" / "release"
CONTRACT_PATH = REPO_ROOT.parent / "specs" / "006-multi-language-sdk-automated-testing-framework" / "contracts" / "interop-matrix-contract.json"
if not CONTRACT_PATH.exists():
    CONTRACT_PATH = REPO_ROOT / "specs" / "006-multi-language-sdk-automated-testing-framework" / "contracts" / "interop-matrix-contract.json"


import unicodedata


def compute_sha256(filepath: Path) -> str:
    """Computes SHA-256 hex digest for a single file."""
    h = hashlib.sha256()
    with open(filepath, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()


def compute_recursive_sha256(root_dir: Path) -> str:
    """
    Computes deterministic recursive SHA-256 checksum of directory contents or file.
    Normalizes relative paths using Unicode NFC for cross-platform/APFS consistency.
    Unwraps single top-level directory if present.
    """
    if root_dir.is_file():
        return compute_sha256(root_dir)

    subdirs = list(root_dir.iterdir()) if root_dir.is_dir() else []
    if len(subdirs) == 1 and subdirs[0].is_dir():
        root_dir = subdirs[0]

    hasher = hashlib.sha256()
    all_files: List[Tuple[str, Path]] = []

    for item in root_dir.rglob("*"):
        if item.is_file():
            rel_path = unicodedata.normalize("NFC", item.relative_to(root_dir).as_posix())
            all_files.append((rel_path, item))

    all_files.sort(key=lambda x: x[0])

    for rel_path, file_path in all_files:
        file_sha = compute_sha256(file_path)
        hasher.update(rel_path.encode("utf-8"))
        hasher.update(b":")
        hasher.update(file_sha.encode("utf-8"))
        hasher.update(b";")

    return hasher.hexdigest()


def prepare_datasets(temp_root: Path) -> Dict[str, Tuple[Path, str]]:
    """
    Generates standard canonical test fixtures:
    1. text: single text document
    2. nested: multi-level directory hierarchy with varying files
    3. unicode: files with CJK, accented, and Emoji names
    4. large: compressible pseudorandom binary payload (~512KB)
    """
    datasets: Dict[str, Tuple[Path, str]] = {}

    # 1. Text dataset
    text_dir = temp_root / "dataset_text"
    text_dir.mkdir(parents=True, exist_ok=True)
    text_file = text_dir / "document.txt"
    text_file.write_text("TTZip High-Performance Cross-Language Matrix Test Payload\n" * 100, encoding="utf-8")
    datasets["text"] = (text_dir, compute_recursive_sha256(text_dir))

    # 2. Nested dataset
    nested_dir = temp_root / "dataset_nested"
    nested_dir.mkdir(parents=True, exist_ok=True)
    (nested_dir / "root.txt").write_text("Root level content\n", encoding="utf-8")
    sub1 = nested_dir / "level1" / "level2" / "level3"
    sub1.mkdir(parents=True, exist_ok=True)
    (sub1 / "deep.txt").write_text("Deeply nested file payload in level3\n", encoding="utf-8")
    (nested_dir / "level1" / "sibling.txt").write_text("Sibling file in level1\n", encoding="utf-8")
    datasets["nested"] = (nested_dir, compute_recursive_sha256(nested_dir))

    # 3. Unicode dataset
    unicode_dir = temp_root / "dataset_unicode"
    unicode_dir.mkdir(parents=True, exist_ok=True)
    (unicode_dir / "中文测试_文档.txt").write_text("TTZip 跨语言一致性验证\n", encoding="utf-8")
    (unicode_dir / "日本語_テスト.txt").write_text("高速アーカイブエンジン\n", encoding="utf-8")
    (unicode_dir / "한국어_테스트.txt").write_text("초고속 네이티브 압축\n", encoding="utf-8")
    (unicode_dir / "🚀_rocket_emoji.txt").write_text("Rocket speed archiving with ARM PMULL\n", encoding="utf-8")
    datasets["unicode"] = (unicode_dir, compute_recursive_sha256(unicode_dir))

    # 4. Large compressible binary dataset (~512KB)
    large_dir = temp_root / "dataset_large"
    large_dir.mkdir(parents=True, exist_ok=True)
    large_file = large_dir / "data.bin"
    chunk = b"TTZIP_BINARY_COMPRESSIBLE_BLOCK_DATA_STREAM_0123456789ABCDEF\n" * 16
    with open(large_file, "wb") as f:
        for _ in range(512):
            f.write(chunk)
    datasets["large"] = (large_dir, compute_recursive_sha256(large_dir))

    return datasets


class SdkRunner:
    """Abstract interface to launch headless CLI for a specific SDK."""

    def __init__(self, name: str):
        self.name = name

    def is_available(self) -> bool:
        raise NotImplementedError

    def create_archive(self, format_name: str, src_path: Path, dst_archive: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        raise NotImplementedError

    def extract_archive(self, src_archive: Path, dst_dir: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        raise NotImplementedError


class PythonSdkRunner(SdkRunner):
    def __init__(self):
        super().__init__("python")
        self.cli_path = REPO_ROOT / "python" / "interop_cli.py"

    def is_available(self) -> bool:
        return self.cli_path.exists() and shutil.which("python3") is not None

    def create_archive(self, format_name: str, src_path: Path, dst_archive: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        cmd = [sys.executable, str(self.cli_path), "--create", format_name, str(src_path), str(dst_archive)]
        if password:
            cmd.extend(["--password", password])
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        duration_ms = int((time.perf_counter() - t0) * 1000)
        return (res.returncode == 0, res.stderr if res.returncode != 0 else "", duration_ms)

    def extract_archive(self, src_archive: Path, dst_dir: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        cmd = [sys.executable, str(self.cli_path), "--extract", str(src_archive), str(dst_dir)]
        if password:
            cmd.extend(["--password", password])
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        duration_ms = int((time.perf_counter() - t0) * 1000)
        return (res.returncode == 0, res.stderr if res.returncode != 0 else "", duration_ms)


class CSdkRunner(SdkRunner):
    def __init__(self):
        super().__init__("c")
        self.bin_path = REPO_ROOT / "sdk" / "c" / "interop_cli"

    def ensure_built(self) -> bool:
        if self.bin_path.exists():
            return True
        src_c = REPO_ROOT / "sdk" / "c" / "interop_cli.c"
        if not src_c.exists():
            return False
        clang = shutil.which("clang")
        if not clang:
            return False
        cmd = [
            clang, "-std=c11",
            "-I", str(REPO_ROOT / "Sources" / "CTTZipBridge" / "include"),
            str(src_c),
            "-L", str(RUST_RELEASE),
            "-lttzip_engine",
            f"-Wl,-rpath,{RUST_RELEASE}",
            "-o", str(self.bin_path),
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        return res.returncode == 0 and self.bin_path.exists()

    def is_available(self) -> bool:
        return self.ensure_built()

    def create_archive(self, format_name: str, src_path: Path, dst_archive: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        cmd = [str(self.bin_path), "--create", format_name, str(src_path), str(dst_archive)]
        if password:
            cmd.extend(["--password", password])
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        duration_ms = int((time.perf_counter() - t0) * 1000)
        return (res.returncode == 0, res.stderr if res.returncode != 0 else "", duration_ms)

    def extract_archive(self, src_archive: Path, dst_dir: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        cmd = [str(self.bin_path), "--extract", str(src_archive), str(dst_dir)]
        if password:
            cmd.extend(["--password", password])
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        duration_ms = int((time.perf_counter() - t0) * 1000)
        return (res.returncode == 0, res.stderr if res.returncode != 0 else "", duration_ms)


class CppSdkRunner(SdkRunner):
    def __init__(self):
        super().__init__("cpp")
        self.bin_path = REPO_ROOT / "sdk" / "cpp" / "interop_cli"

    def ensure_built(self) -> bool:
        if self.bin_path.exists():
            return True
        src_cpp = REPO_ROOT / "sdk" / "cpp" / "interop_cli.cpp"
        if not src_cpp.exists():
            return False
        clangpp = shutil.which("clang++")
        if not clangpp:
            return False
        cmd = [
            clangpp, "-std=c++20",
            "-I", str(REPO_ROOT / "Sources" / "CTTZipBridge" / "include"),
            str(src_cpp),
            "-L", str(RUST_RELEASE),
            "-lttzip_engine",
            f"-Wl,-rpath,{RUST_RELEASE}",
            "-o", str(self.bin_path),
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        return res.returncode == 0 and self.bin_path.exists()

    def is_available(self) -> bool:
        return self.ensure_built()

    def create_archive(self, format_name: str, src_path: Path, dst_archive: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        cmd = [str(self.bin_path), "--create", format_name, str(src_path), str(dst_archive)]
        if password:
            cmd.extend(["--password", password])
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        duration_ms = int((time.perf_counter() - t0) * 1000)
        return (res.returncode == 0, res.stderr if res.returncode != 0 else "", duration_ms)

    def extract_archive(self, src_archive: Path, dst_dir: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        cmd = [str(self.bin_path), "--extract", str(src_archive), str(dst_dir)]
        if password:
            cmd.extend(["--password", password])
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        duration_ms = int((time.perf_counter() - t0) * 1000)
        return (res.returncode == 0, res.stderr if res.returncode != 0 else "", duration_ms)


class GoSdkRunner(SdkRunner):
    def __init__(self):
        super().__init__("go")
        self.bin_path = REPO_ROOT / "sdk" / "go" / "interop_cli"

    def ensure_built(self) -> bool:
        if self.bin_path.exists():
            return True
        src_go = REPO_ROOT / "sdk" / "go" / "interop_cli.go"
        if not src_go.exists():
            return False
        go_bin = shutil.which("go")
        if not go_bin:
            return False
        cmd = [go_bin, "build", "-o", str(self.bin_path), str(src_go)]
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT / "sdk" / "go"))
        return res.returncode == 0 and self.bin_path.exists()

    def is_available(self) -> bool:
        return self.ensure_built()

    def create_archive(self, format_name: str, src_path: Path, dst_archive: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        cmd = [str(self.bin_path), "--create", format_name, str(src_path), str(dst_archive)]
        if password:
            cmd.extend(["--password", password])
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        duration_ms = int((time.perf_counter() - t0) * 1000)
        return (res.returncode == 0, res.stderr if res.returncode != 0 else "", duration_ms)

    def extract_archive(self, src_archive: Path, dst_dir: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        cmd = [str(self.bin_path), "--extract", str(src_archive), str(dst_dir)]
        if password:
            cmd.extend(["--password", password])
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        duration_ms = int((time.perf_counter() - t0) * 1000)
        return (res.returncode == 0, res.stderr if res.returncode != 0 else "", duration_ms)


class JvmSdkRunner(SdkRunner):
    def __init__(self):
        super().__init__("jvm")
        self.bin_dir = REPO_ROOT / "sdk" / "jvm" / "bin"
        self.javac_bin = shutil.which("javac") or "/opt/homebrew/opt/openjdk@21/bin/javac"
        self.java_bin = shutil.which("java") or "/opt/homebrew/opt/openjdk@21/bin/java"
        self.dylib_path = RUST_RELEASE / "libttzip_engine.dylib"

    def ensure_built(self) -> bool:
        if not Path(self.javac_bin).exists() or not Path(self.java_bin).exists():
            return False
        src_ttzip = REPO_ROOT / "sdk" / "jvm" / "src" / "main" / "java" / "com" / "ttzip" / "TTZip.java"
        src_cli = REPO_ROOT / "sdk" / "jvm" / "src" / "test" / "java" / "com" / "ttzip" / "InteropCli.java"
        if not src_ttzip.exists() or not src_cli.exists():
            return False
        self.bin_dir.mkdir(parents=True, exist_ok=True)
        cmd = [
            self.javac_bin,
            "--enable-preview", "--source", "21",
            "-d", str(self.bin_dir),
            str(src_ttzip), str(src_cli),
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        return res.returncode == 0

    def is_available(self) -> bool:
        return self.ensure_built()

    def create_archive(self, format_name: str, src_path: Path, dst_archive: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        cmd = [
            self.java_bin,
            "--enable-preview",
            "--enable-native-access=ALL-UNNAMED",
            "-cp", str(self.bin_dir),
            f"-Dttzip.lib.path={self.dylib_path}",
            "com.ttzip.InteropCli",
            "--create", format_name, str(src_path), str(dst_archive),
        ]
        if password:
            cmd.extend(["--password", password])
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        duration_ms = int((time.perf_counter() - t0) * 1000)
        return (res.returncode == 0, res.stderr if res.returncode != 0 else "", duration_ms)

    def extract_archive(self, src_archive: Path, dst_dir: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        cmd = [
            self.java_bin,
            "--enable-preview",
            "--enable-native-access=ALL-UNNAMED",
            "-cp", str(self.bin_dir),
            f"-Dttzip.lib.path={self.dylib_path}",
            "com.ttzip.InteropCli",
            "--extract", str(src_archive), str(dst_dir),
        ]
        if password:
            cmd.extend(["--password", password])
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT))
        duration_ms = int((time.perf_counter() - t0) * 1000)
        return (res.returncode == 0, res.stderr if res.returncode != 0 else "", duration_ms)


class DartSdkRunner(SdkRunner):
    def __init__(self):
        super().__init__("dart")
        self.cli_path = REPO_ROOT / "sdk" / "dart" / "bin" / "interop_cli.dart"
        self.dart_bin = shutil.which("dart")

    def is_available(self) -> bool:
        return self.dart_bin is not None and self.cli_path.exists()

    def create_archive(self, format_name: str, src_path: Path, dst_archive: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        if not self.dart_bin:
            return (False, "Dart binary not found", 0)
        cmd = [self.dart_bin, "run", str(self.cli_path), "--create", format_name, str(src_path), str(dst_archive)]
        if password:
            cmd.extend(["--password", password])
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT / "sdk" / "dart"))
        duration_ms = int((time.perf_counter() - t0) * 1000)
        return (res.returncode == 0, res.stderr if res.returncode != 0 else "", duration_ms)

    def extract_archive(self, src_archive: Path, dst_dir: Path, password: Optional[str] = None) -> Tuple[bool, str, int]:
        if not self.dart_bin:
            return (False, "Dart binary not found", 0)
        cmd = [self.dart_bin, "run", str(self.cli_path), "--extract", str(src_archive), str(dst_dir)]
        if password:
            cmd.extend(["--password", password])
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT / "sdk" / "dart"))
        duration_ms = int((time.perf_counter() - t0) * 1000)
        return (res.returncode == 0, res.stderr if res.returncode != 0 else "", duration_ms)


def run_full_interop_matrix(output_path: Optional[Path] = None) -> Dict[str, Any]:
    """
    Executes cross-language N x N round-trip tests between all available SDKs
    and outputs a structured report conforming to interop-matrix-contract.json.
    """
    runners: List[SdkRunner] = [
        PythonSdkRunner(),
        CSdkRunner(),
        CppSdkRunner(),
        GoSdkRunner(),
        JvmSdkRunner(),
        DartSdkRunner(),
    ]

    active_runners = [r for r in runners if r.is_available()]
    formats = ["zip", "tar", "tar.gz", "tar.zst"]

    temp_dir = Path(tempfile.mkdtemp(prefix="ttzip_interop_matrix_"))
    try:
        datasets = prepare_datasets(temp_dir)
        matrix_entries: List[Dict[str, Any]] = []

        total_combinations = len(active_runners) * len(active_runners) * len(formats) * len(datasets)
        print(f"⚡️ Running TTZip Cross-Language Interop Matrix ({len(active_runners)} SDKs, {len(formats)} formats, {len(datasets)} datasets = {total_combinations} combinations)...")

        passed_count = 0
        failed_count = 0

        for creator in active_runners:
            for extractor in active_runners:
                for fmt in formats:
                    for fixture_name, (src_path, expected_sha256) in datasets.items():
                        ext = f".{fmt}" if not fmt.startswith(".") else fmt
                        archive_path = temp_dir / f"arc_{creator.name}_{fmt}_{fixture_name}{ext}"
                        extract_dst = temp_dir / f"ext_{creator.name}_to_{extractor.name}_{fmt}_{fixture_name}"
                        extract_dst.mkdir(parents=True, exist_ok=True)

                        # 1. Create Archive
                        create_ok, create_err, c_dur = creator.create_archive(fmt, src_path, archive_path)
                        if not create_ok or not archive_path.exists():
                            status = "error" if not create_ok else "failed"
                            matrix_entries.append({
                                "sourceSdk": creator.name,
                                "targetSdk": extractor.name,
                                "format": fmt,
                                "fixture": fixture_name,
                                "status": status,
                                "extractedSha256": "0" * 64,
                                "expectedSha256": expected_sha256,
                                "durationMs": c_dur,
                            })
                            failed_count += 1
                            continue

                        # 2. Extract Archive
                        extract_ok, extract_err, e_dur = extractor.extract_archive(archive_path, extract_dst)
                        total_dur = c_dur + e_dur

                        if not extract_ok:
                            matrix_entries.append({
                                "sourceSdk": creator.name,
                                "targetSdk": extractor.name,
                                "format": fmt,
                                "fixture": fixture_name,
                                "status": "error",
                                "extractedSha256": "0" * 64,
                                "expectedSha256": expected_sha256,
                                "durationMs": total_dur,
                            })
                            failed_count += 1
                            continue

                        # 3. Validate recursive SHA-256
                        # If src_path is a directory, the extracted folder contains the items inside or the top folder
                        extracted_sha256 = compute_recursive_sha256(extract_dst)

                        # Match validation
                        if extracted_sha256 == expected_sha256:
                            status = "passed"
                            passed_count += 1
                        else:
                            # Also check if top directory was unwrapped or retained
                            # e.g. extract_dst contains single subfolder named after dataset
                            subdirs = list(extract_dst.iterdir())
                            if len(subdirs) == 1 and subdirs[0].is_dir() and compute_recursive_sha256(subdirs[0]) == expected_sha256:
                                status = "passed"
                                extracted_sha256 = expected_sha256
                                passed_count += 1
                            else:
                                status = "mismatch"
                                failed_count += 1

                        entry = {
                            "sourceSdk": creator.name,
                            "targetSdk": extractor.name,
                            "format": fmt,
                            "fixture": fixture_name,
                            "status": status,
                            "extractedSha256": extracted_sha256,
                            "expectedSha256": expected_sha256,
                            "durationMs": total_dur,
                        }
                        matrix_entries.append(entry)

                        # Clean up archive & extracted files for this round-trip
                        if archive_path.exists():
                            archive_path.unlink()
                        shutil.rmtree(extract_dst, ignore_errors=True)

        report = {
            "matrix": matrix_entries,
        }

        print(f"✅ Interop Matrix Complete: {passed_count} passed, {failed_count} failed across {len(matrix_entries)} tests.")

        if output_path:
            output_path.parent.mkdir(parents=True, exist_ok=True)
            with open(output_path, "w", encoding="utf-8") as f:
                json.dump(report, f, indent=2)
            print(f"Exported interop matrix report to: {output_path}")

        return report

    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)


class TestInteropMatrix(unittest.TestCase):
    """Unit test runner for PyTest / Unittest CI discovery."""

    def test_interop_matrix_round_trips(self):
        report_path = REPO_ROOT / "reports" / "interop-matrix-report.json"
        report = run_full_interop_matrix(report_path)

        self.assertIn("matrix", report)
        self.assertGreater(len(report["matrix"]), 0)

        # Validate conformance to contract
        mismatches = [e for e in report["matrix"] if e["status"] != "passed"]
        if mismatches:
            msg = f"Interop matrix encountered {len(mismatches)} failures:\n"
            for m in mismatches[:5]:
                msg += f"  - {m['sourceSdk']} -> {m['targetSdk']} (format: {m['format']}, fixture: {m['fixture']}): status={m['status']}\n"
            self.fail(msg)


if __name__ == "__main__":
    out_file = None
    if "--output" in sys.argv:
        idx = sys.argv.index("--output")
        if idx + 1 < len(sys.argv):
            out_file = Path(sys.argv[idx + 1])
            del sys.argv[idx:idx + 2]
    elif "-o" in sys.argv:
        idx = sys.argv.index("-o")
        if idx + 1 < len(sys.argv):
            out_file = Path(sys.argv[idx + 1])
            del sys.argv[idx:idx + 2]

    if not out_file:
        out_file = REPO_ROOT / "reports" / "interop-matrix-report.json"

    report_data = run_full_interop_matrix(out_file)
    failed = [e for e in report_data["matrix"] if e["status"] != "passed"]
    sys.exit(1 if failed else 0)
