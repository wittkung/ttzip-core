#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

"""
upstream_report_gen.py - Standardized Markdown & JSON Report Generator for Upstream PRs
Generates canonical 13-micro and 50-macro sorted tables.
"""

import sys
import os
import json
import argparse

def generate_report(json_path, output_md_path):
    with open(json_path) as f:
        data = json.load(f)
        
    dev_M = data.get('dev_macro', {})
    cand_M = data.get('cand_macro', {})
    dev_u = data.get('dev_micro', {})
    cand_u = data.get('cand_micro', {})
    
    # 1. Micro table
    micro_lengths = [1, 10, 16, 24, 32, 40, 48, 56, 64, 80, 100, 175, 256]
    micro_lines = [
        '| len | base | fixed | fixed Δ |',
        '|----:|-----:|------:|--------:|'
    ]
    for l in micro_lengths:
        k = f'compare256/native/{l}'
        t0 = dev_u.get(k, 0.0)
        t1 = cand_u.get(k, 0.0)
        d = (t1 - t0) / t0 * 100 if t0 > 0 else 0.0
        micro_lines.append(f'| {l:<3} | {t0:.2f} |  {t1:.2f} | {d:+.1f}%   |')
    micro_table = "\n".join(micro_lines)
    
    # 2. Canonical macro table
    workloads = ['text', 'striped_rgb', 'dna', 'mixed', 'short_match', 'random', 'literals', 'realistic_rgb']
    sizes = [131072, 1048576]
    levels_map = {
        'text': [1, 3, 6, 9],
        'striped_rgb': [3, 6, 9],
        'dna': [3, 6, 9],
        'mixed': [3, 6, 9],
        'short_match': [3, 6, 9],
        'random': [3, 6, 9],
        'literals': [3, 6, 9],
        'realistic_rgb': [3, 6, 9]
    }
    
    macro_lines = [
        '| benchmark | base | fixed | fixed Δ |',
        '|---|---:|---:|---:|'
    ]
    for w in workloads:
        for s in sizes:
            for lvl in levels_map[w]:
                k = f'deflate_bench/level/{w}/{s}/{lvl}'
                if k in dev_M:
                    t0_ns = dev_M[k]
                    t1_ns = cand_M[k]
                    if t0_ns < 1e6:
                        t0_str = f'{t0_ns/1e3:.1f} µs'
                        t1_str = f'{t1_ns/1e3:.1f} µs'
                    else:
                        t0_str = f'{t0_ns/1e6:.2f} ms'
                        t1_str = f'{t1_ns/1e6:.2f} ms'
                    d = (t1_ns - t0_ns) / t0_ns * 100
                    d_str = f'{d:+.1f}%'
                    if abs(d) < 0.05:
                        d_str = '0.0%'
                    if d <= -10.0:
                        d_str = f'**{d_str}**'
                        t1_str = f'**{t1_str}**'
                    b_name = f'`deflate_bench` {w}/{s}/{lvl}'
                    macro_lines.append(f'| {b_name} | {t0_str} | {t1_str} | {d_str} |')
    macro_table = "\n".join(macro_lines)
    
    report_md = "# Upstream Benchmark Verification Report\n\n## Microbenchmark (compare256/native across 13 lengths)\n\n" + micro_table + "\n\n## Macrobenchmark (deflate_bench across 50 test points)\n\n" + macro_table + "\n"
    
    if output_md_path:
        with open(output_md_path, 'w') as f:
            f.write(report_md)
        print(f'Report written to {output_md_path}')
    else:
        print(report_md)

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    default_json = os.environ.get(
        'UPSTREAM_BENCH_RESULTS_JSON',
        os.path.join(os.path.dirname(__file__), 'full_deflate_matrix_results.json')
    )
    parser.add_argument('--input-json', default=default_json)
    parser.add_argument('--output-md', default=None)
    args = parser.parse_args()
    generate_report(args.input_json, args.output_md)
