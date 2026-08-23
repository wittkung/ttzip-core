// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.

package com.ttzip;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

public class TTZipTest {

    public static void main(String[] args) throws Exception {
        System.out.println("⚡️ Running TTZip Java 21+ SDK Test Suite...");

        // 1. Version Check
        String v = TTZip.version();
        assert v.equals("1.0.0") : "Version must be 1.0.0";
        System.out.println("  [PASS] Java SDK version: " + v);

        // 2. CRC-32 Check
        byte[] payload = "TTZip High-Throughput Java SDK Test".getBytes();
        int crc = TTZip.crc32(payload);
        assert crc != 0 : "CRC32 cannot be 0";
        System.out.println("  [PASS] Java SDK CRC-32: 0x" + Integer.toHexString(crc).toUpperCase());

        // 3. Compression & Extraction Round-trip
        Path tmpDir = Files.createTempDirectory("ttzip_java_test_");
        Path sample = tmpDir.resolve("sample.txt");
        Files.writeString(sample, "Java 21 Enterprise Interop with TTZip Pure Microkernel");

        Path archive = tmpDir.resolve("sample.zip");
        Path dest = tmpDir.resolve("extracted");

        TTZip.compress(List.of(sample.toString()), archive.toString());
        assert Files.exists(archive) : "Archive must exist";
        System.out.println("  [PASS] Java SDK compress() created archive");

        TTZip.extract(archive.toString(), dest.toString());
        Path extractedFile = dest.resolve("sample.txt");
        assert Files.exists(extractedFile) : "Extracted file must exist";
        String readBack = Files.readString(extractedFile);
        assert readBack.equals("Java 21 Enterprise Interop with TTZip Pure Microkernel") : "Content mismatch";
        System.out.println("  [PASS] Java SDK extract() payload verified");

        // Clean up
        Files.deleteIfExists(extractedFile);
        Files.deleteIfExists(dest);
        Files.deleteIfExists(archive);
        Files.deleteIfExists(sample);
        Files.deleteIfExists(tmpDir);

        System.out.println("✅ All Java SDK tests passed successfully!");
    }
}
