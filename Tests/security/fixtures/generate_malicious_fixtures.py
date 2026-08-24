#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# Malicious & Security Test Fixtures Generator for TTZip SDK Testing Matrix.
# Produces:
#   1. Zip Slip directory traversal archives (relative ../, absolute /, windows ..\)
#   2. High compression ratio Zip Bomb archives & descriptors (1000:1+ expansion)
#   3. Truncated & corrupted headers across ZIP, TAR, GZIP, and ZSTD formats
#   4. Structured contract-compliant JSON scenario manifest.

import argparse
import hashlib
import io
import json
import os
import shutil
import struct
import sys
import tarfile
import zlib
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Any

DEFAULT_FIXTURES_DIR = Path(__file__).resolve().parent / "corpus"
DEFAULT_MANIFEST_PATH = Path(__file__).resolve().parent / "security_fixtures_manifest.json"


def compute_sha256_bytes(data: bytes) -> str:
    """Compute SHA-256 hex digest of in-memory bytes."""
    return hashlib.sha256(data).hexdigest()


class MaliciousFixturesGenerator:
    """Constructs adversarial and corrupted archive samples for fuzzing & security tests."""

    def __init__(self, output_dir: Path, verbose: bool = False):
        self.output_dir = output_dir.resolve()
        self.verbose = verbose
        self.scenarios: List[Dict[str, Any]] = []

    def log(self, msg: str) -> None:
        if self.verbose:
            print(f"[SecurityFixtures] {msg}")

    def clean(self) -> None:
        if self.output_dir.exists():
            self.log(f"Cleaning existing directory: {self.output_dir}")
            shutil.rmtree(self.output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)

    def register_scenario(
        self,
        name: str,
        target_sdk: str,
        attack_payload: str,
        expected_outcome: str,
        actual_outcome: str = "FIXTURE_GENERATED",
        status: str = "passed",
        ratio: Optional[float] = None,
        out_of_bounds_written: bool = False,
        peak_rss_mb: float = 0.0,
    ) -> Dict[str, Any]:
        """Registers a security scenario compliant with SecurityFixtureContract."""
        scenario: Dict[str, Any] = {
            "name": name,
            "targetSdk": target_sdk,
            "attackPayload": attack_payload,
            "expectedOutcome": expected_outcome,
            "actualOutcome": actual_outcome,
            "outOfBoundsWritten": out_of_bounds_written,
            "peakRssMb": peak_rss_mb,
            "status": status,
        }
        if ratio is not None:
            scenario["ratio"] = float(ratio)
        self.scenarios.append(scenario)
        return scenario

    # -------------------------------------------------------------------------
    # Low-level Raw ZIP Archive Builder (Bypasses stdlib path sanitization)
    # -------------------------------------------------------------------------
    @staticmethod
    def _build_raw_zip(entries: List[Tuple[bytes, bytes, int]]) -> bytes:
        """
        Builds raw ZIP bytes.
        entries is a list of tuples: (filename_bytes, content_bytes, compression_method)
        compression_method: 0 = Store, 8 = Deflate
        """
        body = bytearray()
        cd_records = bytearray()
        cd_count = len(entries)

        for filename, data, method in entries:
            offset = len(body)
            uncomp_size = len(data)
            crc = zlib.crc32(data) & 0xFFFFFFFF

            if method == 8:
                comp_obj = zlib.compressobj(level=9, method=zlib.DEFLATED, wbits=-15)
                comp_data = comp_obj.compress(data) + comp_obj.flush()
                comp_size = len(comp_data)
            else:
                method = 0
                comp_data = data
                comp_size = uncomp_size

            # Local File Header
            local_hdr = struct.pack(
                "<4sHHHHHIIIHH",
                b"PK\x03\x04",
                20,      # version needed
                0,       # general purpose bit flag
                method,  # compression method
                0,       # last mod time
                0,       # last mod date
                crc,
                comp_size,
                uncomp_size,
                len(filename),
                0,       # extra field length
            ) + filename + comp_data
            body.extend(local_hdr)

            # Central Directory Record
            cd_record = struct.pack(
                "<4sHHHHHHIIIHHHHHII",
                b"PK\x01\x02",
                20,      # version made by
                20,      # version needed
                0,       # flags
                method,
                0,       # time
                0,       # date
                crc,
                comp_size,
                uncomp_size,
                len(filename),
                0,       # extra field length
                0,       # file comment length
                0,       # disk number start
                0,       # internal file attributes
                0,       # external file attributes
                offset,  # relative offset of local header
            ) + filename
            cd_records.extend(cd_record)

        cd_offset = len(body)
        cd_size = len(cd_records)

        # End of Central Directory Record (EOCD)
        eocd = struct.pack(
            "<4sHHHHIIH",
            b"PK\x05\x06",
            0,         # disk number
            0,         # disk with central dir
            cd_count,  # entries on this disk
            cd_count,  # total entries
            cd_size,
            cd_offset,
            0,         # comment length
        )

        return bytes(body + cd_records + eocd)

    # -------------------------------------------------------------------------
    # 1. Zip Slip Attack Archive Generators
    # -------------------------------------------------------------------------
    def generate_zip_slip_fixtures(self) -> None:
        """Generates various Zip Slip path traversal attack vectors."""
        self.log("Generating Zip Slip attack fixtures...")
        slip_dir = self.output_dir / "zip_slip"
        slip_dir.mkdir(parents=True, exist_ok=True)

        payload_bytes = b"TTZIP_SECURITY_GATE_TRAVERSAL_PAYLOAD_UNAUTHORIZED_OVERWRITE\n"

        # 1.1 Relative directory traversal (../../evil_relative.txt)
        file_rel = slip_dir / "zip_slip_relative.zip"
        rel_entries = [
            (b"../../evil_relative.txt", payload_bytes, 0),
            (b"nested/dir/../../../../escape_depth4.txt", payload_bytes, 0),
            (b"valid_file.txt", b"Valid Safe File", 0),
        ]
        file_rel.write_bytes(self._build_raw_zip(rel_entries))
        self.register_scenario(
            name="zip_slip_relative_traversal",
            target_sdk="all",
            attack_payload=file_rel.name,
            expected_outcome="BLOCKED_PATH_TRAVERSAL_OR_SANITIZED",
        )

        # 1.2 Absolute POSIX Path Traversal (/tmp/evil_absolute.txt)
        file_abs = slip_dir / "zip_slip_absolute.zip"
        abs_entries = [
            (b"/tmp/ttzip_security_pwned.txt", payload_bytes, 0),
            (b"/etc/ttzip_arbitrary_overwrite.conf", payload_bytes, 0),
        ]
        file_abs.write_bytes(self._build_raw_zip(abs_entries))
        self.register_scenario(
            name="zip_slip_absolute_path_traversal",
            target_sdk="all",
            attack_payload=file_abs.name,
            expected_outcome="BLOCKED_ABSOLUTE_PATH_EXTRACTION",
        )

        # 1.3 Windows Path Traversal (..\..\evil_win.txt & C:\evil.txt)
        file_win = slip_dir / "zip_slip_windows.zip"
        win_entries = [
            (b"..\\..\\evil_win.txt", payload_bytes, 0),
            (b"C:\\Windows\\System32\\evil_win_drive.dll", payload_bytes, 0),
        ]
        file_win.write_bytes(self._build_raw_zip(win_entries))
        self.register_scenario(
            name="zip_slip_windows_backslash_traversal",
            target_sdk="all",
            attack_payload=file_win.name,
            expected_outcome="BLOCKED_WINDOWS_TRAVERSAL_SEPARATORS",
        )

        # 1.4 TAR Archive Traversal (../../evil_tar.txt)
        file_tar = slip_dir / "zip_slip_tar.tar"
        tar_buf = io.BytesIO()
        with tarfile.open(fileobj=tar_buf, mode="w") as tar:
            # Entry 1: ../../evil_tar.txt
            tarinfo1 = tarfile.TarInfo(name="../../evil_tar.txt")
            tarinfo1.size = len(payload_bytes)
            tar.addfile(tarinfo1, io.BytesIO(payload_bytes))

            # Entry 2: /tmp/evil_tar_abs.txt
            tarinfo2 = tarfile.TarInfo(name="/tmp/evil_tar_abs.txt")
            tarinfo2.size = len(payload_bytes)
            tar.addfile(tarinfo2, io.BytesIO(payload_bytes))
        file_tar.write_bytes(tar_buf.getvalue())
        self.register_scenario(
            name="tar_slip_directory_traversal",
            target_sdk="all",
            attack_payload=file_tar.name,
            expected_outcome="BLOCKED_TAR_PATH_TRAVERSAL",
        )

    # -------------------------------------------------------------------------
    # 2. High Compression Ratio Bomb Descriptors & Samples
    # -------------------------------------------------------------------------
    def generate_zip_bomb_fixtures(self) -> None:
        """Generates high compression ratio expansion bomb fixtures."""
        self.log("Generating Zip Bomb / decompression quota attack fixtures...")
        bomb_dir = self.output_dir / "zip_bomb"
        bomb_dir.mkdir(parents=True, exist_ok=True)

        # 2.1 Single Flat Zero-Stream Bomb (100 MB repetitive zeros compressed to < 100 KB)
        uncompressed_size = 100 * 1024 * 1024  # 100 MB
        chunk_size = 1024 * 1024               # 1 MB chunk of zeros
        zeros_chunk = b"\x00" * chunk_size

        comp_obj = zlib.compressobj(level=9, method=zlib.DEFLATED, wbits=-15)
        comp_stream = bytearray()
        for _ in range(100):
            comp_stream.extend(comp_obj.compress(zeros_chunk))
        comp_stream.extend(comp_obj.flush())

        flat_bomb_file = bomb_dir / "zip_bomb_flat_100mb.zip"
        # Construct ZIP manually with huge uncompressed size
        filename = b"zero_bomb_100mb.bin"
        crc = 0x00000000

        local_hdr = struct.pack(
            "<4sHHHHHIIIHH",
            b"PK\x03\x04",
            20, 0, 8, 0, 0,
            crc,
            len(comp_stream),
            uncompressed_size,
            len(filename), 0,
        ) + filename + bytes(comp_stream)

        cd_record = struct.pack(
            "<4sHHHHHHIIIHHHHHII",
            b"PK\x01\x02",
            20, 20, 0, 8, 0, 0,
            crc,
            len(comp_stream),
            uncompressed_size,
            len(filename), 0, 0, 0, 0, 0,
            0,
        ) + filename

        eocd = struct.pack(
            "<4sHHHHIIH",
            b"PK\x05\x06",
            0, 0, 1, 1,
            len(cd_record),
            len(local_hdr),
            0,
        )

        flat_bomb_bytes = local_hdr + cd_record + eocd
        flat_bomb_file.write_bytes(flat_bomb_bytes)
        ratio = uncompressed_size / max(1, len(flat_bomb_bytes))

        self.register_scenario(
            name="zip_bomb_flat_expansion_ratio_overflow",
            target_sdk="all",
            attack_payload=flat_bomb_file.name,
            ratio=round(ratio, 2),
            expected_outcome="REJECTED_MAX_EXPANSION_RATIO_EXCEEDED",
        )

        # 2.2 Overlapping Central Directory Amplification Bomb (100 overlapping entries)
        overlap_bomb_file = bomb_dir / "zip_bomb_overlap_multi_entry.zip"
        cd_multi = bytearray()
        num_entries = 100
        for i in range(num_entries):
            fname = f"overlap_entry_{i:03d}.bin".encode("ascii")
            cd_rec = struct.pack(
                "<4sHHHHHHIIIHHHHHII",
                b"PK\x01\x02",
                20, 20, 0, 8, 0, 0,
                crc,
                len(comp_stream),
                uncompressed_size,
                len(fname), 0, 0, 0, 0, 0,
                0,  # All point to offset 0 (same local header)
            ) + fname
            cd_multi.extend(cd_rec)

        eocd_multi = struct.pack(
            "<4sHHHHIIH",
            b"PK\x05\x06",
            0, 0, num_entries, num_entries,
            len(cd_multi),
            len(local_hdr),
            0,
        )

        overlap_bomb_bytes = local_hdr + cd_multi + eocd_multi
        overlap_bomb_file.write_bytes(overlap_bomb_bytes)
        total_uncompressed = uncompressed_size * num_entries  # 10 GB
        overlap_ratio = total_uncompressed / max(1, len(overlap_bomb_bytes))

        self.register_scenario(
            name="zip_bomb_overlapping_central_directory",
            target_sdk="all",
            attack_payload=overlap_bomb_file.name,
            ratio=round(overlap_ratio, 2),
            expected_outcome="REJECTED_MEMORY_QUOTA_EXCEEDED",
        )

    # -------------------------------------------------------------------------
    # 3. Truncated & Corrupted Header Samples
    # -------------------------------------------------------------------------
    def generate_corrupted_stream_fixtures(self) -> None:
        """Generates malformed, corrupted, and truncated stream samples."""
        self.log("Generating corrupted / truncated stream fixtures...")
        corrupt_dir = self.output_dir / "corrupted_streams"
        corrupt_dir.mkdir(parents=True, exist_ok=True)

        # Baseline valid sample to corrupt
        baseline_zip = self._build_raw_zip([(b"baseline.txt", b"Baseline sample for corruption testing", 0)])

        # 3.1 Truncated Central Directory in ZIP
        trunc_cd_file = corrupt_dir / "corrupt_truncated_central_dir.zip"
        # Cut 20 bytes off before EOCD
        trunc_cd_file.write_bytes(baseline_zip[:-30])
        self.register_scenario(
            name="corrupt_truncated_central_directory",
            target_sdk="all",
            attack_payload=trunc_cd_file.name,
            expected_outcome="SAFE_DECOMPRESSION_ERROR_NO_CRASH",
        )

        # 3.2 CRC-32 Mismatch in ZIP
        crc_bad_file = corrupt_dir / "corrupt_crc_mismatch.zip"
        # Modify a payload byte
        bad_zip = bytearray(baseline_zip)
        # Flip a bit in the data section
        if len(bad_zip) > 35:
            bad_zip[35] ^= 0xFF
        crc_bad_file.write_bytes(bad_zip)
        self.register_scenario(
            name="corrupt_crc32_checksum_mismatch",
            target_sdk="all",
            attack_payload=crc_bad_file.name,
            expected_outcome="ERROR_CRC_CHECKSUM_FAILED",
        )

        # 3.3 Invalid Magic Signature (Random binary header)
        invalid_magic_file = corrupt_dir / "corrupt_invalid_magic.zip"
        invalid_magic_file.write_bytes(b"\xDE\xAD\xBE\xEF\xCA\xFE\xBA\xBE" + b"\x00" * 64)
        self.register_scenario(
            name="corrupt_invalid_magic_signature",
            target_sdk="all",
            attack_payload=invalid_magic_file.name,
            expected_outcome="ERROR_UNSUPPORTED_OR_CORRUPT_ARCHIVE_FORMAT",
        )

        # 3.4 Truncated TAR Archive (Cut mid-header block at 128 bytes)
        tar_trunc_file = corrupt_dir / "corrupt_truncated_tar.tar"
        tar_buf = io.BytesIO()
        with tarfile.open(fileobj=tar_buf, mode="w") as tar:
            data = b"Sample text for tar corruption" * 10
            tinfo = tarfile.TarInfo(name="sample.txt")
            tinfo.size = len(data)
            tar.addfile(tinfo, io.BytesIO(data))
        valid_tar = tar_buf.getvalue()
        tar_trunc_file.write_bytes(valid_tar[:128])  # Cut midway into 512-byte header
        self.register_scenario(
            name="corrupt_truncated_tar_header",
            target_sdk="all",
            attack_payload=tar_trunc_file.name,
            expected_outcome="ERROR_UNEXPECTED_EOF_SAFE_RETURN",
        )

        # 3.5 Truncated GZIP Stream (Missing 8-byte trailer)
        gzip_trunc_file = corrupt_dir / "corrupt_truncated_gzip.gz"
        raw_gzip = zlib.compress(b"Sample payload for gzip test", wbits=31)
        gzip_trunc_file.write_bytes(raw_gzip[:-6])  # Strip part of trailer
        self.register_scenario(
            name="corrupt_truncated_gzip_stream",
            target_sdk="all",
            attack_payload=gzip_trunc_file.name,
            expected_outcome="ERROR_CORRUPT_GZIP_TRAILER_SAFE_RETURN",
        )

        # 3.6 Truncated Zstandard Stream (Zstandard magic only)
        zstd_trunc_file = corrupt_dir / "corrupt_truncated_zstd.zst"
        zstd_magic = b"\x28\xB5\x2F\xFD"  # Standard Zstandard frame magic
        zstd_trunc_file.write_bytes(zstd_magic + b"\x00\x01\x02")
        self.register_scenario(
            name="corrupt_truncated_zstandard_frame",
            target_sdk="all",
            attack_payload=zstd_trunc_file.name,
            expected_outcome="ERROR_CORRUPT_ZSTD_FRAME_SAFE_RETURN",
        )

    # -------------------------------------------------------------------------
    # Orchestration & Manifest Export
    # -------------------------------------------------------------------------
    def generate_all(self) -> Dict[str, Any]:
        """Generates all malicious fixtures and exports contract JSON manifest."""
        self.clean()
        self.generate_zip_slip_fixtures()
        self.generate_zip_bomb_fixtures()
        self.generate_corrupted_stream_fixtures()

        manifest = {
            "scenarios": self.scenarios,
        }

        # Write manifest to output directory as well as default manifest path
        out_manifest_1 = self.output_dir / "security_fixtures_manifest.json"
        out_manifest_1.write_text(json.dumps(manifest, indent=2, ensure_ascii=False), encoding="utf-8")

        DEFAULT_MANIFEST_PATH.parent.mkdir(parents=True, exist_ok=True)
        DEFAULT_MANIFEST_PATH.write_text(json.dumps(manifest, indent=2, ensure_ascii=False), encoding="utf-8")

        self.log(f"Generated {len(self.scenarios)} security attack scenarios.")
        return manifest


