# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

try:
    from ._ttzip import (
        TTZipError,
        AuthenticationError,
        CorruptArchiveError,
        SecurityError,
    )
except ImportError:
    class TTZipError(Exception):
        """Base exception for all TTZip runtime and compression errors."""
        pass

    class AuthenticationError(TTZipError):
        """Raised when archive password is wrong or required but missing."""
        pass

    class CorruptArchiveError(TTZipError):
        """Raised when archive header is corrupted or checksum verification fails."""
        pass

    class SecurityError(TTZipError):
        """Raised when a security boundary violation occurs (e.g. Zip Slip path traversal)."""
        pass

__all__ = [
    "TTZipError",
    "AuthenticationError",
    "CorruptArchiveError",
    "SecurityError",
]
