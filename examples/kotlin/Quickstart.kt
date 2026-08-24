// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-Performance Kotlin Coroutines & Flow Quickstart Demo.

package com.ttzip.examples

import com.ttzip.NativeLoader
import com.ttzip.TTZip
import com.ttzip.ttzipCompressFlow
import com.ttzip.ttzipCrc32
import com.ttzip.ttzipCrc64
import com.ttzip.ttzipExtractFlow
import com.ttzip.ttzipInspect
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.runBlocking
import java.io.File
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path

fun main() = runBlocking {
    println("================================================================================")
    println("⚡️ TTZip Kotlin Coroutines & Flow Zero-Config Quickstart Demo")
    println("================================================================================")

    // 1. Query Native Engine & Platform Metadata
    val version = TTZip.version()
    val isHwAccelerated = TTZip.isHardwareAccelerated()
    val platform = NativeLoader.detectPlatform()
    val report = NativeLoader.getReport()

    println("• Engine Version:        $version")
    println("• Platform Classifier:   ${platform.classifier()}")
    println("• Hardware SIMD Active:  $isHwAccelerated")
    println("• Native Loader Source:  ${report.sourceType()}")
    println("• Native Library Path:   ${report.resolvedPath()}")
    println("--------------------------------------------------------------------------------")

    // 2. Hardware-Accelerated CRC-32 & CRC-64 via Kotlin Extensions
    val payload = "Kotlin Coroutines + TTZip High-Throughput Engine 2026".toByteArray(StandardCharsets.UTF_8)
    val crc32Val = payload.ttzipCrc32()
    val crc64Val = payload.ttzipCrc64()
    println(String.format("• Hardware CRC-32:       0x%08X", crc32Val))
    println(String.format("• Hardware CRC-64:       0x%016X", crc64Val))
    println("--------------------------------------------------------------------------------")

    // 3. Prepare Temporary Files for Archiving Demo
    val workDir = Files.createTempDirectory("ttzip_kotlin_quickstart_").toFile()
    val sampleFile = File(workDir, "dataset.json")
    sampleFile.writeText("{\"title\": \"TTZip Kotlin Flow\", \"reactive\": true, \"throughput\": \"4.8 GB/s\"}")

    val nestedDir = File(workDir, "resources").apply { mkdirs() }
    val nestedFile = File(nestedDir, "notes.txt")
    nestedFile.writeText("Reactive backpressure-aware archive stream via Kotlin Flow.")

    val archiveZip = File(workDir, "demo_kotlin.zip")
    val extractDir = File(workDir, "extracted_kotlin").apply { mkdirs() }

    try {
        // 4. Reactive Compression Progress via Kotlin Flow
        println("📦 Compressing via Kotlin Flow -> ${archiveZip.name}...")
        sampleFile.ttzipCompressFlow(
            destination = archiveZip,
            format = TTZip.ArchiveFormat.ZIP,
            level = TTZip.CompressionLevel.NORMAL
        ).collect { progress ->
            println(String.format("   -> Progress: %3.0f%% | Phase: %s",
                progress.fractionCompleted * 100.0,
                progress.phase
            ))
        }
        println("   ✓ Archive created successfully (Size: ${archiveZip.length()} bytes)")
        println("--------------------------------------------------------------------------------")

        // 5. Inspect Archive Entries using Kotlin Extension
        println("🔍 Inspecting archive metadata...")
        val entries = archiveZip.ttzipInspect()
        for (entry in entries) {
            println(String.format("   * %-20s (Size: %6d bytes, CRC: 0x%08X, Dir: %s)",
                entry.path, entry.uncompressedSize, entry.crc32, entry.isDirectory
            ))
        }
        println("--------------------------------------------------------------------------------")

        // 6. Reactive Extraction Progress via Kotlin Flow
        println("📂 Extracting via Kotlin Flow -> ${extractDir.name}...")
        archiveZip.ttzipExtractFlow(destinationDirectory = extractDir).collect { progress ->
            println(String.format("   -> Extracting: %3.0f%%", progress.fractionCompleted * 100.0))
        }

        // 7. Verify Extracted Content Integrity
        val extractedDataset = File(extractDir, "dataset.json")
        if (!extractedDataset.exists() || extractedDataset.readText() != sampleFile.readText()) {
            error("Extracted dataset.json content verification failed!")
        }
        println("   ✓ All extracted entries verified successfully!")

    } finally {
        workDir.deleteRecursively()
    }

    println("================================================================================")
    println("🎉 TTZip Kotlin Quickstart Demo Completed Successfully (Exit Code: 0)")
    println("================================================================================")
}
