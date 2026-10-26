#!/usr/bin/env python3
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

"""Independent Q4 verification runner (Rev 2) for TTZip performance & benchmark audit.

Executes Entry A (standalone `libttzip_engine.dylib`) and Entry B (Release Candidate
`ttarchive-worker`) benchmarks with per-round post-timing relative-path + SHA-256
verification and `RSE <= 2.0%` gated comparisons.
"""

from __future__ import annotations

import ctypes
import json
import os
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Tuple

from telemetry_models import (
    CLI_PATH,
    DYLIB_PATH,
    FMT_TAR_GZ,
    FMT_TAR_ZSTD,
    FMT_ZIP,
    LVL_FAST,
    LVL_FASTEST,
    LVL_NORMAL,
    OUT_JSON,
    RC_ACCEPT_MANIFEST,
    RC_DIRECT_MANIFEST,
    RC_DIRECT_WORKER,
    REPO_ROOT,
    RSE_GATE_THRESHOLD_PCT,
    TTZipCreateOptions,
    TTZipEntryMetadata,
    TTZipExtractOptions,
    TTZipVfsMatchDto,
    compute_stats,
    generate_corpora,
    load_engine,
    sha256_file,
    tree_manifest_sha256,
    welch_t_test,
)


def run_in_memory_microbenchmarks(lib: ctypes.CDLL, corpora: Dict[str, Any]) -> Dict[str, Any]:
    """Measures standalone dylib in-memory CRC32, Deflate (real vs synthetic vs zero), and VFS search."""
    results: Dict[str, Any] = {
        "entry_point": "Entry_A_Standalone_Engine_Dylib",
        "crc32": [],
        "deflate": [],
        "vfs_search": [],
    }

    s2_file = Path(corpora["S2_high_entropy"]["path"]) / "random_media_payload.bin"
    raw_32m = s2_file.read_bytes()
    raw_64m = raw_32m + raw_32m

    for label, buf_size, loops in [
        ("L1_64KiB", 64 * 1024, 2000),
        ("L2_512KiB", 512 * 1024, 500),
        ("SLC_4MiB", 4 * 1024 * 1024, 128),
        ("DRAM_64MiB", 64 * 1024 * 1024, 16),
    ]:
        buf = ctypes.create_string_buffer(raw_64m[:buf_size], buf_size)
        crc_val = 0
        for _ in range(10):
            crc_val = lib.ttzip_rust_crc32(crc_val, buf, buf_size)

        mib_s_samples, gb_s_samples, raw_ns = [], [], []
        for _ in range(30):
            t0 = time.perf_counter_ns()
            c = 0
            for _ in range(loops):
                c = lib.ttzip_rust_crc32(c, buf, buf_size)
            t1 = time.perf_counter_ns()
            if c == 0xDEAD0000:
                raise RuntimeError("unreachable")
            dt_ns = t1 - t0
            raw_ns.append(dt_ns)
            dt_s = dt_ns / 1e9
            scanned = buf_size * loops
            mib_s_samples.append((scanned / 1048576.0) / dt_s)
            gb_s_samples.append((scanned / 1e9) / dt_s)

        results["crc32"].append(
            {
                "regime": label,
                "buffer_bytes": buf_size,
                "loops_per_sample": loops,
                "raw_elapsed_ns": raw_ns,
                "mib_per_s": compute_stats(mib_s_samples),
                "gb_per_s_decimal": compute_stats(gb_s_samples),
                "crc32_hex": f"0x{crc_val:08x}",
            }
        )

    r1_root = Path(corpora["R1_repo_real_corpus"]["path"])
    r1_chunks: List[bytes] = []
    r1_len = 0
    for dp, _, fns in sorted(os.walk(r1_root)):
        for fn in sorted(fns):
            p = Path(dp) / fn
            if p.is_file():
                b = p.read_bytes()
                r1_chunks.append(b)
                r1_len += len(b)
    real_4m = (b"".join(r1_chunks) * ((4 * 1024 * 1024 // max(r1_len, 1)) + 2))[: 4 * 1024 * 1024]
    s1_4m = (Path(corpora["S1_synthetic_repeatable_text"]["path"]) / "structured_telemetry.log").read_bytes()[
        : 4 * 1024 * 1024
    ]
    zero_4m = b"\x00" * (4 * 1024 * 1024)

    for sample_name, corpus_kind, payload in [
        ("R1_Real_Repo_Code_Docs_4MiB", "authorized_real_world", real_4m),
        ("S1_Synthetic_Repeatable_Log_4MiB", "synthetic_high_repetition", s1_4m),
        ("S0_All_Zero_RLE_4MiB", "synthetic_zero_degenerate", zero_4m),
    ]:
        src_len = len(payload)
        src_buf = ctypes.create_string_buffer(payload, src_len)
        for level in [1, 6, 9]:
            bound = lib.ttzip_rust_deflate_compress_bound(src_len, level)
            comp_buf = ctypes.create_string_buffer(bound)
            decomp_buf = ctypes.create_string_buffer(src_len)
            out_comp_len = ctypes.c_size_t(0)
            out_decomp_len = ctypes.c_size_t(0)

            rc = lib.ttzip_rust_deflate_compress(
                src_buf, src_len, comp_buf, bound, level, ctypes.byref(out_comp_len)
            )
            assert rc == 0
            comp_len = out_comp_len.value

            comp_mibs, comp_gbs, decomp_mibs, decomp_gbs = [], [], [], []
            raw_comp_ns, raw_decomp_ns = [], []
            for _ in range(25):
                t0 = time.perf_counter_ns()
                rc_c = lib.ttzip_rust_deflate_compress(
                    src_buf, src_len, comp_buf, bound, level, ctypes.byref(out_comp_len)
                )
                t1 = time.perf_counter_ns()
                assert rc_c == 0

                t2 = time.perf_counter_ns()
                rc_d = lib.ttzip_rust_deflate_decompress(
                    comp_buf, comp_len, decomp_buf, src_len, ctypes.byref(out_decomp_len)
                )
                t3 = time.perf_counter_ns()
                assert rc_d == 0 and out_decomp_len.value == src_len
                assert decomp_buf.raw[:src_len] == payload

                raw_comp_ns.append(t1 - t0)
                raw_decomp_ns.append(t3 - t2)
                comp_mibs.append((src_len / 1048576.0) / ((t1 - t0) / 1e9))
                comp_gbs.append((src_len / 1e9) / ((t1 - t0) / 1e9))
                decomp_mibs.append((src_len / 1048576.0) / ((t3 - t2) / 1e9))
                decomp_gbs.append((src_len / 1e9) / ((t3 - t2) / 1e9))

            results["deflate"].append(
                {
                    "sample": sample_name,
                    "corpus_type": corpus_kind,
                    "level": level,
                    "input_bytes": src_len,
                    "compressed_bytes": comp_len,
                    "ratio_pct": round(comp_len / src_len * 100.0, 3),
                    "raw_compress_ns": raw_comp_ns,
                    "raw_decompress_ns": raw_decomp_ns,
                    "compress_mib_s": compute_stats(comp_mibs),
                    "compress_gb_s_decimal": compute_stats(comp_gbs),
                    "decompress_mib_s": compute_stats(decomp_mibs),
                    "decompress_gb_s_decimal": compute_stats(decomp_gbs),
                    "per_round_byte_verified": True,
                }
            )

    for node_count in [5_000, 100_000, 1_000_000]:
        EntryArray = TTZipEntryMetadata * node_count
        entries = EntryArray()
        path_holder = []
        meta_size = ctypes.sizeof(TTZipEntryMetadata)
        for i in range(node_count):
            p_bytes = f"root/dir_{i // 1000}/sub_{(i % 1000) // 50}/file_{i}.dat".encode("utf-8")
            path_holder.append(p_bytes)
            e = entries[i]
            e.struct_size = meta_size
            e.abi_version = 3
            e.path = ctypes.cast(ctypes.c_char_p(p_bytes), ctypes.c_char_p)
            e.uncompressed_size = 1024
            e.compressed_size = 512
            e.crc32 = 0x12345678
            e.mtime_epoch_secs = 1700000000
            e.mode = 0o644
            e.is_directory = False
            e.is_encrypted = False
            e.compression_method = 8
            e.detected_encoding = None

        t_build0 = time.perf_counter_ns()
        handle = lib.ttzip_rust_vfs_tree_build(entries, node_count, b"ProjectRoot")
        t_build1 = time.perf_counter_ns()
        build_ms = (t_build1 - t_build0) / 1e6

        MatchArray = TTZipVfsMatchDto * 64
        matches = MatchArray()
        for q_label, q_str in [
            ("best_case_first", b"file_0.dat"),
            ("worst_case_last", f"file_{node_count - 1}.dat".encode("utf-8")),
            ("non_existent", b"non_existent_xyz_999.dat"),
        ]:
            cnt = lib.ttzip_rust_vfs_search_zero_alloc(handle, q_str, matches, 64)
            lat_ms, raw_ns = [], []
            runs = 30 if node_count <= 100_000 else 15
            for _ in range(runs):
                t0 = time.perf_counter_ns()
                cnt = lib.ttzip_rust_vfs_search_zero_alloc(handle, q_str, matches, 64)
                t1 = time.perf_counter_ns()
                raw_ns.append(t1 - t0)
                lat_ms.append((t1 - t0) / 1e6)
            results["vfs_search"].append(
                {
                    "node_count": node_count,
                    "tree_build_ms": round(build_ms, 2),
                    "query_type": q_label,
                    "query": q_str.decode("utf-8"),
                    "matched_count": int(cnt),
                    "raw_elapsed_ns": raw_ns,
                    "latency_ms": compute_stats(lat_ms),
                }
            )
        lib.ttzip_rust_vfs_tree_free(handle)

    return results


def _make_create_opts(fmt: int, level: int, threads: int) -> TTZipCreateOptions:
    opts = TTZipCreateOptions()
    opts.struct_size = ctypes.sizeof(TTZipCreateOptions)
    opts.abi_version = 3
    opts.format = fmt
    opts.level = level
    opts.encryption = 0
    opts.password = None
    opts.thread_budget = threads
    opts.solid_block_size_mb = 0
    opts.progress_callback = None
    opts.user_data = None
    return opts


def _make_extract_opts(dst_bytes: bytes, threads: int = 0) -> TTZipExtractOptions:
    opts = TTZipExtractOptions()
    opts.struct_size = ctypes.sizeof(TTZipExtractOptions)
    opts.abi_version = 3
    opts.destination_path = ctypes.cast(ctypes.c_char_p(dst_bytes), ctypes.c_char_p)
    opts.password = None
    opts.thread_budget = threads
    opts.collision_policy = 1
    opts.preserve_permissions = True
    opts.dry_run = False
    opts.progress_callback = None
    opts.user_data = None
    return opts


def run_e2e_archive_benchmarks(
    lib: ctypes.CDLL, corpora: Dict[str, Any], work_dir: Path
) -> Tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
    """Measures Warm Page Cache (buffered close) E2E Create & Extract with per-round full tree verification."""
    e2e_rows: List[Dict[str, Any]] = []
    failure_records: List[Dict[str, Any]] = []
    out_dir = work_dir / "e2e_runs"
    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    def bench_op(
        claim_group: str,
        config_tier: str,
        corpus_id: str,
        entry_point: str,
        tool_label: str,
        config_desc: str,
        create_fn: Callable[[Path, Path], None],
        extract_fn: Callable[[Path, Path], None],
        ext_suffix: str,
        min_runs: int = 7,
        max_runs: int = 12,
    ) -> None:
        cinfo = corpora[corpus_id]
        src_path = Path(cinfo["path"])
        raw_bytes = cinfo["total_bytes"]
        expected_files = cinfo["file_count"]
        expected_manifest = cinfo["manifest_sha256"]

        create_mibs: List[float] = []
        create_gbs: List[float] = []
        extract_mibs: List[float] = []
        extract_gbs: List[float] = []
        rounds_log: List[Dict[str, Any]] = []
        archive_size = 0
        all_rounds_valid = True

        r = 0
        while r <= max_runs:
            is_warmup = r == 0
            arc_path = out_dir / f"arc_{len(e2e_rows)}_{r}{ext_suffix}"
            ext_dest = out_dir / f"ext_{len(e2e_rows)}_{r}"
            if arc_path.exists():
                arc_path.unlink()
            if ext_dest.exists():
                shutil.rmtree(ext_dest)
            ext_dest.mkdir(parents=True, exist_ok=True)

            err_msg: Optional[str] = None
            t0 = time.perf_counter_ns()
            try:
                create_fn(src_path, arc_path)
            except Exception as exc:
                err_msg = f"create_exception: {exc}"
            t1 = time.perf_counter_ns()

            archive_size = arc_path.stat().st_size if arc_path.exists() else 0
            if archive_size == 0 and err_msg is None:
                err_msg = "empty_archive_output"

            t2 = time.perf_counter_ns()
            if err_msg is None:
                try:
                    extract_fn(arc_path, ext_dest)
                except Exception as exc:
                    err_msg = f"extract_exception: {exc}"
            t3 = time.perf_counter_ns()

            ext_files, ext_bytes, actual_manifest = tree_manifest_sha256(ext_dest, src_path.name)
            round_valid = (
                err_msg is None
                and ext_files == expected_files
                and ext_bytes == raw_bytes
                and actual_manifest == expected_manifest
            )

            dt_c_ns = t1 - t0
            dt_e_ns = t3 - t2
            rounds_log.append(
                {
                    "round_index": r,
                    "is_warmup": is_warmup,
                    "create_elapsed_ns": dt_c_ns,
                    "create_elapsed_ms": round(dt_c_ns / 1e6, 3),
                    "extract_elapsed_ns": dt_e_ns,
                    "extract_elapsed_ms": round(dt_e_ns / 1e6, 3),
                    "archive_output_bytes": archive_size,
                    "extracted_file_count": ext_files,
                    "extracted_total_bytes": ext_bytes,
                    "extracted_manifest_sha256": actual_manifest,
                    "integrity_verified": round_valid,
                    "error": err_msg,
                }
            )

            arc_path.unlink(missing_ok=True)
            shutil.rmtree(ext_dest, ignore_errors=True)

            if not round_valid:
                all_rounds_valid = False
                failure_records.append(
                    {
                        "claim_group": claim_group,
                        "corpus_id": corpus_id,
                        "tool": tool_label,
                        "round_index": r,
                        "expected_files": expected_files,
                        "actual_files": ext_files,
                        "expected_bytes": raw_bytes,
                        "actual_bytes": ext_bytes,
                        "expected_manifest": expected_manifest,
                        "actual_manifest": actual_manifest,
                        "error": err_msg,
                    }
                )
                break

            if not is_warmup:
                dt_c = dt_c_ns / 1e9
                dt_e = dt_e_ns / 1e9
                create_mibs.append((raw_bytes / 1048576.0) / dt_c)
                create_gbs.append((raw_bytes / 1e9) / dt_c)
                extract_mibs.append((raw_bytes / 1048576.0) / dt_e)
                extract_gbs.append((raw_bytes / 1e9) / dt_e)

                if r >= min_runs:
                    c_st = compute_stats(create_gbs)
                    e_st = compute_stats(extract_gbs)
                    if c_st["rse_gate_passed"] and e_st["rse_gate_passed"]:
                        break
            r += 1

        c_mib_st = compute_stats(create_mibs) if all_rounds_valid else compute_stats([])
        c_gb_st = compute_stats(create_gbs) if all_rounds_valid else compute_stats([])
        e_mib_st = compute_stats(extract_mibs) if all_rounds_valid else compute_stats([])
        e_gb_st = compute_stats(extract_gbs) if all_rounds_valid else compute_stats([])

        if config_tier == "candidate_worker":
            create_ep = "Entry_A_Standalone_Engine_Dylib"
            extract_ep = "Entry_B_Candidate_Worker_Direct_CLI"
        else:
            create_ep = entry_point
            extract_ep = entry_point

        e2e_rows.append(
            {
                "claim_group": claim_group,
                "config_tier": config_tier,
                "corpus_id": corpus_id,
                "corpus_type": cinfo["corpus_type"],
                "entry_point": entry_point,
                "create_entry_point": create_ep,
                "extract_entry_point": extract_ep,
                "cache_and_flush_regime": "Warm_OS_Page_Cache_Buffered_Write_Close_No_Fsync",
                "raw_input_bytes": raw_bytes,
                "expected_file_count": expected_files,
                "archive_output_bytes": archive_size,
                "compression_ratio_pct": round(archive_size / raw_bytes * 100.0, 2) if raw_bytes else 0.0,
                "tool": tool_label,
                "config": config_desc,
                "timed_runs_n": len(create_gbs),
                "all_timed_rounds_integrity_verified": all_rounds_valid and len(create_gbs) > 0,
                "expected_tree_sha256": expected_manifest,
                "create_mib_s": c_mib_st,
                "create_gb_s_decimal": c_gb_st,
                "extract_mib_s": e_mib_st,
                "extract_gb_s_decimal": e_gb_st,
                "raw_create_gb_s_samples": [round(x, 4) for x in create_gbs],
                "raw_extract_gb_s_samples": [round(x, 4) for x in extract_gbs],
                "rounds_telemetry": rounds_log,
            }
        )

    def ttzip_create(fmt: int, level: int, threads: int) -> Callable[[Path, Path], None]:
        def _fn(src: Path, arc: Path) -> None:
            entries = sorted(p for p in src.iterdir())
            Arr = ctypes.c_char_p * len(entries)
            arr = Arr(*[str(p).encode("utf-8") for p in entries])
            opts = _make_create_opts(fmt, level, threads)
            rc = lib.ttzip_rust_archive_create_unified(
                arr, len(entries), str(arc).encode("utf-8"), ctypes.byref(opts), 0
            )
            if rc != 0:
                raise RuntimeError(f"ttzip create failed rc={rc}")
        return _fn

    def ttzip_extract(threads: int = 0) -> Callable[[Path, Path], None]:
        def _fn(arc: Path, dst: Path) -> None:
            dst_b = str(dst).encode("utf-8")
            opts = _make_extract_opts(dst_b, threads)
            out_cnt = ctypes.c_uint64(0)
            rc = lib.ttzip_rust_archive_extract_unified_v2(
                str(arc).encode("utf-8"), dst_b, ctypes.byref(opts), ctypes.byref(out_cnt), None
            )
            if rc != 0:
                raise RuntimeError(f"ttzip extract failed rc={rc}")
        return _fn

    def candidate_worker_extract() -> Callable[[Path, Path], None]:
        def _fn(arc: Path, dst: Path) -> None:
            subprocess.run(
                [str(RC_DIRECT_WORKER), "extract", "--dest-dir", str(dst), str(arc)],
                check=True,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        return _fn

    for cid in ["R1_repo_real_corpus", "S1_synthetic_repeatable_text", "S2_high_entropy", "S4_synthetic_small_tree"]:
        bench_op(
            "TAR.ZST", "default", cid, "Entry_A_Standalone_Engine_Dylib",
            "TTZip Engine (libttzip_engine.dylib)", "Zstd L3, threads=auto(18)",
            ttzip_create(FMT_TAR_ZSTD, LVL_FAST, 0), ttzip_extract(0), ".tar.zst"
        )
        if cid in ("R1_repo_real_corpus", "S1_synthetic_repeatable_text", "S2_high_entropy") and RC_DIRECT_WORKER.exists():
            bench_op(
                "TAR.ZST", "candidate_worker", cid,
                "Create:Entry_A_Standalone_Engine_Dylib / Extract:Entry_B_Candidate_Worker_Direct_CLI",
                "TTZip Paired (Create: dylib / Extract: Candidate Worker Direct CLI)",
                "Create: dylib Zstd L3 (Entry A) / Extract: ttarchive-worker direct CLI (Entry B)",
                ttzip_create(FMT_TAR_ZSTD, LVL_FAST, 0), candidate_worker_extract(), ".tar.zst"
            )
        bench_op(
            "TAR.ZST", "default", cid, "External_Competitor_CLI",
            "bsdtar + zstd (1.5.7)", "tar --zstd (L3, 1T default)",
            lambda s, a: subprocess.run(["/usr/bin/tar", "--no-mac-metadata", "--zstd", "-cf", str(a), "-C", str(s), "."], check=True, stdin=subprocess.DEVNULL),
            lambda a, d: subprocess.run(["/usr/bin/tar", "--zstd", "-xf", str(a), "-C", str(d)], check=True, stdin=subprocess.DEVNULL),
            ".tar.zst",
        )
        bench_op(
            "TAR.ZST", "equivalent_mt", cid, "External_Competitor_CLI",
            "bsdtar + zstd -T18", "zstd -3 -T18",
            lambda s, a: subprocess.run(f"/usr/bin/tar --no-mac-metadata -cf - -C '{s}' . | /opt/homebrew/bin/zstd -3 -T18 -q -o '{a}'", shell=True, check=True, stdin=subprocess.DEVNULL),
            lambda a, d: subprocess.run(f"/opt/homebrew/bin/zstd -d -q -c '{a}' | /usr/bin/tar -xf - -C '{d}'", shell=True, check=True, stdin=subprocess.DEVNULL),
            ".tar.zst",
        )

        bench_op(
            "ZIP", "default", cid, "Entry_A_Standalone_Engine_Dylib",
            "TTZip Engine (libttzip_engine.dylib)", "Deflate L6, threads=auto(18)",
            ttzip_create(FMT_ZIP, LVL_NORMAL, 0), ttzip_extract(0), ".zip"
        )
        if cid in ("R1_repo_real_corpus", "S2_high_entropy") and RC_DIRECT_WORKER.exists():
            bench_op(
                "ZIP", "candidate_worker", cid,
                "Create:Entry_A_Standalone_Engine_Dylib / Extract:Entry_B_Candidate_Worker_Direct_CLI",
                "TTZip Paired (Create: dylib / Extract: Candidate Worker Direct CLI)",
                "Create: dylib ZIP L6 (Entry A) / Extract: ttarchive-worker direct CLI (Entry B)",
                ttzip_create(FMT_ZIP, LVL_NORMAL, 0), candidate_worker_extract(), ".zip"
            )
        bench_op(
            "ZIP", "equivalent_fast", cid, "Entry_A_Standalone_Engine_Dylib",
            "TTZip Engine (libttzip_engine.dylib)", "Deflate L1, threads=18",
            ttzip_create(FMT_ZIP, LVL_FASTEST, 18), ttzip_extract(18), ".zip"
        )
        bench_op(
            "ZIP", "default", cid, "External_Competitor_CLI",
            "macOS ditto", "ditto -c -k --norsrc (L6, 1T)",
            lambda s, a: subprocess.run(["/usr/bin/ditto", "-c", "-k", "--norsrc", str(s), str(a)], check=True, stdin=subprocess.DEVNULL),
            lambda a, d: subprocess.run(["/usr/bin/ditto", "-x", "-k", "--norsrc", str(a), str(d)], check=True, stdin=subprocess.DEVNULL),
            ".zip",
        )
        bench_op(
            "ZIP", "default", cid, "External_Competitor_CLI",
            "7zz (26.02)", "7zz a -tzip (mx=5, mmt=18T)",
            lambda s, a: subprocess.run(["/opt/homebrew/bin/7zz", "a", "-tzip", "-bso0", "-bsp0", str(a), "."], cwd=str(s), check=True, stdin=subprocess.DEVNULL),
            lambda a, d: subprocess.run(["/opt/homebrew/bin/7zz", "x", "-y", "-bso0", "-bsp0", f"-o{d}", str(a)], check=True, stdin=subprocess.DEVNULL),
            ".zip",
        )
        bench_op(
            "ZIP", "equivalent_fast", cid, "External_Competitor_CLI",
            "7zz (26.02)", "7zz a -tzip -mx=1 -mmt=18",
            lambda s, a: subprocess.run(["/opt/homebrew/bin/7zz", "a", "-tzip", "-mx=1", "-mmt=18", "-bso0", "-bsp0", str(a), "."], cwd=str(s), check=True, stdin=subprocess.DEVNULL),
            lambda a, d: subprocess.run(["/opt/homebrew/bin/7zz", "x", "-y", "-mmt=18", "-bso0", "-bsp0", f"-o{d}", str(a)], check=True, stdin=subprocess.DEVNULL),
            ".zip",
        )

    bench_op(
        "TAR.GZ", "default", "R1_repo_real_corpus", "Entry_A_Standalone_Engine_Dylib",
        "TTZip Engine (libttzip_engine.dylib)", "Gzip L6 (single-thread libarchive create / zlib-ng extract)",
        ttzip_create(FMT_TAR_GZ, LVL_NORMAL, 0), ttzip_extract(0), ".tar.gz"
    )
    bench_op(
        "TAR.GZ", "default", "R1_repo_real_corpus", "External_Competitor_CLI",
        "bsdtar -czf", "bsdtar internal gzip (L6, 1T)",
        lambda s, a: subprocess.run(["/usr/bin/tar", "--no-mac-metadata", "-czf", str(a), "-C", str(s), "."], check=True, stdin=subprocess.DEVNULL),
        lambda a, d: subprocess.run(["/usr/bin/tar", "-xzf", str(a), "-C", str(d)], check=True, stdin=subprocess.DEVNULL),
        ".tar.gz",
    )
    bench_op(
        "TAR.GZ", "equivalent_mt", "R1_repo_real_corpus", "External_Competitor_CLI",
        "bsdtar + pigz (2.8)", "pigz -6 -p 18",
        lambda s, a: subprocess.run(f"/usr/bin/tar --no-mac-metadata -cf - -C '{s}' . | /opt/homebrew/bin/pigz -6 -p 18 > '{a}'", shell=True, check=True, stdin=subprocess.DEVNULL),
        lambda a, d: subprocess.run(f"/opt/homebrew/bin/pigz -d -p 18 -c '{a}' | /usr/bin/tar -xf - -C '{d}'", shell=True, check=True, stdin=subprocess.DEVNULL),
        ".tar.gz",
    )

    shutil.rmtree(out_dir, ignore_errors=True)
    return e2e_rows, failure_records


def compute_gated_comparisons(e2e_rows: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Computes Welch's t-test and speedup ratios ONLY when both sides pass integrity and RSE <= 2.0%."""
    comparisons: List[Dict[str, Any]] = []
    by_group_corpus: Dict[Tuple[str, str], List[Dict[str, Any]]] = {}
    for row in e2e_rows:
        by_group_corpus.setdefault((row["claim_group"], row["corpus_id"]), []).append(row)

    for (claim_group, cid), rows in by_group_corpus.items():
        ttzip_rows = [r for r in rows if r["tool"].startswith("TTZip")]
        comp_rows = [r for r in rows if not r["tool"].startswith("TTZip")]
        for t_row in ttzip_rows:
            for c_row in comp_rows:
                if t_row["config_tier"] == "equivalent_fast" and c_row["config_tier"] != "equivalent_fast":
                    continue
                if t_row["config_tier"] != "equivalent_fast" and c_row["config_tier"] == "equivalent_fast":
                    continue
                for metric_kind, t_key, samples_key in [
                    ("create", "create_gb_s_decimal", "raw_create_gb_s_samples"),
                    ("extract", "extract_gb_s_decimal", "raw_extract_gb_s_samples"),
                ]:
                    t_st = t_row[t_key]
                    c_st = c_row[t_key]
                    both_valid = (
                        t_row["all_timed_rounds_integrity_verified"]
                        and c_row["all_timed_rounds_integrity_verified"]
                    )
                    both_rse_ok = bool(t_st["rse_gate_passed"] and c_st["rse_gate_passed"])
                    t_test = welch_t_test(t_row[samples_key], c_row[samples_key])
                    raw_ratio = round(t_st["median"] / c_st["median"], 2) if c_st["median"] > 0 else None
                    if metric_kind == "create":
                        op_ep = t_row.get("create_entry_point", "Entry_A_Standalone_Engine_Dylib")
                        op_tool = (
                            "TTZip Engine (libttzip_engine.dylib) [paired run for worker extract]"
                            if t_row["config_tier"] == "candidate_worker"
                            else t_row["tool"]
                        )
                    else:
                        op_ep = t_row.get("extract_entry_point", t_row["entry_point"])
                        op_tool = (
                            "TTZip Candidate Worker Direct CLI (ttarchive-worker)"
                            if t_row["config_tier"] == "candidate_worker"
                            else t_row["tool"]
                        )
                    comparisons.append(
                        {
                            "claim_group": claim_group,
                            "ttzip_config_tier": t_row["config_tier"],
                            "competitor_config_tier": c_row["config_tier"],
                            "corpus_id": cid,
                            "corpus_type": t_row["corpus_type"],
                            "operation": metric_kind,
                            "ttzip_entry_point": op_ep,
                            "ttzip_tool": op_tool,
                            "competitor_tool": c_row["tool"],
                            "ttzip_median_gb_s": t_st["median"],
                            "ttzip_rse_pct": t_st["rse_pct"],
                            "competitor_median_gb_s": c_st["median"],
                            "competitor_rse_pct": c_st["rse_pct"],
                            "rse_gate_2pct_both_passed": both_rse_ok,
                            "welch_t_test": t_test,
                            "published_speedup_x": raw_ratio if (both_valid and both_rse_ok) else None,
                            "unverified_exploratory_ratio_x": raw_ratio,
                            "speedup_publication_status": (
                                "EXPLORATORY_RSE_GATE_PASSED"
                                if (both_valid and both_rse_ok)
                                else "WITHHELD_RSE_EXCEEDS_2PCT_GATE"
                            ),
                        }
                    )
    return comparisons


def compute_summary_counts(e2e_rows: List[Dict[str, Any]], failures: List[Dict[str, Any]]) -> Dict[str, int]:
    formal_timed = sum(int(r.get("timed_runs_n", 0)) for r in e2e_rows)
    total_rounds = sum(len(r.get("rounds_telemetry", [])) for r in e2e_rows)
    warmup_rounds = total_rounds - formal_timed
    return {
        "e2e_configurations_count": len(e2e_rows),
        "e2e_formal_timed_rounds_total": formal_timed,
        "e2e_warmup_rounds_total": warmup_rounds,
        "e2e_total_verified_rounds": total_rounds,
        "integrity_failures_count": len(failures),
    }


def main() -> None:
    rc_direct_meta = json.loads(RC_DIRECT_MANIFEST.read_text(encoding="utf-8")) if RC_DIRECT_MANIFEST.exists() else {}
    rc_accept_meta = json.loads(RC_ACCEPT_MANIFEST.read_text(encoding="utf-8")) if RC_ACCEPT_MANIFEST.exists() else {}

    entry_inventory = {
        "Entry_A_Standalone_Engine_Dylib": {
            "measured_in_this_run": True,
            "Scope": "C-ABI in-process library calls (codec microbenchmarks + unified archive create/extract)",
            "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, text=True).strip(),
            "dylib_path": str(DYLIB_PATH),
            "dylib_sha256": sha256_file(DYLIB_PATH),
            "ttzip_cli_sha256": sha256_file(CLI_PATH) if CLI_PATH.exists() else None,
        },
        "Entry_B_Candidate_Worker_Direct_CLI": {
            "measured_in_this_run": RC_DIRECT_WORKER.exists(),
            "Scope": "Direct CLI subprocess invocation of candidate ttarchive-worker (`extract --dest-dir`) with dir_jail and staged_metadata; does NOT cover host App Sandbox launch chain, security-scoped bookmark resolution, or XPC/pipe handoff (verified separately by H); create is not supported by ttarchive-worker CLI and remains Entry A dylib",
            "worker_path": str(RC_DIRECT_WORKER),
            "worker_sha256": sha256_file(RC_DIRECT_WORKER) if RC_DIRECT_WORKER.exists() else None,
            "manifest_git_commit": rc_direct_meta.get("git_commit"),
            "manifest_dirty_files_count": len(rc_direct_meta.get("git_dirty_files", [])),
        },
        "Entry_C_Candidate_GUI_App": {
            "measured_in_this_run": False,
            "Scope": "Full macOS SwiftUI/AppKit GUI (`TTZip.app/Contents/MacOS/TTZip`) user waiting time and sandboxed worker launch chain - verified separately by H/P2",
            "direct_app_sha256": rc_direct_meta.get("executables_sha256", {}).get("TTZip"),
            "acceptance_app_sha256": rc_accept_meta.get("executables_sha256", {}).get("TTZip"),
        },
    }

    if "--recompute-comparisons" in sys.argv and OUT_JSON.exists():
        data = json.loads(OUT_JSON.read_text(encoding="utf-8"))
        for row in data.get("e2e_archive_benchmarks", []):
            if row.get("config_tier") == "candidate_worker":
                row["create_entry_point"] = "Entry_A_Standalone_Engine_Dylib"
                row["extract_entry_point"] = "Entry_B_Candidate_Worker_Direct_CLI"
                row["entry_point"] = "Create:Entry_A_Standalone_Engine_Dylib / Extract:Entry_B_Candidate_Worker_Direct_CLI"
                row["tool"] = "TTZip Paired (Create: dylib / Extract: Candidate Worker Direct CLI)"
                if row.get("claim_group") == "TAR.ZST":
                    row["config"] = "Create: dylib Zstd L3 (Entry A) / Extract: ttarchive-worker direct CLI (Entry B)"
                elif row.get("claim_group") == "ZIP":
                    row["config"] = "Create: dylib ZIP L6 (Entry A) / Extract: ttarchive-worker direct CLI (Entry B)"
            else:
                ep = row.get("entry_point", "")
                row["create_entry_point"] = ep
                row["extract_entry_point"] = ep
        data["entry_points_inventory"] = entry_inventory
        data["summary_counts"] = compute_summary_counts(
            data.get("e2e_archive_benchmarks", []), data.get("integrity_failure_records", [])
        )
        data["gated_speedup_comparisons"] = compute_gated_comparisons(data["e2e_archive_benchmarks"])
        OUT_JSON.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
        print(
            f"[Q4-Rev2] Synchronized metadata, summary_counts={data['summary_counts']}, "
            f"and {len(data['gated_speedup_comparisons'])} gated comparisons in {OUT_JSON}"
        )
        return

    work_dir = Path("/tmp/ttzip_q4_verification_v2")
    work_dir.mkdir(parents=True, exist_ok=True)
    corpora = generate_corpora(work_dir / "corpora")
    lib = load_engine()
    in_mem = run_in_memory_microbenchmarks(lib, corpora)
    e2e, failures = run_e2e_archive_benchmarks(lib, corpora, work_dir)
    comparisons = compute_gated_comparisons(e2e)

    report = {
        "schema": "ttzip.q4.performance_verification.v2",
        "classification": "STANDALONE_ENGINE_AND_CANDIDATE_WORKER_DIRECT_CLI_EXPLORATORY_RESULTS_PENDING_H_P2_SIGN_OFF",
        "summary_counts": compute_summary_counts(e2e, failures),
        "environment": {
            "os_uname": " ".join(platform.uname()),
            "python_version": platform.python_version(),
            "cpu_count": os.cpu_count(),
            "loadavg_at_completion": os.getloadavg(),
            "cache_regime": "Warm_OS_Page_Cache",
            "disk_flush_semantics": "Buffered_Write_Close_Without_F_FULLFSYNC",
            "statistical_gate_rse_max_pct": RSE_GATE_THRESHOLD_PCT,
            "outlier_filter": "Hampel_3.0_sigma_MAD",
        },
        "entry_points_inventory": entry_inventory,
        "corpora": corpora,
        "integrity_failure_records": failures,
        "in_memory_microbenchmarks": in_mem,
        "e2e_archive_benchmarks": e2e,
        "gated_speedup_comparisons": comparisons,
    }
    OUT_JSON.parent.mkdir(parents=True, exist_ok=True)
    OUT_JSON.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    shutil.rmtree(work_dir, ignore_errors=True)
    print(f"[Q4-Rev2] Complete! Failures={len(failures)}. Telemetry written to {OUT_JSON}")


if __name__ == "__main__":
    main()
