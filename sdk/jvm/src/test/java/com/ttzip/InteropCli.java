// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Java 22+.
// Headless Panama FFM Interop CLI Runner.

package com.ttzip;

import java.util.List;

public final class InteropCli {

    private static TTZip.ArchiveFormat parseFormat(String fmtStr) {
        if (fmtStr == null) return TTZip.ArchiveFormat.ZIP;
        String s = fmtStr.toLowerCase();
        return switch (s) {
            case "zip" -> TTZip.ArchiveFormat.ZIP;
            case "7z", "7zip", "sevenzip" -> TTZip.ArchiveFormat.SEVEN_ZIP;
            case "tar" -> TTZip.ArchiveFormat.TAR;
            case "tar.gz", "targz", "tgz", "gz" -> TTZip.ArchiveFormat.TAR_GZ;
            case "tar.bz2", "tarbz2", "tbz2", "bz2" -> TTZip.ArchiveFormat.TAR_BZ2;
            case "tar.xz", "tarxz", "txz", "xz" -> TTZip.ArchiveFormat.TAR_XZ;
            case "tar.zst", "tarzst", "tar.zstd", "zst" -> TTZip.ArchiveFormat.TAR_ZSTD;
            default -> TTZip.ArchiveFormat.ZIP;
        };
    }

    private static void printUsage() {
        System.err.println("Usage:");
        System.err.println("  InteropCli --create <format> <src> <dst> [--password <pwd>]");
        System.err.println("  InteropCli --extract <src> <dst> [--password <pwd>]");
        System.err.println("  InteropCli --version");
    }

    public static void main(String[] args) {
        if (args.length < 1) {
            printUsage();
            System.exit(2);
        }

        if (args[0].equals("--version")) {
            System.out.println(TTZip.version());
            System.exit(0);
        }

        String mode = null;
        String formatStr = null;
        String src = null;
        String dst = null;
        String password = null;

        int i = 0;
        while (i < args.length) {
            String arg = args[i];
            switch (arg) {
                case "--create" -> {
                    mode = "create";
                    if (i + 3 >= args.length) {
                        System.err.println("Error: --create requires <format> <src> <dst>");
                        System.exit(2);
                    }
                    formatStr = args[i + 1];
                    src = args[i + 2];
                    dst = args[i + 3];
                    i += 4;
                }
                case "--extract" -> {
                    mode = "extract";
                    if (i + 2 >= args.length) {
                        System.err.println("Error: --extract requires <src> <dst>");
                        System.exit(2);
                    }
                    src = args[i + 1];
                    dst = args[i + 2];
                    i += 3;
                }
                case "--password" -> {
                    if (i + 1 >= args.length) {
                        System.err.println("Error: --password requires an argument");
                        System.exit(2);
                    }
                    password = args[i + 1];
                    i += 2;
                }
                default -> {
                    System.err.println("Unknown argument: " + arg);
                    printUsage();
                    System.exit(2);
                }
            }
        }

        if (mode == null) {
            printUsage();
            System.exit(2);
        }

        try {
            if ("create".equals(mode)) {
                TTZip.ArchiveFormat fmt = parseFormat(formatStr);
                TTZip.compress(
                    List.of(src),
                    dst,
                    fmt,
                    TTZip.CompressionLevel.NORMAL,
                    password,
                    0,
                    null
                );
                System.exit(0);
            } else if ("extract".equals(mode)) {
                TTZip.extract(src, dst, password, 0, null);
                System.exit(0);
            }
        } catch (Throwable t) {
            System.err.println("Error: " + t.getMessage());
            t.printStackTrace(System.err);
            System.exit(1);
        }

        System.exit(2);
    }
}
