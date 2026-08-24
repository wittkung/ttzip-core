# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# 50-Point Matrix Benchmark and Pareto Optimization Test Suite for TTZip Python SDK.

import unittest
import ttzip


class TestTTZip50PointMatrixBench(unittest.TestCase):
    def test_matrix_benchmark_execution_and_pareto_optimality(self):
        # Run 50-point matrix evaluation on 64KB synthetic JSON corpus
        report = ttzip.benchmark_matrix(corpus_type="synthetic_json", corpus_size=65536, iterations=1)

        print("\n" + "=" * 115)
        print("⚡️ [Python SDK] 50-Point Multi-Codec Matrix Benchmark Report")
        print("=" * 115)
        print(f"{'Idx':>3} | {'Engine':<10} | {'Lvl':<3} | {'Original':>8} | {'Compressed':>10} | {'Savings':>8} | {'Comp Speed':>12} | {'Decomp Speed':>12} | {'Pareto'}")
        print("-" * 115)

        for idx, pt in enumerate(report.points):
            pareto_str = "⭐ Rank 1 (Optimal)" if pt.is_pareto_optimal else "   Dominated"
            print(
                f"{idx+1:>3} | {pt.algorithm:<10} | L{pt.level:<2} | {pt.original_size_bytes:>8} B | "
                f"{pt.compressed_size_bytes:>10} B | {pt.space_savings_pct:>7.1f}% | "
                f"{pt.compress_throughput_mbs:>9.1f} MB/s | {pt.decompress_throughput_mbs:>9.1f} MB/s | {pareto_str}"
            )

        print("-" * 115)
        print(
            f"Summary: {report.total_points_evaluated} Points Evaluated | "
            f"{report.pareto_optimal_count} Pareto Optimal Points | "
            f"Peak Comp: {report.peak_compress_throughput_mbs:.1f} MB/s | "
            f"Peak Decomp: {report.peak_decompress_throughput_mbs:.1f} MB/s | "
            f"Max Savings: {report.max_space_savings_pct:.1f}% | "
            f"Gate: {'✅ PASS' if report.passed_gate else '❌ FAIL'}"
        )
        print("=" * 115)

        # Assertions
        self.assertGreaterEqual(report.total_points_evaluated, 30)
        self.assertGreater(report.pareto_optimal_count, 0)
        self.assertGreater(report.peak_compress_throughput_mbs, 100.0)
        self.assertGreater(report.peak_decompress_throughput_mbs, 100.0)
        self.assertTrue(report.passed_gate)


if __name__ == "__main__":
    unittest.main()
