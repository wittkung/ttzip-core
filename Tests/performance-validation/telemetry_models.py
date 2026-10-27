#!/usr/bin/env python3
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

"""C-ABI bindings, statistical evaluators (Hampel 3-sigma, RSE <= 2.0%, Welch's t-test),
full relative-path SHA-256 tree manifest verifier, and corpus builders for Q4 verification.
"""

from __future__ import annotations

import ctypes
import hashlib
import math
import os
import shutil
import statistics
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

REPO_ROOT = Path(__file__).resolve().parents[5]
OPT_BIN = Path(
    "/private/var/tmp/_bazel_kevintung/bc7b818d367659311eafb4e72b48b678"
    "/execroot/_main/bazel-out/darwin_arm64-opt/bin"
)
DYLIB_PATH = OPT_BIN / "products/ttzip/core/rust/ttzip-engine/libttzip_engine.dylib"
CLI_PATH = OPT_BIN / "products/ttzip/core/rust/ttzip-tui/ttzip_cli"
RC_DIRECT_MANIFEST = REPO_ROOT / "products/ttzip/apple/dist/bazel/candidates/rc2-preview/direct/release-manifest.json"
RC_DIRECT_WORKER = (
    REPO_ROOT / "products/ttzip/apple/dist/bazel/candidates/rc2-preview/direct/TTZip.app/Contents/Helpers/ttarchive-worker"
)
RC_ACCEPT_MANIFEST = (
    REPO_ROOT / "products/ttzip/apple/dist/bazel/candidates/rc2-preview/acceptance/release-manifest.json"
)
OUT_JSON = REPO_ROOT / "products/ttzip/docs/acceptance/q4-performance/reproducible-measurements.json"

RSE_GATE_THRESHOLD_PCT = 2.0

# Rust ArchiveFormat enum discriminants from crates/ttzip-types/src/archive_format.rs
FMT_SEVENZIP = 1
FMT_ZIP = 2
FMT_TAR = 4
FMT_TAR_GZ = 63
FMT_TAR_ZSTD = 66

# TTZipCompressionLevel discriminants from crates/ttzip-types/src/formats.rs
LVL_STORE = 0
LVL_FASTEST = 1
LVL_FAST = 3
LVL_NORMAL = 6
LVL_MAXIMUM = 9


class TTZipCreateOptions(ctypes.Structure):
    """C-ABI layout matching TTZipCreateOptions in crates/ttzip-types/src/options/mod.rs."""

    _fields_ = [
        ("struct_size", ctypes.c_uint32),
        ("abi_version", ctypes.c_uint32),
        ("format", ctypes.c_int32),
        ("level", ctypes.c_int32),
        ("encryption", ctypes.c_int32),
        ("password", ctypes.c_char_p),
        ("thread_budget", ctypes.c_uint32),
        ("solid_block_size_mb", ctypes.c_uint32),
        ("progress_callback", ctypes.c_void_p),
        ("user_data", ctypes.c_void_p),
    ]


class TTZipExtractOptions(ctypes.Structure):
    """C-ABI layout matching TTZipExtractOptions in crates/ttzip-types/src/options/mod.rs."""

    _fields_ = [
        ("struct_size", ctypes.c_uint32),
        ("abi_version", ctypes.c_uint32),
        ("destination_path", ctypes.c_char_p),
        ("password", ctypes.c_char_p),
        ("thread_budget", ctypes.c_uint32),
        ("collision_policy", ctypes.c_uint8),
        ("preserve_permissions", ctypes.c_bool),
        ("dry_run", ctypes.c_bool),
        ("progress_callback", ctypes.c_void_p),
        ("user_data", ctypes.c_void_p),
    ]


