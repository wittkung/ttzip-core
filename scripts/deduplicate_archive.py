#!/usr/bin/env python3
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

"""Static archive bitcode stripper and 3-pass duplicate member/symbol deduplicator."""

from __future__ import annotations

from collections import Counter
import glob
import os
import re
import shutil
import subprocess
import sys
import tempfile
from typing import Dict, List, Optional, Set, Tuple

HEX_ADDR_RE = re.compile(r"^[0-9a-fA-F]+$")


def find_llvm_objcopy() -> Optional[str]:
    direct = shutil.which("llvm-objcopy")
    if direct:
        return direct
    sysroot = subprocess.run(
        ["rustc", "--print", "sysroot"], capture_output=True, text=True, check=False
    ).stdout.strip()
    vv_out = subprocess.run(
        ["rustc", "-vV"], capture_output=True, text=True, check=False
    ).stdout
    host = ""
    for line in vv_out.splitlines():
        if line.startswith("host:"):
            host = line.split(":", 1)[1].strip()
            break
    if sysroot and host:
        candidate = os.path.join(sysroot, "lib", "rustlib", host, "bin", "llvm-objcopy")
        if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            return candidate
    rustup_home = os.environ.get("RUSTUP_HOME") or os.path.expanduser("~/.rustup")
    pattern = os.path.join(rustup_home, "toolchains", "*", "lib", "rustlib", "*", "bin", "llvm-objcopy")
    for candidate in sorted(glob.glob(pattern)):
        if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            return candidate
    return None


def strip_embedded_bitcode(archive: str) -> None:
    objcopy = find_llvm_objcopy()
    if not objcopy:
        print(f"--> [WARN] llvm-objcopy not found; keeping the embedded bitcode in {os.path.basename(archive)}")
        return
    subprocess.run(
        [objcopy, "--remove-section=__LLVM,__bitcode", archive],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )


def _defined_global_symbols(object_path: str) -> Set[str]:
    proc = subprocess.run(
        ["nm", "-g", object_path], capture_output=True, text=True, check=False
    )
    symbols: Set[str] = set()
    for line in (proc.stdout or "").splitlines():
        parts = line.split()
        if len(parts) >= 3 and HEX_ADDR_RE.match(parts[0]) and parts[1] != "U":
            symbols.add(parts[-1])
    return symbols


