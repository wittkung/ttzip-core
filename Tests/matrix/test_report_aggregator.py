#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# Structured Test Report Aggregator & Serializer for TTZip Multilingual SDK Matrix.
# Validates against specs/.../contracts/sdk-test-report-contract.json
# Generates structured JSON reports and standard JUnit XML files.

import argparse
import datetime
import json
import os
import platform
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Any, Union

CONTRACT_PATH = (
    Path(__file__).resolve().parent.parent.parent.parent
    / "specs"
    / "006-multi-language-sdk-automated-testing-framework"
    / "contracts"
    / "sdk-test-report-contract.json"
)


class TestSuiteResult:
    """Represents a single test suite execution within an SDK."""

    def __init__(
        self,
        name: str,
        status: str = "passed",
        duration_ms: int = 0,
        failure_message: Optional[str] = None,
    ):
        self.name = name
        self.status = status if status in ("passed", "failed", "skipped") else "passed"
        self.duration_ms = max(0, int(duration_ms))
        self.failure_message = failure_message

    def to_dict(self) -> Dict[str, Any]:
        return {
            "name": self.name,
            "status": self.status,
            "durationMs": self.duration_ms,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "TestSuiteResult":
        return cls(
            name=data["name"],
            status=data.get("status", "passed"),
            duration_ms=data.get("durationMs", 0),
        )


class SdkResult:
    """Represents the test results for a specific SDK ecosystem."""

    def __init__(
        self,
        language: str,
        toolchain_available: bool = True,
        status: str = "passed",
        duration_ms: int = 0,
        total_tests: int = 0,
        passed_tests: int = 0,
        failed_tests: int = 0,
        skipped_tests: int = 0,
        test_suites: Optional[List[TestSuiteResult]] = None,
    ):
        self.language = language
        self.toolchain_available = bool(toolchain_available)
        self.status = status if status in ("passed", "failed", "skipped") else "passed"
        self.duration_ms = max(0, int(duration_ms))
        self.total_tests = max(0, int(total_tests))
        self.passed_tests = max(0, int(passed_tests))
        self.failed_tests = max(0, int(failed_tests))
        self.skipped_tests = max(0, int(skipped_tests))
        self.test_suites: List[TestSuiteResult] = test_suites or []

    def add_suite(self, suite: TestSuiteResult) -> None:
        self.test_suites.append(suite)

    def recompute_from_suites(self) -> None:
        if not self.test_suites:
            return
        self.total_tests = len(self.test_suites)
        self.passed_tests = sum(1 for s in self.test_suites if s.status == "passed")
        self.failed_tests = sum(1 for s in self.test_suites if s.status == "failed")
        self.skipped_tests = sum(1 for s in self.test_suites if s.status == "skipped")
        self.duration_ms = sum(s.duration_ms for s in self.test_suites)
        if self.failed_tests > 0:
            self.status = "failed"
        elif self.passed_tests > 0:
            self.status = "passed"
        else:
            self.status = "skipped"

    def to_dict(self) -> Dict[str, Any]:
        data: Dict[str, Any] = {
            "language": self.language,
            "toolchainAvailable": self.toolchain_available,
            "status": self.status,
            "durationMs": self.duration_ms,
            "totalTests": self.total_tests,
            "passedTests": self.passed_tests,
            "failedTests": self.failed_tests,
            "skippedTests": self.skipped_tests,
        }
        if self.test_suites:
            data["testSuites"] = [s.to_dict() for s in self.test_suites]
        return data

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "SdkResult":
        suites = [TestSuiteResult.from_dict(s) for s in data.get("testSuites", [])]
        return cls(
            language=data["language"],
            toolchain_available=data.get("toolchainAvailable", True),
            status=data.get("status", "passed"),
            duration_ms=data.get("durationMs", 0),
            total_tests=data.get("totalTests", 0),
            passed_tests=data.get("passedTests", 0),
            failed_tests=data.get("failedTests", 0),
            skipped_tests=data.get("skippedTests", 0),
            test_suites=suites,
        )


class EnvironmentInfo:
    """Represents host hardware & compiler runtime versions."""

    def __init__(
        self,
        os_name: str,
        cpu_cores: int,
        rustc_version: Optional[str] = None,
        swift_version: Optional[str] = None,
        python_version: Optional[str] = None,
        go_version: Optional[str] = None,
        java_version: Optional[str] = None,
    ):
        self.os_name = os_name
        self.cpu_cores = max(1, int(cpu_cores))
        self.rustc_version = rustc_version or ""
        self.swift_version = swift_version or ""
        self.python_version = python_version or ""
        self.go_version = go_version or ""
        self.java_version = java_version or ""

    def to_dict(self) -> Dict[str, Any]:
        data: Dict[str, Any] = {
            "os": self.os_name,
            "cpuCores": self.cpu_cores,
        }
        if self.rustc_version:
            data["rustcVersion"] = self.rustc_version
        if self.swift_version:
            data["swiftVersion"] = self.swift_version
        if self.python_version:
            data["pythonVersion"] = self.python_version
        if self.go_version:
            data["goVersion"] = self.go_version
        if self.java_version:
            data["javaVersion"] = self.java_version
        return data

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "EnvironmentInfo":
        return cls(
            os_name=data.get("os", "unknown"),
            cpu_cores=data.get("cpuCores", 1),
            rustc_version=data.get("rustcVersion"),
            swift_version=data.get("swiftVersion"),
            python_version=data.get("pythonVersion"),
            go_version=data.get("goVersion"),
            java_version=data.get("javaVersion"),
        )

    @classmethod
    def auto_detect(cls) -> "EnvironmentInfo":
        """Auto detects host environment."""
        os_sys = platform.system().lower()
        cores = os.cpu_count() or 1
        py_ver = f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}"
        return cls(os_name=os_sys, cpu_cores=cores, python_version=py_ver)


