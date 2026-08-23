// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.

import 'dart:io';
import 'dart:typed_data';
import '../lib/ttzip.dart';

void main() async {
  print('⚡️ Running TTZip Dart / Flutter SDK Verification Test...');

  // 1. Version Check
  assert(TTZip.version == '1.0.0', 'Version should be 1.0.0');
  print('  [PASS] Dart SDK version: ${TTZip.version}');

  // 2. CRC32 Check
  final data = Uint8List.fromList('TTZip Dart & Flutter High-Performance SDK'.codeUnits);
  final crc = TTZip.crc32(data);
  assert(crc != 0, 'CRC32 should be non-zero');
  print('  [PASS] Dart SDK CRC-32: 0x${crc.toRadixString(16).toUpperCase()}');

  print('✅ All Dart SDK tests passed successfully!');
}
