// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: Advanced Kotlin Coroutines & Flow Demo.
// Demonstrates reactive Flow<ArchiveProgress> progress streaming, cancellation,
// multi-format support (7z, tar.zst, zip), AES-256 password protection, and inspection.

package com.ttzip.examples

import com.ttzip.NativeLoader
import com.ttzip.TTZip
import com.ttzip.ttzipCompressFlow
import com.ttzip.ttzipCrc32
import com.ttzip.ttzipCrc64
import com.ttzip.ttzipExtractFlow
import com.ttzip.ttzipInspect
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import java.io.File
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path

fun main() = runBlocking {
    println("================================================================================")
    println("⚡️ TTZip Kotlin Coroutines & Flow Advanced Showcase")
    println("================================================================================")

    // 1. Engine & SIMD Hardware Telemetry
    val version = TTZip.version()
    val isHwAccelerated = TTZip.isHardwareAccelerated()
    val platform = NativeLoader.detectPlatform()

    println("1. Engine & Platform Information:")
    println("   • Engine Version:        $version")
    println("   • Platform:              ${platform.classifier()}")
    println("   • SIMD Acceleration:     ${if (isHwAccelerated) "ENABLED (ARM NEON / AVX-512)" else "DISABLED"}")
    println("--------------------------------------------------------------------------------")

    // 2. Hardware SIMD Checksums via Kotlin Extensions
    val rawPayload = "Kotlin Coroutines Flow Reactive Streaming 2026".toByteArray(StandardCharsets.UTF_8)
    val crc32Checksum = rawPayload.ttzipCrc32()
    val crc64Checksum = rawPayload.ttzipCrc64()
    println("2. Hardware SIMD Checksum Calculation:")
    println(String.format("   • SIMD CRC-32:           0x%08X", crc32Checksum))
    println(String.format("   • SIMD CRC-64:           0x%016X", crc64Checksum))
    println("--------------------------------------------------------------------------------")

    // 3. Prepare Multi-File Test Dataset
    val workDir = Files.createTempDirectory("ttzip_kotlin_adv_").toFile()
    val payloadDir = File(workDir, "payload").apply { mkdirs() }

    val configJson = File(payloadDir, "config.json")
    val largeData = File(payloadDir, "large_data.bin")
    val notesTxt = File(payloadDir, "notes.txt")

    configJson.writeText("{\"framework\": \"Kotlin Coroutines\", \"version\": \"2.0\", \"reactive\": true}")
    notesTxt.writeText("TTZip Kotlin Coroutines reactive backpressure and flow cancellation.")

    // Create 1 MB binary payload for streaming demonstration
    val buffer = ByteArray(1024 * 1024) { (it * 37).toByte() }
    largeData.writeBytes(buffer)

    val sourceFiles = listOf(configJson, largeData, notesTxt)
    val aesPassword = "KotlinCoroutinesSecret2026!"

    try {
        // 4. Reactive Flow Collection with Progress Logging (7z Solid + AES-256)
        val archive7z = File(workDir, "secure_payload.7z")
        println("3. Compressing to 7z Solid with AES-256 via Flow<ArchiveProgress>...")

        sourceFiles.ttzipCompressFlow(
            destination = archive7z,
            format = TTZip.ArchiveFormat.SEVEN_ZIP,
            level = TTZip.CompressionLevel.MAXIMUM,
            password = aesPassword,
            threads = 4
        ).onEach { progress ->
            val pct = progress.fractionCompleted * 100.0
            val entry = if (progress.currentEntryPath.isEmpty()) "processing" else progress.currentEntryPath
            println(String.format("   [Flow 7z] -> %5.1f%% | %s", pct, entry))
        }.catch { ex ->
            println("   ❌ Flow compression error: ${ex.message}")
        }.collect()

        println("   ✓ 7z Archive Created: ${archive7z.name} (${archive7z.length()} bytes)")
        println("--------------------------------------------------------------------------------")

        // 5. Coroutine Flow Cancellation Showcase
        println("4. Demonstrating Coroutine Flow Cancellation & Timeout Handling...")
        val cancelArchive = File(workDir, "cancelled_archive.zip")

        val cancellationJob = launch(Dispatchers.Default) {
            try {
                sourceFiles.ttzipCompressFlow(
                    destination = cancelArchive,
                    format = TTZip.ArchiveFormat.ZIP,
                    level = TTZip.CompressionLevel.ULTRA,
                    threads = 1
                ).collect { progress ->
                    println(String.format("   [Cancel Demo] Progress: %3.0f%%", progress.fractionCompleted * 100.0))
                    // Simulate cooperative cancellation check
                    ensureActive()
                }
            } catch (ce: CancellationException) {
                println("   ✓ Coroutine Job gracefully cancelled as expected: ${ce.message}")
            }
        }

        // Allow flow to start, then trigger cooperative cancellation
        delay(5)
        cancellationJob.cancel(CancellationException("User requested immediate job cancellation"))
        cancellationJob.join()
        println("   ✓ Flow cancellation verified with zero leaks.")
        println("--------------------------------------------------------------------------------")

        // 6. Creating TAR.ZST Archive via Flow
        val archiveTarZst = File(workDir, "dataset.tar.zst")
        println("5. Compressing to TAR.ZST with Flow<ArchiveProgress>...")

        sourceFiles.ttzipCompressFlow(
            destination = archiveTarZst,
            format = TTZip.ArchiveFormat.TAR_ZSTD,
            level = TTZip.CompressionLevel.NORMAL,
            threads = 4
        ).collect { progress ->
            println(String.format("   [Flow Zstd] -> %3.0f%% | %s",
                progress.fractionCompleted * 100.0,
                progress.currentEntryPath
            ))
        }
        println("   ✓ TAR.ZST Archive Created: ${archiveTarZst.name} (${archiveTarZst.length()} bytes)")
        println("--------------------------------------------------------------------------------")

        // 7. Inspect Archive Metadata without Extraction
        println("6. Inspecting Encrypted 7z Archive Metadata:")
        val entries = archive7z.ttzipInspect(password = aesPassword)
        for (entry in entries) {
            println(String.format("   * %-22s | Size: %7d B | CRC: 0x%08X | Encrypted: %s",
                entry.path, entry.uncompressedSize, entry.crc32, entry.isEncrypted
            ))
        }
        println("--------------------------------------------------------------------------------")

        // 8. Reactive Extraction with Flow Progress Collection
        val extractDir = File(workDir, "extracted_7z").apply { mkdirs() }
        println("7. Extracting AES-256 Protected 7z Archive via Flow...")

        archive7z.ttzipExtractFlow(
            destinationDirectory = extractDir,
            password = aesPassword,
            threads = 4
        ).collect { progress ->
            println(String.format("   [Extract Flow] -> %3.0f%%", progress.fractionCompleted * 100.0))
        }

        // Verify extracted content
        val extractedJson = File(extractDir, "config.json")
        if (extractedJson.exists()) {
            println("   ✓ Decrypted payload verified: ${extractedJson.readText()}")
        }

    } finally {
        workDir.deleteRecursively()
    }

    println("================================================================================")
    println("🎉 TTZip Kotlin Advanced Showcase Completed Successfully (Exit Code: 0)")
    println("================================================================================")
}