class TTZipEntryMetadata(ctypes.Structure):
    """C-ABI layout matching TTZipEntryMetadata in crates/ttzip-types/src/dto.rs."""

    _fields_ = [
        ("struct_size", ctypes.c_uint32),
        ("abi_version", ctypes.c_uint32),
        ("path", ctypes.c_char_p),
        ("uncompressed_size", ctypes.c_uint64),
        ("compressed_size", ctypes.c_uint64),
        ("crc32", ctypes.c_uint32),
        ("mtime_epoch_secs", ctypes.c_int64),
        ("mode", ctypes.c_uint32),
        ("is_directory", ctypes.c_bool),
        ("is_encrypted", ctypes.c_bool),
        ("compression_method", ctypes.c_uint16),
        ("detected_encoding", ctypes.c_char_p),
    ]


class TTZipVfsMatchDto(ctypes.Structure):
    """C-ABI layout matching TTZipVfsMatchDto in ttzip-engine/src/fs/vfs/search.rs."""

    _fields_ = [
        ("struct_size", ctypes.c_uint32),
        ("abi_version", ctypes.c_uint32),
        ("name", ctypes.c_char_p),
        ("name_len", ctypes.c_size_t),
        ("path", ctypes.c_char_p),
        ("path_len", ctypes.c_size_t),
        ("uncompressed_size", ctypes.c_uint64),
        ("compressed_size", ctypes.c_uint64),
        ("crc32", ctypes.c_uint32),
        ("score", ctypes.c_int32),
        ("is_directory", ctypes.c_bool),
        ("is_encrypted", ctypes.c_bool),
    ]


def sha256_file(path: Path) -> str:
    """Computes SHA-256 hex digest of a file."""
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(1024 * 1024):
            h.update(chunk)
    return h.hexdigest()


def tree_manifest_sha256(root: Path, wrapper_dir_name: Optional[str] = None) -> Tuple[int, int, str]:
    """Computes deterministic (file_count, total_bytes, manifest_sha256) preserving full relative paths."""
    if root.is_file():
        sz = root.stat().st_size
        return 1, sz, sha256_file(root)

    effective_root = root
    if wrapper_dir_name is not None:
        candidate = root / wrapper_dir_name
        top_entries = [p for p in root.iterdir() if not p.name.startswith(".DS_Store")]
        if len(top_entries) == 1 and top_entries[0].is_dir() and top_entries[0].name == wrapper_dir_name:
            effective_root = candidate

    files: List[Tuple[str, Path]] = []
    for dirpath, _, filenames in os.walk(effective_root):
        for fn in filenames:
            p = Path(dirpath) / fn
            if p.is_file() and not p.is_symlink():
                rel = p.relative_to(effective_root).as_posix()
                files.append((rel, p))
    files.sort(key=lambda item: item[0])

    manifest = hashlib.sha256()
    total_bytes = 0
    for rel_path, fpath in files:
        sz = fpath.stat().st_size
        total_bytes += sz
        fhash = sha256_file(fpath)
        manifest.update(f"{rel_path}:{sz}:{fhash}\n".encode("utf-8"))
    return len(files), total_bytes, manifest.hexdigest()


def hampel_filter_3sigma(samples: List[float], k: float = 3.0) -> Tuple[List[float], List[float], float]:
    """Applies the Hampel 3.0*sigma_MAD outlier filter matching `ttzip-engine/src/benchmark/stats.rs`."""
    if len(samples) < 3:
        return list(samples), [], 0.0
    med = statistics.median(samples)
    abs_devs = [abs(x - med) for x in samples]
    mad = statistics.median(abs_devs)
    sigma = 1.4826 * mad
    if sigma <= 1e-15:
        return list(samples), [], 0.0
    threshold = k * sigma
    cleaned = [x for x in samples if abs(x - med) <= threshold]
    outliers = [x for x in samples if abs(x - med) > threshold]
    return (cleaned if cleaned else list(samples)), outliers, mad


