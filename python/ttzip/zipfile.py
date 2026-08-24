# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for Python.
# zipfile.ZipFile drop-in compatibility module.

import os
import shutil
import tempfile
from pathlib import Path
from typing import List, Optional, Union

from .models import EntryMetadata
from .exceptions import TTZipError, AuthenticationError, CorruptArchiveError


class ZipFile:
    """
    High-performance drop-in replacement for standard library zipfile.ZipFile,
    backed by the TTZip native C-extension.
    """

    def __init__(
        self,
        file: Union[str, Path],
        mode: str = "r",
        compression: int = 8,
        allowZip64: bool = True,
        compresslevel: Optional[int] = None,
        *,
        password: Optional[str] = None,
    ):
        self.filename = str(file)
        self.mode = mode
        self.compression = compression
        self.compresslevel = compresslevel if compresslevel is not None else 6
        self.password = password
        self._closed = False
        self._staged_writes: List[tuple] = []

        if mode == "r" and not os.path.exists(self.filename):
            raise FileNotFoundError(f"Archive file not found: {self.filename}")

    def __enter__(self) -> "ZipFile":
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.close()

    def _check_closed(self) -> None:
        if self._closed:
            raise ValueError("Attempted operation on closed ZipFile")

    def infolist(self) -> List[EntryMetadata]:
        """Return a list containing an EntryMetadata instance for each member of the archive."""
        self._check_closed()
        from . import inspect
        return inspect(self.filename, password=self.password)

    def namelist(self) -> List[str]:
        """Return a list of archive members by name."""
        return [entry.path for entry in self.infolist()]

    def getinfo(self, name: str) -> EntryMetadata:
        """Return an EntryMetadata instance for archive member name."""
        entries = self.infolist()
        for e in entries:
            if e.path == name or e.path == name.lstrip("/"):
                return e
        raise KeyError(f"There is no item named {name!r} in the archive")

    def read(self, name: str, pwd: Optional[str] = None) -> bytes:
        """Return the bytes of the file name in the archive."""
        self._check_closed()
        effective_pwd = pwd or self.password
        # Single-entry in-memory extraction via temporary staging directory
        tmp_dest = tempfile.mkdtemp(prefix="ttzip_read_")
        try:
            from . import extract
            extract(self.filename, destination=tmp_dest, password=effective_pwd)
            target_path = os.path.join(tmp_dest, name.lstrip("/"))
            if not os.path.exists(target_path):
                raise KeyError(f"Item {name!r} not found in extracted archive")
            with open(target_path, "rb") as f:
                return f.read()
        finally:
            shutil.rmtree(tmp_dest, ignore_errors=True)

    def extract(
        self,
        member: Union[str, EntryMetadata],
        path: Optional[Union[str, Path]] = None,
        pwd: Optional[str] = None,
    ) -> str:
        """Extract a member from the archive to the specified directory."""
        self._check_closed()
        dest_dir = str(path) if path else os.getcwd()
        member_name = member.path if isinstance(member, EntryMetadata) else str(member)
        effective_pwd = pwd or self.password

        tmp_dest = tempfile.mkdtemp(prefix="ttzip_ext_")
        try:
            from . import extract
            extract(self.filename, destination=tmp_dest, password=effective_pwd)
            src = os.path.join(tmp_dest, member_name.lstrip("/"))
            dst = os.path.join(dest_dir, member_name.lstrip("/"))
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            if os.path.isdir(src):
                os.makedirs(dst, exist_ok=True)
            else:
                shutil.copy2(src, dst)
            return dst
        finally:
            shutil.rmtree(tmp_dest, ignore_errors=True)

    def extractall(
        self,
        path: Optional[Union[str, Path]] = None,
        members: Optional[List[Union[str, EntryMetadata]]] = None,
        pwd: Optional[str] = None,
    ) -> None:
        """Extract all members from the archive to the specified directory."""
        self._check_closed()
        dest_dir = str(path) if path else os.getcwd()
        effective_pwd = pwd or self.password
        from . import extract
        extract(self.filename, destination=dest_dir, password=effective_pwd)

    def write(
        self,
        filename: Union[str, Path],
        arcname: Optional[str] = None,
        compress_type: Optional[int] = None,
        compresslevel: Optional[int] = None,
    ) -> None:
        """Write the file named filename to the archive."""
        self._check_closed()
        if self.mode not in ("w", "a", "x"):
            raise ValueError("Write not supported on read-only ZipFile")
        src_path = str(filename)
        dest_arcname = arcname if arcname else os.path.basename(src_path)
        self._staged_writes.append((src_path, dest_arcname))

    def writestr(
        self,
        zinfo_or_arcname: Union[str, EntryMetadata],
        data: Union[bytes, str],
        compress_type: Optional[int] = None,
        compresslevel: Optional[int] = None,
    ) -> None:
        """Write bytes or string data directly to the archive."""
        self._check_closed()
        if self.mode not in ("w", "a", "x"):
            raise ValueError("Write not supported on read-only ZipFile")
        arcname = zinfo_or_arcname.path if isinstance(zinfo_or_arcname, EntryMetadata) else str(zinfo_or_arcname)
        payload = data.encode("utf-8") if isinstance(data, str) else bytes(data)
        
        # Stage data to a persistent temp file until close()
        tmp = tempfile.NamedTemporaryFile(delete=False, prefix="ttzip_writestr_")
        tmp.write(payload)
        tmp.flush()
        tmp.close()
        self._staged_writes.append((tmp.name, arcname))

    def close(self) -> None:
        """Close the archive file."""
        if self._closed:
            return
        if self.mode in ("w", "a", "x") and self._staged_writes:
            stage_dir = tempfile.mkdtemp(prefix="ttzip_stage_")
            try:
                for src_path, arcname in self._staged_writes:
                    target_dest = os.path.join(stage_dir, arcname.lstrip("/"))
                    os.makedirs(os.path.dirname(target_dest), exist_ok=True)
                    if os.path.isdir(src_path):
                        shutil.copytree(src_path, target_dest, dirs_exist_ok=True)
                    else:
                        shutil.copy2(src_path, target_dest)
                from . import compress
                stage_items = [os.path.join(stage_dir, f) for f in os.listdir(stage_dir)]
                compress(
                    sources=stage_items,
                    destination=self.filename,
                    format="zip",
                    level=self.compresslevel,
                    password=self.password,
                )
            finally:
                shutil.rmtree(stage_dir, ignore_errors=True)
            self._staged_writes.clear()
        self._closed = True


