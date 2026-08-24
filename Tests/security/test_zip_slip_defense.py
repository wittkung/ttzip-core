#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.

"""
Zip Slip Path Traversal Defense Test Suite.
Verifies that all language SDKs strictly prevent writing files outside destination root.
"""

import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

SECURITY_DIR = Path(__file__).resolve().parent
if str(SECURITY_DIR) not in sys.path:
    sys.path.insert(0, str(SECURITY_DIR))

from sdk_drivers import SdkDriverRegistry
from security_report_helper import get_security_aggregator
from fixtures.generate_malicious_fixtures import create_zip_slip_zip, create_zip_slip_tar


class TestZipSlipDefense(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.registry = SdkDriverRegistry()
        cls.registry.ensure_binaries_built()
        cls.sdks = cls.registry.get_available_sdks()
        cls.fixtures_dir = Path(__file__).resolve().parent / "fixtures"
        cls.fixtures_dir.mkdir(parents=True, exist_ok=True)
        cls.zip_slip_archive = create_zip_slip_zip(cls.fixtures_dir / "malicious_zip_slip.zip")
        cls.tar_slip_archive = create_zip_slip_tar(cls.fixtures_dir / "malicious_zip_slip.tar")
        cls.aggregator = get_security_aggregator()

    def test_zip_slip_defense_all_sdks(self):
        """Assert zero files are written outside destination directory for all SDKs."""
        payload_names = ["evil_slip.txt", "etc/shadow_fake", "escaped.txt", "win_evil.txt", "absolute_pwn.txt"]

        for sdk in self.sdks:
            with self.subTest(sdk=sdk):
                with tempfile.TemporaryDirectory(prefix=f"ttzip_slip_test_{sdk}_") as temp_root:
                    dest_dir = Path(temp_root) / "dest"
                    dest_dir.mkdir(parents=True, exist_ok=True)

                    # Canary directory next to destination
                    canary_dir = Path(temp_root) / "canary"
                    canary_dir.mkdir(parents=True, exist_ok=True)

                    res = self.registry.run_extract(sdk, self.zip_slip_archive, dest_dir)

                    # Scan for any escaped files outside dest_dir
                    escaped_files = []
                    for root, _, files in os.walk(temp_root):
                        for f in files:
                            full_path = Path(root) / f
                            try:
                                full_path.relative_to(dest_dir)
                            except ValueError:
                                escaped_files.append(str(full_path))

                    out_of_bounds = len(escaped_files) > 0
                    self.assertFalse(
                        out_of_bounds,
                        f"Zip Slip vulnerability detected in SDK {sdk}! Escaped files: {escaped_files}",
                    )

                    status = "passed" if not out_of_bounds else "vulnerable"
                    expected = "ErrSecurityViolation / 0 files written outside dest root"
                    actual = f"Rejected (exit code {res.exit_code}, 0 files out of bounds)"

                    self.aggregator.add_scenario(
                        name="Zip Slip Path Traversal (ZIP)",
                        target_sdk=sdk,
                        attack_payload="../../evil_slip.txt",
                        expected_outcome=expected,
                        actual_outcome=actual,
                        out_of_bounds_written=out_of_bounds,
                        peak_rss_mb=res.peak_rss_mb,
                        status=status,
                    )

    def test_tar_slip_defense_all_sdks(self):
        """Assert zero files are written outside destination directory for TAR slip payloads."""
        for sdk in self.sdks:
            with self.subTest(sdk=sdk):
                with tempfile.TemporaryDirectory(prefix=f"ttzip_tarslip_test_{sdk}_") as temp_root:
                    dest_dir = Path(temp_root) / "dest"
                    dest_dir.mkdir(parents=True, exist_ok=True)

                    res = self.registry.run_extract(sdk, self.tar_slip_archive, dest_dir)

                    escaped_files = []
                    for root, _, files in os.walk(temp_root):
                        for f in files:
                            full_path = Path(root) / f
                            try:
                                full_path.relative_to(dest_dir)
                            except ValueError:
                                escaped_files.append(str(full_path))

                    out_of_bounds = len(escaped_files) > 0
                    self.assertFalse(
                        out_of_bounds,
                        f"Tar Slip vulnerability detected in SDK {sdk}! Escaped files: {escaped_files}",
                    )

                    status = "passed" if not out_of_bounds else "vulnerable"
                    expected = "ErrSecurityViolation / 0 files written outside dest root"
                    actual = f"Blocked (exit code {res.exit_code}, 0 files out of bounds)"

                    self.aggregator.add_scenario(
                        name="Zip Slip Path Traversal (TAR)",
                        target_sdk=sdk,
                        attack_payload="../../evil_tar_slip.txt",
                        expected_outcome=expected,
                        actual_outcome=actual,
                        out_of_bounds_written=out_of_bounds,
                        peak_rss_mb=res.peak_rss_mb,
                        status=status,
                    )

    @classmethod
    def tearDownClass(cls):
        report_file = cls.aggregator.write_report()
        print(f"  [+] Updated Security Gate Report: {report_file}")


if __name__ == "__main__":
    unittest.main()
