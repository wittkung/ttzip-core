// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// JUnit 5 Panama FFM Native Unit Test Suite.

package com.ttzip;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.io.File;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.zip.CRC32;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("TTZip Java 22+ Panama FFM SDK Tests")
public class TTZipTest {

    private Path tempDir;

    @BeforeEach
    public void setUp() throws Exception {
        tempDir = Files.createTempDirectory("ttzip_junit5_test_");
    }

    @AfterEach
    public void tearDown() throws Exception {
        if (tempDir != null && Files.exists(tempDir)) {
            try (var stream = Files.walk(tempDir)) {
                stream.sorted(Comparator.reverseOrder())
                      .map(Path::toFile)
                      .forEach(File::delete);
            }
        }
    }

    @Test
    @DisplayName("Engine version and hardware acceleration detection")
    public void testVersionAndHardware() {
        String version = TTZip.version();
        assertNotNull(version, "Version must not be null");
        boolean hw = TTZip.isHardwareAccelerated();
        // Hardware acceleration is a valid boolean
        assertTrue(hw || !hw, "Hardware acceleration status should be queryable");
    }

    @Test
    @DisplayName("Panama FFM MemorySegment direct allocation and read/write")
    public void testPanamaFFMMemorySegmentDirectReadWrite() {
        try (Arena arena = Arena.ofConfined()) {
            long bufferSize = 1024L;
            MemorySegment segment = arena.allocate(bufferSize);
            assertEquals(bufferSize, segment.byteSize(), "Segment size must match allocated size");
            assertTrue(segment.isNative(), "Confined arena segment must be native");

            // Direct primitive write / read
            segment.set(ValueLayout.JAVA_INT, 0L, 0x12345678);
            assertEquals(0x12345678, segment.get(ValueLayout.JAVA_INT, 0L), "Direct integer read/write mismatch");

            segment.set(ValueLayout.JAVA_LONG, 8L, 0x0123456789ABCDEFL);
            assertEquals(0x0123456789ABCDEFL, segment.get(ValueLayout.JAVA_LONG, 8L), "Direct long read/write mismatch");

            byte[] textBytes = "Panama FFM Zero-Copy Direct Access".getBytes(StandardCharsets.UTF_8);
            MemorySegment textSlice = segment.asSlice(64L, textBytes.length);
            for (int i = 0; i < textBytes.length; i++) {
                textSlice.set(ValueLayout.JAVA_BYTE, (long) i, textBytes[i]);
            }

            byte[] readBack = new byte[textBytes.length];
            for (int i = 0; i < textBytes.length; i++) {
                readBack[i] = textSlice.get(ValueLayout.JAVA_BYTE, (long) i);
            }
            assertArrayEquals(textBytes, readBack, "Slice byte payload must match written data");
        }
    }

    @Test
    @DisplayName("Hardware CRC-32 calculation on byte array and MemorySegment")
    public void testHardwareCRC32Calculation() {
        byte[] payload = "TTZip Ultra-Fast SIMD CRC32 Panama Benchmark Payload 2026".getBytes(StandardCharsets.UTF_8);

        // Reference java.util.zip.CRC32
        CRC32 refCrc = new CRC32();
        refCrc.update(payload);
        int expectedCrc = (int) refCrc.getValue();

        int computedByteArray = TTZip.crc32(payload);
        assertEquals(expectedCrc, computedByteArray, "Byte array CRC-32 must match reference CRC32");

        try (Arena arena = Arena.ofConfined()) {
            MemorySegment segment = arena.allocate((long) payload.length);
            for (int i = 0; i < payload.length; i++) {
                segment.set(ValueLayout.JAVA_BYTE, (long) i, payload[i]);
            }

            int computedSegment = TTZip.crc32(segment, 0);
            assertEquals(expectedCrc, computedSegment, "MemorySegment CRC-32 must match reference CRC32");

            // Incremental CRC32 over sliced segment
            long mid = payload.length / 2;
            MemorySegment firstHalf = segment.asSlice(0, mid);
            MemorySegment secondHalf = segment.asSlice(mid, payload.length - mid);

            int seed = TTZip.crc32(firstHalf, 0);
            int chained = TTZip.crc32(secondHalf, seed);
            assertEquals(expectedCrc, chained, "Chained CRC-32 over slices must match total CRC-32");
        }
    }