def compute_stats(samples: List[float]) -> Dict[str, Any]:
    """Computes raw and Hampel-cleaned statistics and evaluates the strict `RSE <= 2.0%` gate."""
    if not samples:
        return {
            "n_raw": 0,
            "n_clean": 0,
            "outliers_removed": 0,
            "median": 0.0,
            "iqr": 0.0,
            "mean": 0.0,
            "sd": 0.0,
            "rse_pct": 0.0,
            "raw_rse_pct": 0.0,
            "rse_gate_threshold_pct": RSE_GATE_THRESHOLD_PCT,
            "rse_gate_passed": False,
            "min": 0.0,
            "max": 0.0,
        }
    cleaned, outliers, mad = hampel_filter_3sigma(samples, k=3.0)
    s_raw = sorted(samples)
    s_clean = sorted(cleaned)
    n_raw = len(s_raw)
    n_clean = len(s_clean)

    med = statistics.median(s_clean)
    iqr = (
        (statistics.quantiles(s_clean, n=4)[2] - statistics.quantiles(s_clean, n=4)[0])
        if n_clean >= 4
        else (s_clean[-1] - s_clean[0])
    )
    mean_clean = statistics.mean(s_clean)
    sd_clean = statistics.stdev(s_clean) if n_clean > 1 else 0.0
    rse_clean = ((sd_clean / math.sqrt(n_clean)) / mean_clean * 100.0) if mean_clean > 0 else 0.0

    mean_raw = statistics.mean(s_raw)
    sd_raw = statistics.stdev(s_raw) if n_raw > 1 else 0.0
    rse_raw = ((sd_raw / math.sqrt(n_raw)) / mean_raw * 100.0) if mean_raw > 0 else 0.0

    return {
        "n_raw": n_raw,
        "n_clean": n_clean,
        "outliers_removed": len(outliers),
        "median": round(med, 3),
        "iqr": round(iqr, 3),
        "mad": round(mad, 3),
        "mean": round(mean_clean, 3),
        "sd": round(sd_clean, 3),
        "rse_pct": round(rse_clean, 2),
        "raw_rse_pct": round(rse_raw, 2),
        "rse_gate_threshold_pct": RSE_GATE_THRESHOLD_PCT,
        "rse_gate_passed": bool(rse_clean <= RSE_GATE_THRESHOLD_PCT),
        "min": round(s_clean[0], 3),
        "max": round(s_clean[-1], 3),
    }


def welch_t_test(samples_a: List[float], samples_b: List[float]) -> Dict[str, Any]:
    """Executes Welch's two-sample heteroscedastic t-test on Hampel-cleaned samples."""
    clean_a, _, _ = hampel_filter_3sigma(samples_a)
    clean_b, _, _ = hampel_filter_3sigma(samples_b)
    na, nb = len(clean_a), len(clean_b)
    if na < 2 or nb < 2:
        return {"t_stat": 0.0, "df": 0.0, "significant_alpha_0_05": False}
    ma, mb = statistics.mean(clean_a), statistics.mean(clean_b)
    va, vb = statistics.variance(clean_a), statistics.variance(clean_b)
    se_sq = (va / na) + (vb / nb)
    if se_sq <= 1e-18:
        return {"t_stat": 0.0, "df": float(na + nb - 2), "significant_alpha_0_05": False}
    t_stat = (ma - mb) / math.sqrt(se_sq)
    num = se_sq * se_sq
    den = ((va / na) ** 2) / (na - 1) + ((vb / nb) ** 2) / (nb - 1)
    df = num / den if den > 1e-18 else float(na + nb - 2)
    t_crit = 1.96 + (2.4 / max(df, 2.0)) + (1.8 / (max(df, 2.0) ** 2))
    return {
        "t_stat": round(t_stat, 3),
        "df": round(df, 2),
        "t_crit_alpha_0_05": round(t_crit, 3),
        "significant_alpha_0_05": bool(abs(t_stat) > t_crit),
    }