def create_zip_slip_zip(output_path: Path) -> Path:
    """Helper to generate a zip slip zip file at output_path."""
    output_path.parent.mkdir(parents=True, exist_ok=True)
    payload_bytes = b"TTZIP_SECURITY_GATE_TRAVERSAL_PAYLOAD_UNAUTHORIZED_OVERWRITE\n"
    rel_entries = [
        (b"../../evil_slip.txt", payload_bytes, 0),
        (b"nested/dir/../../../../escape_depth4.txt", payload_bytes, 0),
        (b"valid_file.txt", b"Valid Safe File", 0),
    ]
    raw_zip = MaliciousFixturesGenerator._build_raw_zip(rel_entries)
    output_path.write_bytes(raw_zip)
    return output_path


def create_zip_slip_tar(output_path: Path) -> Path:
    """Helper to generate a zip slip tar file at output_path."""
    output_path.parent.mkdir(parents=True, exist_ok=True)
    payload_bytes = b"TTZIP_SECURITY_GATE_TRAVERSAL_PAYLOAD_UNAUTHORIZED_OVERWRITE\n"
    tar_buf = io.BytesIO()
    with tarfile.open(fileobj=tar_buf, mode="w") as tar:
        tarinfo1 = tarfile.TarInfo(name="../../evil_tar_slip.txt")
        tarinfo1.size = len(payload_bytes)
        tar.addfile(tarinfo1, io.BytesIO(payload_bytes))
    output_path.write_bytes(tar_buf.getvalue())
    return output_path


