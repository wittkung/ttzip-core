#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
"""
TTZip Python SDK Quickstart Example.
"""

import ttzip

print(f"⚡️ TTZip Python SDK (v{ttzip.__version__})")
print(f"Hardware Acceleration: {ttzip.is_hardware_accelerated()}")

# Compute CRC32
payload = b"High-Performance Python Data Stream"
crc = ttzip.crc32(payload)
print(f"CRC-32: 0x{crc:08X}")

# Compression & Inspection
ttzip.compress(["setup.py"], "dist_demo.zip", level=6)
entries = ttzip.inspect("dist_demo.zip")
print(f"Archived entries: {len(entries)}")
for entry in entries:
    print(f" - {entry['path']} ({entry['uncompressed_size']} bytes)")