def load_engine() -> ctypes.CDLL:
    """Loads the -c opt libttzip_engine.dylib and binds function signatures."""
    lib = ctypes.CDLL(str(DYLIB_PATH))

    lib.ttzip_rust_crc32.argtypes = [ctypes.c_uint32, ctypes.c_char_p, ctypes.c_size_t]
    lib.ttzip_rust_crc32.restype = ctypes.c_uint32

    lib.ttzip_rust_deflate_compress_bound.argtypes = [ctypes.c_size_t, ctypes.c_int32]
    lib.ttzip_rust_deflate_compress_bound.restype = ctypes.c_size_t

    lib.ttzip_rust_deflate_compress.argtypes = [
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.c_int32,
        ctypes.POINTER(ctypes.c_size_t),
    ]
    lib.ttzip_rust_deflate_compress.restype = ctypes.c_int32

    lib.ttzip_rust_deflate_decompress.argtypes = [
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.POINTER(ctypes.c_size_t),
    ]
    lib.ttzip_rust_deflate_decompress.restype = ctypes.c_int32

    lib.ttzip_rust_archive_create_unified.argtypes = [
        ctypes.POINTER(ctypes.c_char_p),
        ctypes.c_size_t,
        ctypes.c_char_p,
        ctypes.POINTER(TTZipCreateOptions),
        ctypes.c_uint64,
    ]
    lib.ttzip_rust_archive_create_unified.restype = ctypes.c_int32

    lib.ttzip_rust_archive_extract_unified_v2.argtypes = [
        ctypes.c_char_p,
        ctypes.c_char_p,
        ctypes.POINTER(TTZipExtractOptions),
        ctypes.POINTER(ctypes.c_uint64),
        ctypes.c_void_p,
    ]
    lib.ttzip_rust_archive_extract_unified_v2.restype = ctypes.c_int32

    lib.ttzip_rust_vfs_tree_build.argtypes = [
        ctypes.POINTER(TTZipEntryMetadata),
        ctypes.c_size_t,
        ctypes.c_char_p,
    ]
    lib.ttzip_rust_vfs_tree_build.restype = ctypes.c_void_p

    lib.ttzip_rust_vfs_search_zero_alloc.argtypes = [
        ctypes.c_void_p,
        ctypes.c_char_p,
        ctypes.POINTER(TTZipVfsMatchDto),
        ctypes.c_int32,
    ]
    lib.ttzip_rust_vfs_search_zero_alloc.restype = ctypes.c_int32

    lib.ttzip_rust_vfs_tree_free.argtypes = [ctypes.c_void_p]
    lib.ttzip_rust_vfs_tree_free.restype = None

    return lib