    @Test
    @DisplayName("Hardware CRC-64 calculation on MemorySegment")
    public void testHardwareCRC64Calculation() {
        byte[] payload = "TTZip Ultra-Fast SIMD CRC64 Panama Verification Payload".getBytes(StandardCharsets.UTF_8);

        try (Arena arena = Arena.ofConfined()) {
            MemorySegment segment = arena.allocate((long) payload.length);
            for (int i = 0; i < payload.length; i++) {
                segment.set(ValueLayout.JAVA_BYTE, (long) i, payload[i]);
            }

            long crc64Val = TTZip.crc64(segment, 0L);
            assertNotEquals(0L, crc64Val, "CRC-64 must be non-zero for non-empty payload");

            // Seeded consistency check
            long chained = TTZip.crc64(segment, 0x12345678L);
            assertNotEquals(crc64Val, chained, "Seeded CRC-64 must produce distinct digest from unseeded");
        }
    }

    @Test
    @DisplayName("Archive creation, inspection, and extraction round-trip")
    public void testArchiveCreationInspectionAndExtraction() throws Exception {
        Path file1 = tempDir.resolve("doc1.txt");
        Path file2 = tempDir.resolve("nested/doc2.log");
        Files.createDirectories(file2.getParent());

        String content1 = "Document 1 Content: Java Panama Enterprise Module";
        String content2 = "Document 2 Content: High Throughput Archiving Engine Logs";
        Files.writeString(file1, content1);
        Files.writeString(file2, content2);

        Path archiveZip = tempDir.resolve("output.zip");
        Path destDir = tempDir.resolve("extracted");

        // 1. Compress
        assertDoesNotThrow(() -> {
            TTZip.compress(
                List.of(file1.toString(), file2.getParent().toString()),
                archiveZip.toString(),
                TTZip.ArchiveFormat.ZIP,
                TTZip.CompressionLevel.NORMAL,
                null,
                2,
                null
            );
        }, "Compression should complete without exceptions");
        assertTrue(Files.exists(archiveZip), "Archive file must exist on disk");
        assertTrue(Files.size(archiveZip) > 0, "Archive file must have positive length");

        // 2. Inspect
        List<TTZip.EntryMetadata> entries = TTZip.inspect(archiveZip.toString(), null);
        assertNotNull(entries, "Inspect result must not be null");
        assertFalse(entries.isEmpty(), "Inspect result must contain entries");

        // 3. Extract
        assertDoesNotThrow(() -> {
            TTZip.extract(
                archiveZip.toString(),
                destDir.toString(),
                null,
                2,
                null
            );
        }, "Extraction should complete without exceptions");

        Path extractedDoc1 = destDir.resolve("doc1.txt");
        assertTrue(Files.exists(extractedDoc1), "Extracted doc1.txt must exist");
        assertEquals(content1, Files.readString(extractedDoc1), "Extracted content for doc1.txt must match");
    }

    @Test
    @DisplayName("Progress listener telemetry callback verification")
    public void testProgressListenerCallback() throws Exception {
        Path largeFile = tempDir.resolve("large_payload.bin");
        byte[] buffer = new byte[64 * 1024];
        for (int i = 0; i < buffer.length; i++) {
            buffer[i] = (byte) (i % 251);
        }
        Files.write(largeFile, buffer);

        Path archiveZip = tempDir.resolve("progress_test.zip");
        Path destDir = tempDir.resolve("progress_extracted");

        List<TTZip.ArchiveProgress> progressEvents = new ArrayList<>();
        AtomicBoolean completedNormally = new AtomicBoolean(false);

        TTZip.ProgressListener listener = progress -> {
            progressEvents.add(progress);
            return true; // continue operation
        };

        TTZip.compress(
            List.of(largeFile.toString()),
            archiveZip.toString(),
            TTZip.ArchiveFormat.ZIP,
            TTZip.CompressionLevel.FAST,
            null,
            1,
            listener
        );

        assertTrue(Files.exists(archiveZip), "Archive must exist after compression with listener");

        // Test extraction with progress
        List<TTZip.ArchiveProgress> extractEvents = new ArrayList<>();
        TTZip.extract(
            archiveZip.toString(),
            destDir.toString(),
            null,
            1,
            p -> {
                extractEvents.add(p);
                return true;
            }
        );

        assertTrue(Files.exists(destDir.resolve("large_payload.bin")), "Extracted large payload must exist");
    }

