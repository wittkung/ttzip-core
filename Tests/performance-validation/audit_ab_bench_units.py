# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause
"""Q4 read-only audit: detect unit mislabeling in archived A/B benchmark reports.

The retired `statistical_delta.py` parser applied its checksum regex
(`name | GB/s | MB/s | CPB`) to container rows
(`name | Pack MB/s | Extract MB/s | Ratio % | RSS MB`) as well. Container
pack throughput therefore re-appeared as a "(Throughput)" metric with unit
"GB/s", and the size ratio re-appeared as "(Cycles/Byte)".

This script never modifies the reports. It prints one line per affected
container and exits 1 when any mislabeled pair is found.

Usage:
    python3 audit_ab_bench_units.py products/ttzip/core/reports/ab_bench_*.json
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

SUFFIX_THROUGHPUT = " (Throughput)"
SUFFIX_PACK = " (Pack)"


def _index(metrics: list[dict]) -> dict[str, dict]:
    return {m["metric_name"]: m for m in metrics}


def audit_report(path: Path) -> list[str]:
    """Returns human-readable findings for one report."""
    data = json.loads(path.read_text(encoding="utf-8"))
    by_name = _index(data.get("metrics", []))
    findings: list[str] = []
    for name, metric in sorted(by_name.items()):
        if not name.endswith(SUFFIX_THROUGHPUT) or metric.get("unit") != "GB/s":
            continue
        base = name[: -len(SUFFIX_THROUGHPUT)]
        pack = by_name.get(base + SUFFIX_PACK)
        if pack is None or pack.get("unit") != "MB/s":
            continue
        same_base = metric["baseline_mean"] == pack["baseline_mean"]
        same_cand = metric["candidate_mean"] == pack["candidate_mean"]
        if same_base and same_cand:
            # The retired parser's name group also captured preceding separator lines.
            label = base.splitlines()[-1].strip()
            findings.append(
                f"{path.name}: '{label}' GB/s row == Pack MB/s row "
                f"(baseline {metric['baseline_mean']}, candidate {metric['candidate_mean']})"
            )
    return findings


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__)
        return 2
    all_findings: list[str] = []
    for arg in argv[1:]:
        all_findings.extend(audit_report(Path(arg)))
    for line in all_findings:
        print(line)
    print(f"mislabeled_pairs={len(all_findings)}")
    return 1 if all_findings else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