def generate_corpora(base_dir: Path) -> Dict[str, Dict[str, Any]]:
    """Generates the authorized real-world corpus `R1` plus explicitly tagged synthetic corpora."""
    corpora: Dict[str, Dict[str, Any]] = {}

    r1_dir = base_dir / "R1_repo_real_corpus"
    if r1_dir.exists():
        shutil.rmtree(r1_dir)
    r1_dir.mkdir(parents=True, exist_ok=True)
    real_sources = [
        ("rust_crates_types", REPO_ROOT / "products/ttzip/core/rust/crates/ttzip-types/src"),
        ("rust_crates_codecs", REPO_ROOT / "products/ttzip/core/rust/crates/ttzip-codecs/src"),
        ("rust_engine_src", REPO_ROOT / "products/ttzip/core/rust/ttzip-engine/src"),
        ("docs_twin", REPO_ROOT / "products/ttzip/docs/twin"),
        ("core_specs", REPO_ROOT / "products/ttzip/core/specs"),
    ]
    for subdir_name, src_tree in real_sources:
        if src_tree.exists():
            shutil.copytree(
                src_tree,
                r1_dir / subdir_name,
                symlinks=False,
                ignore=shutil.ignore_patterns(".*", "__pycache__", "*.pyc", "target"),
            )

    s1_dir = base_dir / "S1_synthetic_repeatable_text"
    s1_dir.mkdir(parents=True, exist_ok=True)
    s1_file = s1_dir / "structured_telemetry.log"
    lines = [
        f'2026-10-18T14:00:{i % 60:02d}.123Z [INFO] worker={i % 18} '
        f'module="ttzip::archive::stream" event="chunk_flushed" '
        f'seq={i * 17} bytes={4096 + (i % 512)} crc32="0x{(i * 2654435761) & 0xFFFFFFFF:08x}" '
        f'path="/workspace/metastudyline/products/ttzip/src/node_{i % 256}.rs"\n'
        for i in range(4096)
    ]
    block = "".join(lines).encode("utf-8")
    target_s1 = 32 * 1024 * 1024
    with s1_file.open("wb") as f:
        written, idx = 0, 0
        while written < target_s1:
            chunk = f"--- SEGMENT {idx:06d} ---\n".encode("utf-8") + block
            take = min(len(chunk), target_s1 - written)
            f.write(chunk[:take])
            written += take
            idx += 1

    s2_dir = base_dir / "S2_high_entropy"
    s2_dir.mkdir(parents=True, exist_ok=True)
    s2_file = s2_dir / "random_media_payload.bin"
    target_s2 = 32 * 1024 * 1024
    with s2_file.open("wb") as f:
        buf = bytearray(target_s2)
        mv = memoryview(buf)
        base_seed = hashlib.blake2b(b"TTZIP-Q4-HIGH-ENTROPY-SEED", digest_size=64).digest()
        chunk_64k = bytearray(65536)
        for j in range(1024):
            chunk_64k[j * 64 : (j + 1) * 64] = hashlib.blake2b(
                base_seed, person=f"blk-{j}".encode("ascii"), digest_size=64
            ).digest()
        for blk_idx in range(target_s2 // 65536):
            mask = hashlib.blake2b(f"macro-{blk_idx}".encode("ascii"), digest_size=64).digest()
            mv[blk_idx * 65536 : (blk_idx + 1) * 65536] = bytes(
                b ^ mask[k % 64] for k, b in enumerate(chunk_64k)
            )
        f.write(buf)

    s4_dir = base_dir / "S4_synthetic_small_tree"
    s4_dir.mkdir(parents=True, exist_ok=True)
    for i in range(1000):
        sub = s4_dir / f"pkg_{i // 50:02d}"
        sub.mkdir(parents=True, exist_ok=True)
        fp = sub / f"item_{i:04d}.txt"
        content = (
            f"small_file_id={i:04d} dir=pkg_{i // 50:02d} payload="
            + ("ABCD1234_TTZIP_Q4_VALIDATION_" * 340)
        ).encode("utf-8")[:10240]
        fp.write_bytes(content)

    for cid, cpath, ctype, desc in [
        (
            "R1_repo_real_corpus",
            r1_dir,
            "authorized_real_world",
            "First-party TTZip Rust source, specs & twin documentation multi-directory tree",
        ),
        (
            "S1_synthetic_repeatable_text",
            s1_dir,
            "synthetic_high_repetition",
            "Synthetic repeatable log template (32 MiB; tests sliding-window upper bound, NOT real business data)",
        ),
        (
            "S2_high_entropy",
            s2_dir,
            "synthetic_incompressible",
            "Deterministic BLAKE2b high-entropy incompressible stream (32 MiB)",
        ),
        (
            "S4_synthetic_small_tree",
            s4_dir,
            "synthetic_small_files_topology",
            "Synthetic 1,000 small files across 20 subdirectories",
        ),
    ]:
        fcount, tbytes, mhash = tree_manifest_sha256(cpath)
        corpora[cid] = {
            "path": str(cpath),
            "corpus_type": ctype,
            "description": desc,
            "file_count": fcount,
            "total_bytes": tbytes,
            "size_mib": round(tbytes / (1024.0 * 1024.0), 4),
            "size_mb": round(tbytes / 1_000_000.0, 4),
            "manifest_sha256": mhash,
            "manifest_includes_relative_paths": True,
        }
    return corpora
