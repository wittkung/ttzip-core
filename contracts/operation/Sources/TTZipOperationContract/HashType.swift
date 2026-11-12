// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Supported cryptographic and verification hash algorithms.
public enum HashType: String, Sendable {
    case crc32 = "CRC32"
    case sha256 = "SHA-256"
    case md5 = "MD5"
    case sha1 = "SHA-1"
}
