// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
import 'dart:typed_data';
import 'package:ttzip/ttzip.dart';

void main() async {
  print('⚡️ TTZip Dart & Flutter Example (v${TTZip.version})');

  final data = Uint8List.fromList('Flutter Cross-Platform Storage'.codeUnits);
  final crc = TTZip.crc32(data);
  print('CRC-32: 0x${crc.toRadixString(16).toUpperCase()}');

  await TTZip.compress(
    sources: ['pubspec.yaml'],
    destination: 'flutter_demo.zip',
    level: TTZipCompressionLevel.normal,
  );
  print('Archive successfully created with Dart SDK.');
}
