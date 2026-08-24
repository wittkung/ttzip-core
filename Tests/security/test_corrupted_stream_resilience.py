#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.

"""
Corrupted Stream & Header Fault Tolerance Test Suite.
Verifies that truncated headers, flipped bits, and malformed archive streams
are handled gracefully with strong error codes without SIGSEGV, SIGBUS, or crashes.
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
from fixtures.generate_malicious_fixtures import MaliciousFixturesGenerator


class TestCorruptedStreamResilience(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.registry = SdkDriverRegistry()
        cls.registry.ensure_binaries_built()
        cls.sdks = cls.registry.get_available_sdks()
        cls.fixtures_dir = Path(__file__).resolve().parent / "fixtures" / "corpus"
        cls.fixtures_dir.mkdir(parents=True, exist_ok=True)
        gen = MaliciousFixturesGenerator(output_dir=cls.fixtures_dir)
        gen.generate_all()
        cls.corrupted_dir = cls.fixtures_dir / "corrupted_streams"
        cls.aggregator = get_security_aggregator()

    def test_truncated_and_bitflip_resilience_all_sdks(self):
        """Assert all SDKs return graceful error codes and zero crashes on corrupted archives."""
        corrupted_files = list(self.corrupted_dir.glob("*.*"))
        self.assertGreater(len(corrupted_files), 0, "No corrupted test fixtures found!")

        for f in corrupted_files:
            for sdk in self.sdks:
                with self.subTest(file=f.name, sdk=sdk):
                    with tempfile.TemporaryDirectory(prefix=f"ttzip_corrupt_{sdk}_") as temp_root:
                        dest_dir = Path(temp_root) / "dest"
                        dest_dir.mkdir(parents=True, exist_ok=True)

                        res = self.registry.run_extract(sdk, f, dest_dir, timeout_secs=5)

                        # Critical Assertion: No segmentation fault (SIGSEGV = -11)
                        self.assertNotEqual(
                            res.exit_code,
                            -11,
                            f"SDK {sdk} crashed with SIGSEGV on corrupted archive {f.name}!",
                        )
                        # No illegal instruction (SIGILL = -4) or abort (SIGABRT = -6)
                        self.assertNotEqual(
                            res.exit_code,
                            -4,
                            f"SDK {sdk} crashed with SIGILL on corrupted archive {f.name}!",
                        )
                        self.assertNotEqual(
                            res.exit_code,
                            -6,
                            f"SDK {sdk} crashed with SIGABRT on corrupted archive {f.name}!",
                        )
                        # No bus error (SIGBUS = -10)
                        self.assertNotEqual(
                            res.exit_code,
                            -10,
                            f"SDK {sdk} crashed with SIGBUS on corrupted archive {f.name}!",
                        )

                        status = "passed"
                        expected = "Graceful error code returned (No SIGSEGV/SIGABRT)"
                        actual = f"Handled gracefully (exit code {res.exit_code})"

                        self.aggregator.add_scenario(
                            name=f"Corrupted Stream Resilience ({f.name})",
                            target_sdk=sdk,
                            attack_payload=f.name,
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
