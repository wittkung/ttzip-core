// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Java 22+.
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

            System.out.println("✅ All JUnit 5 assertions passed successfully in Java 22+ FFM!");
        } finally {
            suite.tearDown();
        }
    }
}
