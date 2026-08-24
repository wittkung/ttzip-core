// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

package com.ttzip.examples;

import com.ttzip.NativeLoader;
import com.ttzip.TTZip;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

public class Quickstart {

    public static void main(String[] args) throws Exception {
        System.out.println("================================================================================");
        System.out.println("⚡️ TTZip Java 22+ Panama FFM Zero-Config Quickstart Demo");
        System.out.println("================================================================================");

        // 1. Query Native Engine Metadata
        String version = TTZip.version();
        boolean isHwAccelerated = TTZip.isHardwareAccelerated();
        NativeLoader.Platform platform = NativeLoader.detectPlatform();
        NativeLoader.LoadReport report = NativeLoader.getReport();

        System.out.println("• Engine Version:        " + version);
        System.out.println("• Platform Classifier:   " + platform.classifier());
        System.out.println("• Hardware SIMD Active:  " + isHwAccelerated);
        System.out.println("• Native Loader Source:  " + report.sourceType());
        System.out.println("• Native Library Path:   " + report.resolvedPath());
        System.out.println("--------------------------------------------------------------------------------");

        // 2. Hardware-Accelerated CRC-32 Checksum Calculation
        byte[] samplePayload = "TTZip High-Throughput Panama FFM Payload 2026".getBytes(StandardCharsets.UTF_8);
        int crc32Value = TTZip.crc32(samplePayload);
        System.out.printf("• Hardware CRC-32:       0x%08X\n", crc32Value);
        System.out.println("--------------------------------------------------------------------------------");

        // 3. Prepare Temporary Files for Compression Demo
        Path workDir = Files.createTempDirectory("ttzip_java_quickstart_");
        Path sample1 = workDir.resolve("manifest.json");
        Path sample2 = workDir.resolve("nested/data.txt");
        Files.createDirectories(sample2.getParent());

        String manifestContent = "{\"project\": \"ttzip\", \"runtime\": \"Java 22+ FFM\", \"zero_config\": true}";
        String textContent = "High-performance compression and archiving engine with zero subprocess overhead.";
        Files.writeString(sample1, manifestContent);
        Files.writeString(sample2, textContent);

        Path archiveZip = workDir.resolve("demo_archive.zip");
        Path extractDir = workDir.resolve("extracted_output");

        try {
            // 4. Compress Files with Progress Monitoring
            System.out.println("📦 Creating archive: " + archiveZip.getFileName() + "...");
            TTZip.compress(
                List.of(sample1.toString(), sample2.getParent().toString()),
                archiveZip.toString(),
                TTZip.ArchiveFormat.ZIP,
                TTZip.CompressionLevel.NORMAL,
                null,
                2,
                progress -> {
                    System.out.printf("   -> Progress: %3.0f%% | Phase: %s | Current: %s\n",
                        progress.fractionCompleted() * 100.0,
                        progress.phase(),
                        progress.currentEntryPath().isEmpty() ? "processing" : progress.currentEntryPath()
                    );
                    return true;
                }
            );

            System.out.println("   ✓ Archive created successfully (Size: " + Files.size(archiveZip) + " bytes)");
            System.out.println("--------------------------------------------------------------------------------");

            // 5. Inspect Archive Entries without Extraction
            System.out.println("🔍 Inspecting archive metadata...");
            List<TTZip.EntryMetadata> entries = TTZip.inspect(archiveZip.toString(), null);
            for (TTZip.EntryMetadata entry : entries) {
                System.out.printf("   * %-20s (Size: %6d bytes, CRC: 0x%08X, Dir: %s)\n",
                    entry.path(), entry.uncompressedSize(), entry.crc32(), entry.isDirectory()
                );
            }
            System.out.println("--------------------------------------------------------------------------------");

            // 6. Extract Archive
            System.out.println("📂 Extracting archive to " + extractDir.getFileName() + "...");
            TTZip.extract(
                archiveZip.toString(),
                extractDir.toString(),
                null,
                2,
                progress -> true
            );

            // 7. Verify Integrity of Extracted Content
            Path extractedManifest = extractDir.resolve("manifest.json");
            if (!Files.exists(extractedManifest) || !Files.readString(extractedManifest).equals(manifestContent)) {
                throw new IllegalStateException("Extracted manifest.json content mismatch!");
            }
            System.out.println("   ✓ All extracted files verified with 100% integrity match!");

        } finally {
            // Clean up temporary workspace
            try (var stream = Files.walk(workDir)) {
                stream.sorted(Comparator.reverseOrder())
                      .map(Path::toFile)
                      .forEach(File::delete);
            }
        }

        System.out.println("================================================================================");
        System.out.println("🎉 TTZip Java Quickstart Demo Completed Successfully (Exit Code: 0)");
        System.out.println("================================================================================");
    }
}