def create_zip_bomb(output_path: Path, uncompressed_mb: int = 100) -> Tuple[Path, float]:
    """Helper to generate a zip bomb at output_path."""
    output_path.parent.mkdir(parents=True, exist_ok=True)
    uncompressed_size = uncompressed_mb * 1024 * 1024
    zeros_chunk = b"\x00" * (1024 * 1024)
    comp_obj = zlib.compressobj(level=9, method=zlib.DEFLATED, wbits=-15)
    comp_stream = bytearray()
    for _ in range(uncompressed_mb):
        comp_stream.extend(comp_obj.compress(zeros_chunk))
    comp_stream.extend(comp_obj.flush())

    filename = b"zero_bomb_payload.bin"
    crc = 0x00000000
    local_hdr = struct.pack(
        "<4sHHHHHIIIHH",
        b"PK\x03\x04", 20, 0, 8, 0, 0, crc,
        len(comp_stream), uncompressed_size, len(filename), 0,
    ) + filename + bytes(comp_stream)

    cd_record = struct.pack(
        "<4sHHHHHHIIIHHHHHII",
        b"PK\x01\x02", 20, 20, 0, 8, 0, 0, crc,
        len(comp_stream), uncompressed_size, len(filename), 0, 0, 0, 0, 0, 0,
    ) + filename

    eocd = struct.pack(
        "<4sHHHHIIH",
        b"PK\x05\x06", 0, 0, 1, 1, len(cd_record), len(local_hdr), 0,
    )
    raw = local_hdr + cd_record + eocd
    output_path.write_bytes(raw)
    ratio = float(uncompressed_size) / max(1.0, float(len(raw)))
    return output_path, ratio