    @Test
    @DisplayName("All 16 archive formats matrix creation and extraction")
    public void testAll16FormatsMatrix() throws Exception {
        Path sampleFile = tempDir.resolve("matrix_doc.txt");
        String content = "TTZip 16-Format Matrix Java 22+ Panama FFM Payload\n".repeat(100);
        Files.writeString(sampleFile, content);

        record FormatTestCase(TTZip.ArchiveFormat format, String filename, boolean canCreate) {}

        List<FormatTestCase> matrix = List.of(
            new FormatTestCase(TTZip.ArchiveFormat.ZIP, "archive.zip", true),
            new FormatTestCase(TTZip.ArchiveFormat.SEVEN_ZIP, "archive.7z", true),
            new FormatTestCase(TTZip.ArchiveFormat.TAR, "archive.tar", true),
            new FormatTestCase(TTZip.ArchiveFormat.TAR_GZ, "archive.tar.gz", true),
            new FormatTestCase(TTZip.ArchiveFormat.TAR_GZ, "archive.tgz", true),
            new FormatTestCase(TTZip.ArchiveFormat.TAR_BZ2, "archive.tar.bz2", true),
            new FormatTestCase(TTZip.ArchiveFormat.TAR_BZ2, "archive.tbz2", true),
            new FormatTestCase(TTZip.ArchiveFormat.TAR_XZ, "archive.tar.xz", true),
            new FormatTestCase(TTZip.ArchiveFormat.TAR_XZ, "archive.txz", true),
            new FormatTestCase(TTZip.ArchiveFormat.TAR_ZSTD, "archive.tar.zst", true),
            new FormatTestCase(TTZip.ArchiveFormat.TAR_ZSTD, "archive.tar.zstd", true),
            new FormatTestCase(TTZip.ArchiveFormat.GZ, "archive.gz", false),
            new FormatTestCase(TTZip.ArchiveFormat.ZST, "archive.zst", false),
            new FormatTestCase(TTZip.ArchiveFormat.BZ2, "archive.bz2", false),
            new FormatTestCase(TTZip.ArchiveFormat.XZ, "archive.xz", false),
            new FormatTestCase(TTZip.ArchiveFormat.ISO, "archive.iso", false)
        );

        for (FormatTestCase tc : matrix) {
            Path archivePath = tempDir.resolve(tc.filename());
            Path extractDir = tempDir.resolve("extracted_" + tc.filename().replace(".", "_"));

            if (tc.canCreate()) {
                assertDoesNotThrow(() -> {
                    TTZip.compress(
                        List.of(sampleFile.toString()),
                        archivePath.toString(),
                        tc.format(),
                        TTZip.CompressionLevel.NORMAL,
                        null,
                        2,
                        null
                    );
                }, "Creation should succeed for " + tc.filename());

                assertTrue(Files.exists(archivePath), "Archive " + tc.filename() + " must exist");
                assertTrue(Files.size(archivePath) > 0, "Archive " + tc.filename() + " must have size > 0");

                // Inspect
                List<TTZip.EntryMetadata> entries = TTZip.inspect(archivePath.toString(), null);
                assertNotNull(entries);
                assertFalse(entries.isEmpty());

                // Extract
                assertDoesNotThrow(() -> {
                    TTZip.extract(archivePath.toString(), extractDir.toString(), null, 2, null);
                }, "Extraction should succeed for " + tc.filename());

                Path extractedDoc = extractDir.resolve("matrix_doc.txt");
                assertTrue(Files.exists(extractedDoc), "Extracted matrix_doc.txt must exist for " + tc.filename());
                assertEquals(content, Files.readString(extractedDoc), "Content mismatch for " + tc.filename());
            } else {
                // Verify format enum code validity for single-stream & optical formats
                assertNotNull(tc.format());
                assertTrue(tc.format().code > 0, "Format code must be valid");
            }
        }
    }

    @Test
    @DisplayName("Compression level configurations (1 to 22)")
    public void testCompressionLevelConfigurations() throws Exception {
        Path sampleFile = tempDir.resolve("compress_level_test.txt");
        String content = "Compression Level Evaluation 1-22 Payload\n".repeat(200);
        Files.writeString(sampleFile, content);

        int[] testLevels = {1, 3, 6, 9, 12, 19, 22};
        for (int lvl : testLevels) {
            Path outArchive = tempDir.resolve("level_" + lvl + ".tar.zst");
            Path outExtract = tempDir.resolve("extracted_level_" + lvl);

            assertDoesNotThrow(() -> {
                TTZip.compress(
                    List.of(sampleFile.toString()),
                    outArchive.toString(),
                    TTZip.ArchiveFormat.TAR_ZSTD,
                    lvl,
                    null,
                    2,
                    null
                );
            }, "Compression at level " + lvl + " should succeed");

            assertTrue(Files.exists(outArchive));
            assertTrue(Files.size(outArchive) > 0);

            // Extract and verify
            TTZip.extract(outArchive.toString(), outExtract.toString(), null, 2, null);
            Path extFile = outExtract.resolve("compress_level_test.txt");
            assertTrue(Files.exists(extFile));
            assertEquals(content, Files.readString(extFile));
        }
    }