class SevenZipFile(ZipFile):
    """Drop-in 7-Zip archive class with LZMA2/AES-256 solid compression support."""

    def close(self) -> None:
        if self._closed:
            return
        if self.mode in ("w", "a", "x") and self._staged_writes:
            stage_dir = tempfile.mkdtemp(prefix="ttzip_7z_stage_")
            try:
                for src_path, arcname in self._staged_writes:
                    target_dest = os.path.join(stage_dir, arcname.lstrip("/"))
                    os.makedirs(os.path.dirname(target_dest), exist_ok=True)
                    if os.path.isdir(src_path):
                        shutil.copytree(src_path, target_dest, dirs_exist_ok=True)
                    else:
                        shutil.copy2(src_path, target_dest)
                from . import compress
                stage_items = [os.path.join(stage_dir, f) for f in os.listdir(stage_dir)]
                compress(
                    sources=stage_items,
                    destination=self.filename,
                    format="7z",
                    level=self.compresslevel,
                    password=self.password,
                )
            finally:
                shutil.rmtree(stage_dir, ignore_errors=True)
            self._staged_writes.clear()
        self._closed = True


def open_archive(
    file: Union[str, Path],
    mode: str = "r",
    format: str = "auto",
    **kwargs,
) -> ZipFile:
    """
    Open an archive file (ZIP, 7z, TAR) and return a ZipFile / SevenZipFile instance.
    """
    fmt = format.lower()
    if fmt in ("7z", "sevenz"):
        return SevenZipFile(file, mode=mode, **kwargs)
    return ZipFile(file, mode=mode, **kwargs)