def _deduplicate_member_group(
    probe_archive: str, archive: str, workdir: str, group: int, name: str
) -> None:
    members_out = subprocess.run(
        ["ar", "-t", probe_archive], capture_output=True, text=True, check=False
    ).stdout.splitlines()
    total = sum(1 for m in members_out if m == name)
    if total <= 1:
        return
    kept_blobs: List[bytes] = []
    kept_paths: List[str] = []
    kept_symbols: Set[str] = set()
    for index in range(1, total + 1):
        member_path = os.path.join(workdir, f"member_{group}_{index}.o")
        blob = subprocess.run(
            ["ar", "-p", probe_archive, name], capture_output=True, check=False
        ).stdout
        with open(member_path, "wb") as f:
            f.write(blob)
        subprocess.run(
            ["ar", "-d", probe_archive, name],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if not kept_paths:
            kept_blobs.append(blob)
            kept_paths.append(member_path)
            kept_symbols |= _defined_global_symbols(member_path)
            continue
        if any(blob == existing for existing in kept_blobs):
            continue
        member_symbols = _defined_global_symbols(member_path)
        new_symbols = member_symbols - kept_symbols
        if not new_symbols:
            continue
        kept_blobs.append(blob)
        kept_paths.append(member_path)
        kept_symbols |= new_symbols

    for _ in range(total):
        res = subprocess.run(
            ["ar", "-d", archive, name],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if res.returncode != 0:
            break

    readd_dir = os.path.join(workdir, "readd")
    os.makedirs(readd_dir, exist_ok=True)
    for kept_index, kept_path in enumerate(kept_paths, start=1):
        target_name = name if kept_index == 1 else f"{name}.{kept_index}"
        target_path = os.path.join(readd_dir, target_name)
        shutil.copyfile(kept_path, target_path)
        subprocess.run(
            ["ar", "-r", archive, target_path],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )


def _pass1_deduplicate_member_names(archive: str, workdir: str) -> bool:
    members = subprocess.run(
        ["ar", "-t", archive], capture_output=True, text=True, check=False
    ).stdout.splitlines()
    counts = Counter(m for m in members if m)
    duplicates = sorted(name for name, count in counts.items() if count > 1)
    if not duplicates:
        return False
    print(f"--> [INFO] Removing duplicate members from {os.path.basename(archive)}")
    probe_archive = os.path.join(workdir, "probe.a")
    shutil.copyfile(archive, probe_archive)
    for group, name in enumerate(duplicates, start=1):
        _deduplicate_member_group(probe_archive, archive, workdir, group, name)
    return True


def _parse_archive_member_symbols(
    nm_output: str,
) -> Tuple[List[str], Dict[str, List[str]], Dict[str, str]]:
    order: List[str] = []
    member_syms: Dict[str, List[str]] = {}
    first_owner: Dict[str, str] = {}
    seen: Set[Tuple[str, str]] = set()
    current_member = ""
    for line in nm_output.splitlines():
        if line and not line[0].isspace() and line.endswith(":"):
            current_member = line[:-1]
            order.append(current_member)
            continue
        parts = line.split()
        if current_member and len(parts) >= 3 and HEX_ADDR_RE.match(parts[0]) and parts[1] != "U":
            sym = parts[-1]
            if (current_member, sym) not in seen:
                seen.add((current_member, sym))
                member_syms.setdefault(current_member, []).append(sym)
                first_owner.setdefault(sym, current_member)
    return order, member_syms, first_owner


def _find_droppable_members(archive: str) -> List[str]:
    proc = subprocess.run(
        ["nm", "-g", archive], capture_output=True, text=True, check=False
    )
    order, member_syms, first_owner = _parse_archive_member_symbols(proc.stdout or "")
    droppable: List[str] = []
    for member in order:
        syms = member_syms.get(member)
        if not syms:
            continue
        if all(first_owner.get(sym) != member for sym in syms):
            droppable.append(member)
    return droppable


def _pass2_drop_redundant_definers(archive: str) -> None:
    for _ in range(10):
        droppable = _find_droppable_members(archive)
        if not droppable:
            break
        print(f"--> [INFO] Dropping {len(droppable)} redundant members from {os.path.basename(archive)}")
        for member in droppable:
            subprocess.run(
                ["ar", "-d", archive, member],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )


def _pass3_weaken_duplicate_symbols(archive: str) -> None:
    proc = subprocess.run(
        ["nm", "-g", archive], capture_output=True, text=True, check=False
    )
    defined_counts: Counter[str] = Counter()
    for line in (proc.stdout or "").splitlines():
        if line and not line[0].isspace() and line.endswith(":"):
            continue
        parts = line.split()
        if len(parts) >= 3 and HEX_ADDR_RE.match(parts[0]) and parts[1] != "U":
            defined_counts[parts[-1]] += 1
    duplicate_symbols = sorted(sym for sym, count in defined_counts.items() if count > 1)
    if not duplicate_symbols:
        return
    objcopy = find_llvm_objcopy()
    if not objcopy:
        return
    weaken_args = [f"--weaken-symbol={sym}" for sym in duplicate_symbols]
    print(f"--> [INFO] Weakening {len(duplicate_symbols)} duplicated symbols in {os.path.basename(archive)}")
    subprocess.run(
        [objcopy, *weaken_args, archive],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )


def deduplicate_archive(archive: str) -> None:
    workdir = tempfile.mkdtemp(prefix="ttzip-dedup-", dir="/tmp")
    try:
        if not _pass1_deduplicate_member_names(archive, workdir):
            return
        _pass2_drop_redundant_definers(archive)
        _pass3_weaken_duplicate_symbols(archive)
        subprocess.run(
            ["ranlib", "-s", archive],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
    finally:
        shutil.rmtree(workdir, ignore_errors=True)


def main(argv: List[str]) -> int:
    if len(argv) < 3:
        print(f"Usage: {argv[0]} <strip-bitcode|deduplicate> <archive>", file=sys.stderr)
        return 1
    mode, archive = argv[1], argv[2]
    if mode == "strip-bitcode":
        strip_embedded_bitcode(archive)
        return 0
    if mode == "deduplicate":
        deduplicate_archive(archive)
        return 0
    print(f"Unknown mode: {mode}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