class ReportSummary:
    """Overall test execution metric summary."""

    def __init__(
        self,
        total_sdks: int = 0,
        passed_sdks: int = 0,
        failed_sdks: int = 0,
        skipped_sdks: int = 0,
        total_test_cases: int = 0,
        passed_test_cases: int = 0,
        failed_test_cases: int = 0,
        skipped_test_cases: int = 0,
        total_duration_ms: int = 0,
    ):
        self.total_sdks = total_sdks
        self.passed_sdks = passed_sdks
        self.failed_sdks = failed_sdks
        self.skipped_sdks = skipped_sdks
        self.total_test_cases = total_test_cases
        self.passed_test_cases = passed_test_cases
        self.failed_test_cases = failed_test_cases
        self.skipped_test_cases = skipped_test_cases
        self.total_duration_ms = total_duration_ms

    def to_dict(self) -> Dict[str, Any]:
        return {
            "totalSdks": self.total_sdks,
            "passedSdks": self.passed_sdks,
            "failedSdks": self.failed_sdks,
            "skippedSdks": self.skipped_sdks,
            "totalTestCases": self.total_test_cases,
            "passedTestCases": self.passed_test_cases,
            "failedTestCases": self.failed_test_cases,
            "skippedTestCases": self.skipped_test_cases,
            "totalDurationMs": self.total_duration_ms,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ReportSummary":
        return cls(
            total_sdks=data.get("totalSdks", 0),
            passed_sdks=data.get("passedSdks", 0),
            failed_sdks=data.get("failedSdks", 0),
            skipped_sdks=data.get("skippedSdks", 0),
            total_test_cases=data.get("totalTestCases", 0),
            passed_test_cases=data.get("passedTestCases", 0),
            failed_test_cases=data.get("failedTestCases", 0),
            skipped_test_cases=data.get("skippedTestCases", 0),
            total_duration_ms=data.get("totalDurationMs", 0),
        )


class SdkTestReport:
    """Master Multi-Language SDK Test Report model conforming to JSON Schema."""

    def __init__(
        self,
        version: str = "1.0.0",
        timestamp: Optional[str] = None,
        environment: Optional[EnvironmentInfo] = None,
        results: Optional[List[SdkResult]] = None,
    ):
        self.version = version
        self.timestamp = timestamp or datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        self.environment = environment or EnvironmentInfo.auto_detect()
        self.results: List[SdkResult] = results or []
        self.summary = ReportSummary()
        self.recompute_summary()

    def add_result(self, result: SdkResult) -> None:
        self.results.append(result)
        self.recompute_summary()

    def recompute_summary(self) -> None:
        self.summary.total_sdks = len(self.results)
        self.summary.passed_sdks = sum(1 for r in self.results if r.status == "passed")
        self.summary.failed_sdks = sum(1 for r in self.results if r.status == "failed")
        self.summary.skipped_sdks = sum(1 for r in self.results if r.status == "skipped")
        self.summary.total_test_cases = sum(r.total_tests for r in self.results)
        self.summary.passedTestCases = sum(r.passed_tests for r in self.results)
        self.summary.passedTest_cases = self.summary.passedTestCases
        self.summary.passed_test_cases = sum(r.passed_tests for r in self.results)
        self.summary.failed_test_cases = sum(r.failed_tests for r in self.results)
        self.summary.skipped_test_cases = sum(r.skipped_tests for r in self.results)
        self.summary.total_duration_ms = sum(r.duration_ms for r in self.results)

    def to_dict(self) -> Dict[str, Any]:
        self.recompute_summary()
        return {
            "timestamp": self.timestamp,
            "version": self.version,
            "environment": self.environment.to_dict(),
            "summary": self.summary.to_dict(),
            "results": [r.to_dict() for r in self.results],
        }

    def to_json(self, path: Optional[Union[str, Path]] = None, indent: int = 2) -> str:
        data = self.to_dict()
        json_str = json.dumps(data, indent=indent, ensure_ascii=False)
        if path:
            p = Path(path)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(json_str, encoding="utf-8")
        return json_str

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "SdkTestReport":
        env = EnvironmentInfo.from_dict(data.get("environment", {}))
        results = [SdkResult.from_dict(r) for r in data.get("results", [])]
        report = cls(
            version=data.get("version", "1.0.0"),
            timestamp=data.get("timestamp"),
            environment=env,
            results=results,
        )
        report.recompute_summary()
        return report

    @classmethod
    def from_json(cls, json_str_or_path: Union[str, Path]) -> "SdkTestReport":
        if isinstance(json_str_or_path, (str, Path)) and os.path.exists(str(json_str_or_path)):
            content = Path(json_str_or_path).read_text(encoding="utf-8")
        else:
            content = str(json_str_or_path)
        data = json.loads(content)
        return cls.from_dict(data)

    def validate_against_schema(self, schema_path: Optional[Path] = None) -> Tuple[bool, List[str]]:
        """Strict validation of the report data against the JSON contract."""
        schema_file = schema_path or CONTRACT_PATH
        errors: List[str] = []

        if not schema_file.exists():
            errors.append(f"Contract schema not found: {schema_file}")
            return False, errors

        try:
            schema = json.loads(schema_file.read_text(encoding="utf-8"))
        except Exception as e:
            return False, [f"Failed to load schema file: {e}"]

        data = self.to_dict()

        # Top-level required keys
        for req in schema.get("required", []):
            if req not in data:
                errors.append(f"Missing top-level required property: '{req}'")

        if schema.get("additionalProperties") is False:
            for k in data.keys():
                if k not in schema.get("properties", {}):
                    errors.append(f"Unexpected top-level property: '{k}'")

        # Validate Environment
        env_data = data.get("environment", {})
        env_schema = schema.get("properties", {}).get("environment", {})
        for req in env_schema.get("required", []):
            if req not in env_data:
                errors.append(f"Missing required environment property: '{req}'")
        if env_schema.get("additionalProperties") is False:
            for k in env_data.keys():
                if k not in env_schema.get("properties", {}):
                    errors.append(f"Unexpected environment property: '{k}'")

        # Validate Summary
        sum_data = data.get("summary", {})
        sum_schema = schema.get("properties", {}).get("summary", {})
        for req in sum_schema.get("required", []):
            if req not in sum_data:
                errors.append(f"Missing required summary property: '{req}'")
        if sum_schema.get("additionalProperties") is False:
            for k in sum_data.keys():
                if k not in sum_schema.get("properties", {}):
                    errors.append(f"Unexpected summary property: '{k}'")

        # Validate Results
        results_data = data.get("results", [])
        if not isinstance(results_data, list):
            errors.append("'results' must be an array")
        else:
            item_schema = schema.get("properties", {}).get("results", {}).get("items", {})
            item_req = item_schema.get("required", [])
            allowed_status = item_schema.get("properties", {}).get("status", {}).get("enum", [])

            for i, r in enumerate(results_data):
                for req in item_req:
                    if req not in r:
                        errors.append(f"Result #{i} ({r.get('language')}) missing required property '{req}'")
                if allowed_status and r.get("status") not in allowed_status:
                    errors.append(f"Result #{i} has invalid status '{r.get('status')}'; allowed: {allowed_status}")
                if item_schema.get("additionalProperties") is False:
                    for k in r.keys():
                        if k not in item_schema.get("properties", {}):
                            errors.append(f"Result #{i} has unexpected property '{k}'")

        return len(errors) == 0, errors

    def to_junit_xml(self, target_path_or_dir: Union[str, Path]) -> List[Path]:
        """Generates standard JUnit XML files for CI integration."""
        target = Path(target_path_or_dir)
        written_files: List[Path] = []

        is_single_xml_file = target.suffix.lower() == ".xml"

        if not is_single_xml_file:
            target.mkdir(parents=True, exist_ok=True)
            # Write one XML file per SDK
            for res in self.results:
                root = ET.Element(
                    "testsuite",
                    name=f"com.ttzip.sdk.{res.language}",
                    tests=str(res.total_tests),
                    failures=str(res.failed_tests),
                    errors="0",
                    skipped=str(res.skipped_tests),
                    time=f"{res.duration_ms / 1000.0:.3f}",
                    timestamp=self.timestamp,
                )

                if res.test_suites:
                    for s in res.test_suites:
                        tc = ET.SubElement(
                            root,
                            "testcase",
                            classname=f"com.ttzip.sdk.{res.language}.Suite",
                            name=s.name,
                            time=f"{s.duration_ms / 1000.0:.3f}",
                        )
                        if s.status == "failed":
                            fail = ET.SubElement(tc, "failure", message=s.failure_message or "Test failed")
                            fail.text = s.failure_message or "Test failed"
                        elif s.status == "skipped":
                            ET.SubElement(tc, "skipped")
                else:
                    # Single aggregate testcase for the SDK
                    tc = ET.SubElement(
                        root,
                        "testcase",
                        classname=f"com.ttzip.sdk.{res.language}",
                        name="AllSdkTests",
                        time=f"{res.duration_ms / 1000.0:.3f}",
                    )
                    if res.status == "failed":
                        fail = ET.SubElement(tc, "failure", message=f"SDK {res.language} test suite failed")
                        fail.text = f"{res.failed_tests} out of {res.total_tests} tests failed."
                    elif res.status == "skipped":
                        ET.SubElement(tc, "skipped")

                tree = ET.ElementTree(root)
                xml_path = target / f"TEST-ttzip-{res.language}.xml"
                ET.indent(tree, space="  ", level=0)
                tree.write(xml_path, encoding="utf-8", xml_declaration=True)
                written_files.append(xml_path)
        else:
            # Single consolidated testsuites XML file
            target.parent.mkdir(parents=True, exist_ok=True)
            root = ET.Element(
                "testsuites",
                name="TTZip Multilingual SDK Test Matrix",
                tests=str(self.summary.total_test_cases),
                failures=str(self.summary.failed_test_cases),
                errors="0",
                skipped=str(self.summary.skipped_test_cases),
                time=f"{self.summary.total_duration_ms / 1000.0:.3f}",
            )
            for res in self.results:
                suite = ET.SubElement(
                    root,
                    "testsuite",
                    name=f"com.ttzip.sdk.{res.language}",
                    tests=str(res.total_tests),
                    failures=str(res.failed_tests),
                    errors="0",
                    skipped=str(res.skipped_tests),
                    time=f"{res.duration_ms / 1000.0:.3f}",
                )
                if res.test_suites:
                    for s in res.test_suites:
                        tc = ET.SubElement(
                            suite,
                            "testcase",
                            classname=f"com.ttzip.sdk.{res.language}",
                            name=s.name,
                            time=f"{s.duration_ms / 1000.0:.3f}",
                        )
                        if s.status == "failed":
                            fail = ET.SubElement(tc, "failure", message=s.failure_message or "Test failed")
                            fail.text = s.failure_message or "Test failed"
                        elif s.status == "skipped":
                            ET.SubElement(tc, "skipped")
                else:
                    tc = ET.SubElement(
                        suite,
                        "testcase",
                        classname=f"com.ttzip.sdk.{res.language}",
                        name="AllSdkTests",
                        time=f"{res.duration_ms / 1000.0:.3f}",
                    )
                    if res.status == "failed":
                        fail = ET.SubElement(tc, "failure", message=f"SDK {res.language} test suite failed")
                        fail.text = f"{res.failed_tests} out of {res.total_tests} tests failed."
                    elif res.status == "skipped":
                        ET.SubElement(tc, "skipped")

            tree = ET.ElementTree(root)
            ET.indent(tree, space="  ", level=0)
            tree.write(target, encoding="utf-8", xml_declaration=True)
            written_files.append(target)

        return written_files


def main() -> int:
    parser = argparse.ArgumentParser(
        description="TTZip Test Report Aggregator & Serializer",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "-i", "--input",
        nargs="*",
        type=Path,
        help="Input JSON report(s) to aggregate or validate",
    )
    parser.add_argument(
        "-o", "--json-out",
        type=Path,
        help="Output structured JSON report path",
    )
    parser.add_argument(
        "-j", "--junit-out",
        type=Path,
        help="Output JUnit XML directory or consolidated XML file path",
    )
    parser.add_argument(
        "--validate",
        action="store_true",
        help="Validate against sdk-test-report-contract.json",
    )
    parser.add_argument(
        "--toolchains-json",
        type=Path,
        help="Path to detect_toolchains.sh output JSON to merge environment info",
    )
    parser.add_argument(
        "--record-sdk",
        action="store_true",
        help="CLI record mode: append a single SDK result",
    )
    parser.add_argument("--sdk", type=str, help="SDK language identifier")
    parser.add_argument("--status", choices=["passed", "failed", "skipped"], default="passed")
    parser.add_argument("--toolchain-available", choices=["true", "false"], default="true")
    parser.add_argument("--duration-ms", type=int, default=0)
    parser.add_argument("--total", type=int, default=0)
    parser.add_argument("--passed", type=int, default=0)
    parser.add_argument("--failed", type=int, default=0)
    parser.add_argument("--skipped", type=int, default=0)

    args = parser.parse_args()

    # Load initial or combined report
    master_report = SdkTestReport()

    if args.toolchains_json and args.toolchains_json.exists():
        try:
            t_data = json.loads(args.toolchains_json.read_text(encoding="utf-8"))
            env_dict = t_data.get("environment", {})
            master_report.environment = EnvironmentInfo.from_dict(env_dict)
        except Exception as e:
            print(f"[Warning] Failed to parse toolchains JSON: {e}", file=sys.stderr)

    if args.input:
        for inp in args.input:
            if not inp.exists():
                print(f"[Warning] Input file {inp} not found, skipping.", file=sys.stderr)
                continue
            rep = SdkTestReport.from_json(inp)
            for res in rep.results:
                master_report.add_result(res)

    if args.record_sdk and args.sdk:
        rec = SdkResult(
            language=args.sdk,
            toolchain_available=(args.toolchain_available == "true"),
            status=args.status,
            duration_ms=args.duration_ms,
            total_tests=args.total,
            passed_tests=args.passed,
            failed_tests=args.failed,
            skipped_tests=args.skipped,
        )
        master_report.add_result(rec)

    master_report.recompute_summary()

    if args.validate:
        valid, errors = master_report.validate_against_schema()
        if not valid:
            print(f"❌ Contract Validation Failed with {len(errors)} error(s):", file=sys.stderr)
            for err in errors:
                print(f"  - {err}", file=sys.stderr)
            return 1
        else:
            print("✅ Contract Validation Passed: Report conforms strictly to SdkTestReportContract.")

    if args.json_out:
        master_report.to_json(args.json_out)
        print(f"📄 Exported JSON report to: {args.json_out}")

    if args.junit_out:
        files = master_report.to_junit_xml(args.junit_out)
        print(f"📊 Exported {len(files)} JUnit XML report(s) to: {args.junit_out}")

    if not args.json_out and not args.junit_out and not args.validate:
        print(master_report.to_json())

    return 0 if master_report.summary.failed_sdks == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
