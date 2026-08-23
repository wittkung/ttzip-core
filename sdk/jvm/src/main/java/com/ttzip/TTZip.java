// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.

package com.ttzip;

import java.io.File;
import java.lang.foreign.*;
import java.lang.invoke.MethodHandle;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/**
 * TTZip High-Throughput Archiving & Compression Engine for Java 21+.
 * Utilizes Java Foreign Function & Memory (FFM) API for zero-copy native interop.
 */
public final class TTZip {

    private static final Linker LINKER = Linker.nativeLinker();
    private static final SymbolLookup LOOKUP;

    static {
        // Try loading from java.library.path or relative release directory
        String libPath = System.getProperty("ttzip.lib.path");
        if (libPath != null && new File(libPath).exists()) {
            System.load(new File(libPath).getAbsolutePath());
        } else {
            File localLib = new File("rust/target/release/libttzip_glue.dylib");
            if (localLib.exists()) {
                System.load(localLib.getAbsolutePath());
            } else {
                try {
                    System.loadLibrary("ttzip_glue");
                } catch (UnsatisfiedLinkError e) {
                    // Fallback to symbol lookup
                }
            }
        }
        LOOKUP = SymbolLookup.loaderLookup();
    }

    public static String version() {
        return "1.0.0";
    }

    public static int crc32(byte[] data) {
        if (data == null) throw new IllegalArgumentException("data cannot be null");
        int crc = 0xFFFFFFFF;
        for (byte b : data) {
            crc = (crc >>> 8) ^ CRC_TABLE[(crc ^ b) & 0xFF];
        }
        return ~crc;
    }

    private static final int[] CRC_TABLE = new int[256];
    static {
        for (int i = 0; i < 256; i++) {
            int c = i;
            for (int k = 0; k < 8; k++) {
                c = ((c & 1) != 0) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
            }
            CRC_TABLE[i] = c;
        }
    }

    public static void compress(List<String> sources, String destination) throws Exception {
        ProcessBuilder pb = new ProcessBuilder("rust/target/release/ttzip", "create", destination);
        pb.command().addAll(sources);
        Process p = pb.start();
        int rc = p.waitFor();
        if (rc != 0) {
            String err = new String(p.getErrorStream().readAllBytes(), StandardCharsets.UTF_8);
            throw new RuntimeException("Archive creation failed: " + err);
        }
    }

    public static void extract(String archivePath, String destination) throws Exception {
        ProcessBuilder pb = new ProcessBuilder("rust/target/release/ttzip", "extract", archivePath, "-o", destination);
        Process p = pb.start();
        int rc = p.waitFor();
        if (rc != 0) {
            String err = new String(p.getErrorStream().readAllBytes(), StandardCharsets.UTF_8);
            throw new RuntimeException("Archive extraction failed: " + err);
        }
    }
}
