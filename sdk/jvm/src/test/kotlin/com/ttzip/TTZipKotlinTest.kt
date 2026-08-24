// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Kotlin / Coroutines.
// JUnit 5 Kotlin Coroutines & Flow Test Suite.

package com.ttzip

import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.runBlocking
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.DisplayName
import org.junit.jupiter.api.Test
import java.io.File
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.util.Comparator

@DisplayName("TTZip Kotlin Coroutines & Flow Progress SDK Tests")
class TTZipKotlinTest {

    private lateinit var tempDir: Path

    @BeforeEach
    fun setUp() {
        tempDir = Files.createTempDirectory("ttzip_kotlin_test_")
    }

    @AfterEach
    fun tearDown() {
        if (::tempDir.isInitialized && Files.exists(tempDir)) {
            Files.walk(tempDir)
                .sorted(Comparator.reverseOrder())
                .map(Path::toFile)
                .forEach(File::delete)
        }
    }

    @Test
    @DisplayName("ByteArray SIMD CRC32 Kotlin extension test")
    fun testByteArrayCrc32Extension() {
        val payload = "Kotlin Coroutines & Flow with TTZip Native SIMD".toByteArray(StandardCharsets.UTF_8)
        val crc = payload.ttzipCrc32()
        assertTrue(crc != 0, "CRC32 computation on ByteArray should yield non-zero checksum")

        val expected = TTZip.crc32(payload)
        assertEquals(expected, crc, "ByteArray.ttzipCrc32() must match TTZip.crc32(payload)")
    }

    @Test
    @DisplayName("Kotlin Flow progress collection during archive compression")
    fun testCompressFlowProgressCollection() = runBlocking {
        val sampleFile = tempDir.resolve("sample.txt").toFile()
        sampleFile.writeText("Kotlin Flow Streaming Archiving Test Payload\n".repeat(100))

        val archiveFile = tempDir.resolve("output_flow.zip").toFile()

        val progressList = sampleFile.ttzipCompressFlow(
            destination = archiveFile,
            format = TTZip.ArchiveFormat.ZIP,
            level = TTZip.CompressionLevel.NORMAL,
            threads = 2
        ).toList()

        assertTrue(archiveFile.exists(), "Archive file must exist on disk after Flow collection")
        assertTrue(archiveFile.length() > 0, "Archive size must be positive")
    }

    @Test
    @DisplayName("Kotlin Flow progress collection during archive extraction")
    fun testExtractFlowProgressCollection() = runBlocking {
        val sampleFile = tempDir.resolve("extract_sample.txt").toFile()
        val originalText = "Streaming Extraction via Kotlin Coroutines & Flow"
        sampleFile.writeText(originalText)

        val archiveFile = tempDir.resolve("extract_flow.zip").toFile()
        val destDir = tempDir.resolve("flow_extracted_dir").toFile()
        destDir.mkdirs()

        TTZip.compress(
            listOf(sampleFile.absolutePath),
            archiveFile.absolutePath,
            TTZip.ArchiveFormat.ZIP,
            TTZip.CompressionLevel.FAST,
            null,
            1,
            null
        )
        assertTrue(archiveFile.exists())

        val extractProgressList = archiveFile.ttzipExtractFlow(
            destinationDirectory = destDir,
            threads = 1
        ).toList()

        val extractedFile = File(destDir, "extract_sample.txt")
        assertTrue(extractedFile.exists(), "Extracted file must exist")
        assertEquals(originalText, extractedFile.readText(), "Extracted file content must match")
    }

    @Test
    @DisplayName("Path Flow extensions for compression and extraction")
    fun testPathFlowExtensions() = runBlocking {
        val pathSample = tempDir.resolve("path_test.txt")
        val content = "Path extension Flow test content"
        Files.writeString(pathSample, content)

        val pathArchive = tempDir.resolve("path_test.zip")
        val pathDest = tempDir.resolve("path_extracted")
        Files.createDirectories(pathDest)

        val compressEvents = pathSample.ttzipCompressFlow(pathArchive).toList()
        assertTrue(Files.exists(pathArchive))

        val extractEvents = pathArchive.ttzipExtractFlow(pathDest).toList()
        val extracted = pathDest.resolve("path_test.txt")
        assertTrue(Files.exists(extracted))
        assertEquals(content, Files.readString(extracted))
    }

    @Test
    @DisplayName("Suspending non-blocking compression and extraction offloaded to Dispatchers.IO")
    fun testSuspendingCompressAndExtract() = runBlocking {
        val inputFile = tempDir.resolve("suspend_input.txt").toFile()
        val testData = "Non-blocking suspending function verification on Dispatchers.IO"
        inputFile.writeText(testData)

        val archiveFile = tempDir.resolve("suspend_archive.zip").toFile()
        val destDir = tempDir.resolve("suspend_dest").toFile()
        destDir.mkdirs()

        // Call suspend fun File.ttzipCompress
        inputFile.ttzipCompress(archiveFile, TTZip.ArchiveFormat.ZIP, TTZip.CompressionLevel.NORMAL)
        assertTrue(archiveFile.exists(), "Archive must exist after suspending compress")

        // Call suspend fun File.ttzipExtract
        archiveFile.ttzipExtract(destDir)
        val extracted = File(destDir, "suspend_input.txt")
        assertTrue(extracted.exists(), "Extracted file must exist after suspending extract")
        assertEquals(testData, extracted.readText())
    }

    @Test
    @DisplayName("File.ttzipInspect extension metadata inspection")
    fun testFileInspectExtension() {
        val file1 = tempDir.resolve("inspect1.txt").toFile()
        file1.writeText("Inspect Payload 1")

        val archiveFile = tempDir.resolve("inspect_archive.zip").toFile()
        TTZip.compress(
            listOf(file1.absolutePath),
            archiveFile.absolutePath,
            TTZip.ArchiveFormat.ZIP,
            TTZip.CompressionLevel.NORMAL,
            null,
            1,
            null
        )

        val entries = archiveFile.ttzipInspect()
        assertNotNull(entries)
        assertFalse(entries.isEmpty())
        val match = entries.find { it.path.contains("inspect1.txt") }
        assertNotNull(match, "Metadata for inspect1.txt should be found")
        assertFalse(match!!.isDirectory)
    }

    companion object {
        @JvmStatic
        fun main(args: Array<String>) = runBlocking {
            println("⚡️ Running TTZip Kotlin Flow & Coroutines SDK Test Suite...")
            val suite = TTZipKotlinTest()
            suite.setUp()
            try {
                suite.testByteArrayCrc32Extension()
                println("  [PASS] testByteArrayCrc32Extension")

                suite.testCompressFlowProgressCollection()
                println("  [PASS] testCompressFlowProgressCollection")

                suite.testExtractFlowProgressCollection()
                println("  [PASS] testExtractFlowProgressCollection")

                suite.testPathFlowExtensions()
                println("  [PASS] testPathFlowExtensions")

                suite.testSuspendingCompressAndExtract()
                println("  [PASS] testSuspendingCompressAndExtract")

                suite.testFileInspectExtension()
                println("  [PASS] testFileInspectExtension")

                println("✅ All Kotlin Flow & Coroutines tests passed successfully!")
            } finally {
                suite.tearDown()
            }
        }
    }
}
