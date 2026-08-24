#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.

"""
Security Gate Report Aggregator and JSON Serializer.
Produces schema-compliant security-gate-report.json.
"""

import json
import threading
from pathlib import Path
from typing import Any, Dict, List, Optional


class SecurityReportAggregator:
    _instance = None
    _lock = threading.Lock()

    def __init__(self, report_path: Optional[Path] = None):
        if report_path is None:
            self.report_path = Path(__file__).resolve().parent / "security-gate-report.json"
        else:
            self.report_path = report_path
        self.scenarios: List[Dict[str, Any]] = []

    @classmethod
    def get_instance(cls, report_path: Optional[Path] = None) -> "SecurityReportAggregator":
        with cls._lock:
            if cls._instance is None:
                cls._instance = cls(report_path)
            return cls._instance

    def add_scenario(
        self,
        name: str,
        target_sdk: str,
        expected_outcome: str,
        actual_outcome: str,
        status: str,
        attack_payload: Optional[str] = None,
        ratio: Optional[float] = None,
        out_of_bounds_written: Optional[bool] = None,
        peak_rss_mb: Optional[float] = None,
    ) -> None:
        """Adds a scenario entry strictly matching security-fixture-contract.json."""
        entry: Dict[str, Any] = {
            "name": name,
            "targetSdk": target_sdk,
            "expectedOutcome": expected_outcome,
            "actualOutcome": actual_outcome,
            "status": status,
        }
        if attack_payload is not None:
            entry["attackPayload"] = attack_payload
        if ratio is not None:
            entry["ratio"] = round(float(ratio), 2)
        if out_of_bounds_written is not None:
            entry["outOfBoundsWritten"] = bool(out_of_bounds_written)
        if peak_rss_mb is not None:
            entry["peakRssMb"] = round(float(peak_rss_mb), 2)

        with self._lock:
            self.scenarios.append(entry)

    def write_report(self) -> Path:
        """Writes accumulated scenarios to security-gate-report.json."""
        with self._lock:
            report_data = {
                "scenarios": self.scenarios,
            }
            self.report_path.parent.mkdir(parents=True, exist_ok=True)
            with open(self.report_path, "w", encoding="utf-8") as f:
                json.dump(report_data, f, indent=2)
            return self.report_path


def get_security_aggregator() -> SecurityReportAggregator:
    return SecurityReportAggregator.get_instance()
