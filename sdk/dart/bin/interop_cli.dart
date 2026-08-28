// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Dart & Flutter.
// Dart FFI Headless Interop CLI Runner.

import 'dart:io';
import '../lib/ttzip.dart';

TTZipFormat parseFormat(String fmtStr) {
  switch (fmtStr.toLowerCase()) {
    case 'zip':
      return TTZipFormat.zip;
    case '7z':
    case '7zip':
    case 'sevenzip':
      return TTZipFormat.sevenZip;
    case 'tar':
      return TTZipFormat.tar;
    case 'tar.gz':
    case 'targz':
    case 'tgz':
    case 'gz':
      return TTZipFormat.tarGz;
    case 'tar.bz2':
    case 'tarbz2':
    case 'tbz2':
    case 'bz2':
      return TTZipFormat.tarBz2;
    case 'tar.xz':
    case 'tarxz':
    case 'txz':
    case 'xz':
      return TTZipFormat.tarXz;
    case 'tar.zst':
    case 'tarzst':
    case 'tar.zstd':
    case 'zst':
      return TTZipFormat.tarZstd;
    default:
      return TTZipFormat.zip;
  }
}

void printUsage(String prog) {
  stderr.writeln('Usage:');
  stderr.writeln('  $prog --create <format> <src> <dst> [--password <pwd>]');
  stderr.writeln('  $prog --extract <src> <dst> [--password <pwd>]');
  stderr.writeln('  $prog --version');
}

Future<void> main(List<String> args) async {
  if (args.isEmpty) {
    printUsage('interop_cli.dart');
    exit(2);
  }

  if (args[0] == '--version') {
    stdout.writeln(TTZip.version);
    exit(0);
  }

  String? mode;
  String? formatStr;
  String? src;
  String? dst;
  String? password;

  var i = 0;
  while (i < args.length) {
    final arg = args[i];
    switch (arg) {
      case '--create':
        mode = 'create';
        if (i + 3 >= args.length) {
          stderr.writeln('Error: --create requires <format> <src> <dst>');
          exit(2);
        }
        formatStr = args[i + 1];
        src = args[i + 2];
        dst = args[i + 3];
        i += 4;
        break;
      case '--extract':
        mode = 'extract';
        if (i + 2 >= args.length) {
          stderr.writeln('Error: --extract requires <src> <dst>');
          exit(2);
        }
        src = args[i + 1];
        dst = args[i + 2];
        i += 3;
        break;
      case '--password':
        if (i + 1 >= args.length) {
          stderr.writeln('Error: --password requires an argument');
          exit(2);
        }
        password = args[i + 1];
        i += 2;
        break;
      default:
        stderr.writeln('Unknown argument: $arg');
        printUsage('interop_cli.dart');
        exit(2);
    }
  }

  if (mode == null) {
    printUsage('interop_cli.dart');
    exit(2);
  }

  try {
    if (mode == 'create') {
      final format = parseFormat(formatStr ?? 'zip');
      await TTZip.compress(
        sources: [src!],
        destination: dst!,
        format: format,
        level: TTZipCompressionLevel.normal,
        password: password,
      );
      exit(0);
    } else if (mode == 'extract') {
      await TTZip.extract(
        archivePath: src!,
        destination: dst!,
        password: password,
      );
      exit(0);
    }
  } catch (e, st) {
    stderr.writeln('Error: $e');
    stderr.writeln(st);
    exit(1);
  }

  exit(2);
}
