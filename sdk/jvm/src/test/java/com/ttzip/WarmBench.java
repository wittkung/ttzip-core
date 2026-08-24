// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

package com.ttzip;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Random;

public class WarmBench {
    public static void main(String[] args) throws Exception {
        Path tempDir = Files.createTempDirectory("ttzip_warm_java_");
        Path sampleFile = tempDir.resolve("sample.bin");
        Path outZip = tempDir.resolve("out.zip");
        Path extDir = tempDir.resolve("extracted");

        byte[] data = new byte[50 * 1024 * 1024];
        new Random(42).nextBytes(data);
        Files.write(sampleFile, data);

        // 1. Warmup (JIT compilation of Panama FFM DowncallHandle)
        for (int i = 0; i < 3; i++) {
            TTZip.compress(List.of(sampleFile.toString()), outZip.toString(), TTZip.ArchiveFormat.ZIP, TTZip.CompressionLevel.FASTEST, null, 0, null);
            TTZip.extract(outZip.toString(), extDir.toString());
        }

        // 2. Measure In-Process Warm Compression
        int N = 5;
        long t0 = System.nanoTime();
        for (int i = 0; i < N; i++) {
            TTZip.compress(List.of(sampleFile.toString()), outZip.toString(), TTZip.ArchiveFormat.ZIP, TTZip.CompressionLevel.FASTEST, null, 0, null);
        }
        long t1 = System.nanoTime();
        double compMbs = (50.0 * N) / ((t1 - t0) / 1_000_000_000.0);

        // 3. Measure In-Process Warm Extraction
        t0 = System.nanoTime();
        for (int i = 0; i < N; i++) {
            TTZip.extract(outZip.toString(), extDir.toString());
        }
        t1 = System.nanoTime();
        double extMbs = (50.0 * N) / ((t1 - t0) / 1_000_000_000.0);

        System.out.printf("Java 22+ Panama FFM In-Process Warm Compression: %.2f MB/s%n", compMbs);
        System.out.printf("Java 22+ Panama FFM In-Process Warm Extraction:  %.2f MB/s%n", extMbs);
    }
}
