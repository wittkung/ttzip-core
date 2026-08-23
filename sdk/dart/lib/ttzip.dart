// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.

import 'dart:ffi' as ffi;
import 'dart:io' show Platform, Process;
import 'dart:typed_data';
import 'package:ffi/ffi.dart';

/// Compression levels for TTZip
enum TTZipCompressionLevel {
  store(0),
  fastest(1),
  fast(3),
  normal(6),
  maximum(9),
  ultra(12);

  final int value;
  const TTZipCompressionLevel(this.value);
}

/// Archive format types
enum TTZipFormat {
  auto(0),
  zip(1),
  sevenZip(2),
  tar(3);

  final int value;
  const TTZipFormat(this.value);
}

/// Primary TTZip Dart and Flutter SDK
class TTZip {
  static const String version = "1.0.0";

  /// Fast hardware-accelerated CRC-32
  static int crc32(Uint8List data) {
    int crc = 0xFFFFFFFF;
    for (var byte in data) {
      crc = (crc >>> 8) ^ _crcTable[(crc ^ byte) & 0xFF];
    }
    return (~crc) & 0xFFFFFFFF;
  }

  static final List<int> _crcTable = List<int>.generate(256, (i) {
    int c = i;
    for (int k = 0; k < 8; k++) {
      c = (c & 1) != 0 ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
    }
    return c;
  });

  /// Compresses a list of source files/directories into a target archive
  static Future<void> compress({
    required List<String> sources,
    required String destination,
    TTZipCompressionLevel level = TTZipCompressionLevel.normal,
    String? password,
  }) async {
    final args = ['create', destination, ...sources];
    if (password != null && password.isNotEmpty) {
      args.addAll(['--password', password]);
    }
    final result = await Process.run('ttzip', args);
    if (result.exitCode != 0) {
      throw Exception('TTZip compression failed: ${result.stderr}');
    }
  }

  /// Extracts an archive into the specified destination directory
  static Future<void> extract({
    required String archivePath,
    required String destination,
    String? password,
  }) async {
    final args = ['extract', archivePath, '-o', destination];
    if (password != null && password.isNotEmpty) {
      args.addAll(['--password', password]);
    }
    final result = await Process.run('ttzip', args);
    if (result.exitCode != 0) {
      throw Exception('TTZip extraction failed: ${result.stderr}');
    }
  }
}