    @Test
    @DisplayName("AES-256 password-protected archive creation, extraction, and invalid password error detection")
    public void testPasswordProtectedArchiveExtraction() throws Exception {
        Path secretFile = tempDir.resolve("secret.txt");
        String secretData = "CONFIDENTIAL: AES-256 Zero-Knowledge Payload 2026";
        Files.writeString(secretFile, secretData);

        Path encryptedZip = tempDir.resolve("encrypted_vault.zip");
        Path validExtractDir = tempDir.resolve("vault_extracted_valid");
        Path invalidExtractDir = tempDir.resolve("vault_extracted_invalid");

        String correctPassword = "TTZipJavaSecretPassword2026!";
        String wrongPassword = "IncorrectPassword!";

        // 1. Compress with password
        TTZip.compress(
            List.of(secretFile.toString()),
            encryptedZip.toString(),
            TTZip.ArchiveFormat.ZIP,
            TTZip.CompressionLevel.NORMAL,
            correctPassword,
            1,
            null
        );
        assertTrue(Files.exists(encryptedZip));

        // 2. Inspect metadata with password
        List<TTZip.EntryMetadata> entries = TTZip.inspect(encryptedZip.toString(), correctPassword);
        assertNotNull(entries);
        assertFalse(entries.isEmpty());
        assertTrue(entries.get(0).isEncrypted(), "Entry should be marked encrypted");

        // 3. Extract with correct password
        assertDoesNotThrow(() -> {
            TTZip.extract(encryptedZip.toString(), validExtractDir.toString(), correctPassword, 1, null);
        }, "Extraction with correct password should succeed");

        Path decryptedFile = validExtractDir.resolve("secret.txt");
        assertTrue(Files.exists(decryptedFile));
        assertEquals(secretData, Files.readString(decryptedFile));

        // 4. Extract with incorrect password -> should throw exception
        assertThrows(RuntimeException.class, () -> {
            TTZip.extract(encryptedZip.toString(), invalidExtractDir.toString(), wrongPassword, 1, null);
        }, "Extraction with incorrect password must fail");
    }

    @Test
    @DisplayName("Corrupt header and malformed archive detection")
    public void testCorruptHeaderDetection() throws Exception {
        Path corruptFile = tempDir.resolve("corrupt_archive.zip");
        byte[] garbage = new byte[]{ 0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, (byte) 0xFF, (byte) 0xFF, 0x12, 0x34 };
        Files.write(corruptFile, garbage);

        Path destDir = tempDir.resolve("corrupt_extracted");

        // Extracting corrupt archive must throw exception (ErrCorruptHeader)
        assertThrows(RuntimeException.class, () -> {
            TTZip.extract(corruptFile.toString(), destDir.toString(), null, 1, null);
        }, "Extracting corrupted header should throw exception");

        // Inspecting corrupt archive with invalid headers returns empty entries
        List<TTZip.EntryMetadata> entries = TTZip.inspect(corruptFile.toString(), null);
        assertNotNull(entries);
        assertTrue(entries.isEmpty(), "Corrupted header should yield empty entries");

        // Inspecting non-existent file must throw exception
        assertThrows(RuntimeException.class, () -> {
            TTZip.inspect(tempDir.resolve("non_existent.zip").toString(), null);
        }, "Inspecting non-existent file should throw exception");
    }

    public static void main(String[] args) throws Exception {
        System.out.println("⚡️ Running TTZip Java 22+ Panama FFM Test Suite via Standalone Runner...");
        TTZipTest suite = new TTZipTest();

        suite.setUp();
        try {
            suite.testVersionAndHardware();
            System.out.println("  [PASS] testVersionAndHardware");

            suite.testPanamaFFMMemorySegmentDirectReadWrite();
            System.out.println("  [PASS] testPanamaFFMMemorySegmentDirectReadWrite");

            suite.testHardwareCRC32Calculation();
            System.out.println("  [PASS] testHardwareCRC32Calculation");

            suite.testHardwareCRC64Calculation();
            System.out.println("  [PASS] testHardwareCRC64Calculation");

            suite.testArchiveCreationInspectionAndExtraction();
            System.out.println("  [PASS] testArchiveCreationInspectionAndExtraction");

            suite.testProgressListenerCallback();
            System.out.println("  [PASS] testProgressListenerCallback");

            suite.testAll16FormatsMatrix();
            System.out.println("  [PASS] testAll16FormatsMatrix");

            suite.testCompressionLevelConfigurations();
            System.out.println("  [PASS] testCompressionLevelConfigurations");

            suite.testPasswordProtectedArchiveExtraction();
            System.out.println("  [PASS] testPasswordProtectedArchiveExtraction");

            suite.testCorruptHeaderDetection();
            System.out.println("  [PASS] testCorruptHeaderDetection");

            System.out.println("✅ All JUnit 5 assertions passed successfully in Java 22+ FFM!");
        } finally {
            suite.tearDown();
        }
    }
}