def generate_malicious_fixtures(
    output_dir: Optional[Path] = None,
    clean: bool = True,
    verbose: bool = False,
) -> Dict[str, Any]:
    """Programmatic API to generate malicious test fixtures."""
    out = output_dir or DEFAULT_FIXTURES_DIR
    generator = MaliciousFixturesGenerator(output_dir=out, verbose=verbose)
    if clean:
        generator.clean()
    return generator.generate_all()


def main() -> int:
    parser = argparse.ArgumentParser(
        description="TTZip Malicious & Adversarial Fixture Generator",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "-o", "--output-dir",
        type=Path,
        default=DEFAULT_FIXTURES_DIR,
        help="Destination directory for malicious fixtures",
    )
    parser.add_argument(
        "--clean",
        action="store_true",
        default=True,
        help="Clean target directory before generation",
    )
    parser.add_argument(
        "-v", "--verbose",
        action="store_true",
        help="Enable verbose logging",
    )

    args = parser.parse_args()
    print(f"🛡️ Generating TTZip malicious security test fixtures in: {args.output_dir}")
    generator = MaliciousFixturesGenerator(output_dir=args.output_dir, verbose=args.verbose)
    manifest = generator.generate_all()
    print(f"✅ Security test fixture generation complete ({len(manifest['scenarios'])} scenarios).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
