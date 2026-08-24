#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

"""
Cross-SDK Execution Drivers for Security & Resilience Verification.
Supports invoking Rust (via C-ABI/Python), C11, C++20, Go, Python, and Java SDKs.
"""

import os
import resource
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Tuple


@dataclass
class SdkExecutionResult:
    sdk_name: str
    exit_code: int
    stdout: str
    stderr: str
    peak_rss_mb: float
    success: bool
    exception_name: Optional[str] = None


class SdkDriverRegistry:
    """Manages compilation, caching, and execution of all language SDK harnesses."""

    def __init__(self, repo_root: Optional[Path] = None):
        if repo_root is None:
            # SdkDriverRegistry is located at core/tests/security/
            self.repo_root = Path(__file__).resolve().parent.parent.parent
        else:
            self.repo_root = repo_root

        self.c_cli = self.repo_root / "sdk" / "c" / "interop_cli"
        self.cpp_cli = self.repo_root / "sdk" / "cpp" / "interop_cli"
        self.go_cli = self.repo_root / "sdk" / "go" / "interop_cli"
        self.jvm_bin = self.repo_root / "sdk" / "jvm" / "bin"
        self.python_cli = self.repo_root / "python" / "interop_cli.py"
        self.rust_lib = self.repo_root / "rust" / "target" / "release" / "libttzip_engine.dylib"
        if not self.rust_lib.exists():
            self.rust_lib = self.repo_root / "rust" / "target" / "release" / "libttzip_engine.a"

    def ensure_binaries_built(self) -> None:
        """Ensures all SDK headless test binaries are compiled and ready."""
        # C CLI
        if not self.c_cli.exists() and (self.repo_root / "sdk" / "c" / "interop_cli.c").exists():
            cmd = [
                "clang",
                "-std=c11",
                "-I",
                str(self.repo_root / "Sources" / "CTTZipBridge" / "include"),
                str(self.repo_root / "sdk" / "c" / "interop_cli.c"),
                str(self.repo_root / "rust" / "target" / "release" / "libttzip_engine.a"),
                "-larchive",
                "-lbz2",
                "-lz",
                "-llzma",
                "-framework",
                "Security",
                "-o",
                str(self.c_cli),
            ]
            subprocess.run(cmd, check=False, capture_output=True)

        # C++ CLI
        if not self.cpp_cli.exists() and (self.repo_root / "sdk" / "cpp" / "interop_cli.cpp").exists():
            cmd = [
                "clang++",
                "-std=c++20",
                "-I",
                str(self.repo_root / "Sources" / "CTTZipBridge" / "include"),
                str(self.repo_root / "sdk" / "cpp" / "interop_cli.cpp"),
                str(self.repo_root / "rust" / "target" / "release" / "libttzip_engine.a"),
                "-larchive",
                "-lbz2",
                "-lz",
                "-llzma",
                "-framework",
                "Security",
                "-o",
                str(self.cpp_cli),
            ]
            subprocess.run(cmd, check=False, capture_output=True)

        # Go CLI
        if not self.go_cli.exists() and (self.repo_root / "sdk" / "go" / "interop_cli.go").exists():
            cmd = ["go", "build", "-o", str(self.go_cli), str(self.repo_root / "sdk" / "go" / "interop_cli.go")]
            subprocess.run(cmd, cwd=str(self.repo_root / "sdk" / "go"), check=False, capture_output=True)

        # Java Class
        if not (self.jvm_bin / "com" / "ttzip" / "InteropCli.class").exists():
            self.jvm_bin.mkdir(parents=True, exist_ok=True)
            cmd = [
                "javac",
                "--enable-preview",
                "--release",
                "21",
                "-d",
                str(self.jvm_bin),
                str(self.repo_root / "sdk" / "jvm" / "src" / "main" / "java" / "com" / "ttzip" / "TTZip.java"),
                str(self.repo_root / "sdk" / "jvm" / "src" / "test" / "java" / "com" / "ttzip" / "InteropCli.java"),
            ]
            subprocess.run(cmd, check=False, capture_output=True)

    def get_available_sdks(self) -> List[str]:
        """Returns list of all available SDK keys."""
        sdks = ["python"]
        if self.c_cli.exists():
            sdks.append("c")
        if self.cpp_cli.exists():
            sdks.append("cpp")
        if self.go_cli.exists():
            sdks.append("go")
        if (self.jvm_bin / "com" / "ttzip" / "InteropCli.class").exists():
            sdks.append("java")
        return sdks

    def run_extract(
        self,
        sdk: str,
        archive_path: Path,
        destination_path: Path,
        password: Optional[str] = None,
        timeout_secs: int = 10,
    ) -> SdkExecutionResult:
        """Executes extraction in target SDK and captures status, output, and RSS."""
        self.ensure_binaries_built()
        archive_str = str(archive_path.resolve())
        dest_str = str(destination_path.resolve())

        cmd: List[str] = []
        env = dict(os.environ)

        if sdk == "python":
            cmd = [sys.executable, str(self.python_cli), "--extract", archive_str, dest_str]
            if password:
                cmd.extend(["--password", password])
            env["PYTHONPATH"] = str(self.repo_root / "python")

        elif sdk == "c":
            if not self.c_cli.exists():
                return SdkExecutionResult(sdk, -1, "", "C binary not found", 0.0, False)
            cmd = [str(self.c_cli), "--extract", archive_str, dest_str]
            if password:
                cmd.extend(["--password", password])

        elif sdk == "cpp":
            if not self.cpp_cli.exists():
                return SdkExecutionResult(sdk, -1, "", "C++ binary not found", 0.0, False)
            cmd = [str(self.cpp_cli), "--extract", archive_str, dest_str]
            if password:
                cmd.extend(["--password", password])

        elif sdk == "go":
            if not self.go_cli.exists():
                return SdkExecutionResult(sdk, -1, "", "Go binary not found", 0.0, False)
            cmd = [str(self.go_cli), "--extract", archive_str, dest_str]
            if password:
                cmd.extend(["--password", password])

        elif sdk == "java":
            java_dylib = self.repo_root / "rust" / "target" / "release" / "libttzip_engine.dylib"
            cmd = [
                "java",
                "--enable-preview",
                f"-Dttzip.lib.path={java_dylib}",
                "-cp",
                str(self.jvm_bin),
                "com.ttzip.InteropCli",
                "--extract",
                archive_str,
                dest_str,
            ]
            if password:
                cmd.extend(["--password", password])
        else:
            return SdkExecutionResult(sdk, -1, "", f"Unknown SDK: {sdk}", 0.0, False)

        time_bin = "/usr/bin/time"
        use_time_l = Path(time_bin).exists()
        exec_cmd = [time_bin, "-l"] + cmd if use_time_l else cmd

        try:
            proc = subprocess.Popen(
                exec_cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                env=env,
            )
            stdout, raw_stderr = proc.communicate(timeout=timeout_secs)
            exit_code = proc.returncode

            peak_rss_mb = 0.0
            clean_stderr_lines = []

            for line in raw_stderr.splitlines():
                if "maximum resident set size" in line:
                    parts = line.strip().split()
                    if parts and parts[0].isdigit():
                        peak_rss_mb = round(int(parts[0]) / (1024.0 * 1024.0), 2)
                elif any(metric in line for metric in ("real", "user", "sys", "page reclaims", "page faults", "voluntary context switches")):
                    continue
                else:
                    clean_stderr_lines.append(line)

            if peak_rss_mb == 0.0:
                rusage = resource.getrusage(resource.RUSAGE_CHILDREN)
                peak_rss_mb = round(rusage.ru_maxrss / (1024.0 * 1024.0), 2)

            stderr = "\n".join(clean_stderr_lines)

            return SdkExecutionResult(
                sdk_name=sdk,
                exit_code=exit_code,
                stdout=stdout,
                stderr=stderr,
                peak_rss_mb=peak_rss_mb,
                success=(exit_code == 0),
            )
        except subprocess.TimeoutExpired:
            proc.kill()
            return SdkExecutionResult(sdk, -9, "", "Timeout expired", 64.0, False, "TimeoutExpired")
        except Exception as ex:
            return SdkExecutionResult(sdk, -1, "", str(ex), 0.0, False, type(ex).__name__)
