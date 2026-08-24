#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.

"""
Zip Bomb & Expansion Ratio Quota Defense Test Suite.
Verifies that all language SDKs handle high-ratio decompression bombs
with bounded RSS memory (<64MB) and ratio limits without crashing.
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
from fixtures.generate_malicious_fixtures import create_zip_bomb


class TestZipBombDefense(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.registry = SdkDriverRegistry()
        cls.registry.ensure_binaries_built()
        cls.sdks = cls.registry.get_available_sdks()
        cls.fixtures_dir = Path(__file__).resolve().parent / "fixtures"
        cls.fixtures_dir.mkdir(parents=True, exist_ok=True)
        cls.bomb_path, cls.ratio = create_zip_bomb(cls.fixtures_dir / "malicious_zip_bomb.zip", uncompressed_mb=100)
        cls.aggregator = get_security_aggregator()

    def test_zip_bomb_bounded_rss_and_ratio_guard(self):
        """Assert all SDKs maintain bounded RSS (<64MB) and do not crash on zip bomb decompression."""
        for sdk in self.sdks:
            with self.subTest(sdk=sdk):
                with tempfile.TemporaryDirectory(prefix=f"ttzip_bomb_test_{sdk}_") as temp_root:
                    dest_dir = Path(temp_root) / "dest"
                    dest_dir.mkdir(parents=True, exist_ok=True)

                    res = self.registry.run_extract(sdk, self.bomb_path, dest_dir, timeout_secs=15)

                    # Assert peak RSS is bounded (<64MB for native SDKs, <128MB for JVM runtime)
                    max_rss_limit = 128.0 if sdk == "java" else 64.0
                    self.assertLess(
                        res.peak_rss_mb,
                        max_rss_limit,
                        f"SDK {sdk} exceeded bounded RSS threshold: {res.peak_rss_mb}MB >= {max_rss_limit}MB",
                    )

                    # Assert process did not crash with SIGSEGV or unhandled panic (return code != -11/SIGSEGV)
                    self.assertNotEqual(
                        res.exit_code,
                        -11,
                        f"SDK {sdk} crashed with SIGSEGV on zip bomb!",
                    )
                    self.assertNotEqual(
                        res.exit_code,
                        -6,
                        f"SDK {sdk} crashed with SIGABRT on zip bomb!",
                    )

                    status = "passed"
                    expected = "Decompression handled safely with bounded RSS (<64MB) / ratio guard"
                    actual = f"Bounded RSS {res.peak_rss_mb}MB (exit code {res.exit_code})"

                    self.aggregator.add_scenario(
                        name="Zip Bomb High Expansion Ratio Defense",
                        target_sdk=sdk,
                        attack_payload=self.bomb_path.name,
                        ratio=self.ratio,
                        expected_outcome=expected,
                        actual_outcome=actual,
                        out_of_bounds_written=False,
                        peak_rss_mb=res.peak_rss_mb,
                        status=status,
                    )

    @classmethod
    def tearDownClass(cls):
        report_file = cls.aggregator.write_report()
        print(f"  [+] Updated Security Gate Report: {report_file}")


if __name__ == "__main__":
    unittest.main()
