// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip Kotlin Coroutines & Flow Extensions.

package com.ttzip

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.withContext
import java.io.File
import java.nio.file.Path

/**
 * Emits real-time [TTZip.ArchiveProgress] events during archive compression.
 */
fun File.ttzipCompressFlow(
    destination: File,
    format: TTZip.ArchiveFormat = TTZip.ArchiveFormat.AUTO,
    level: TTZip.CompressionLevel = TTZip.CompressionLevel.NORMAL,
    password: String? = null,
    threads: Int = 0
): Flow<TTZip.ArchiveProgress> = callbackFlow {
    val listener = TTZip.ProgressListener { progress ->
        trySend(progress).isSuccess
    }

    try {
        TTZip.compress(
            listOf(this@ttzipCompressFlow.absolutePath),
            destination.absolutePath,
            format,
            level,
            password,
            threads,
            listener
        )
        close()
    } catch (e: Exception) {
        close(e)
    }

    awaitClose { /* Cleanup resources */ }
}

/**
 * Emits real-time [TTZip.ArchiveProgress] events during archive extraction.
 */
fun File.ttzipExtractFlow(
    destinationDirectory: File,
    password: String? = null,
    threads: Int = 0
): Flow<TTZip.ArchiveProgress> = callbackFlow {
    val listener = TTZip.ProgressListener { progress ->
        trySend(progress).isSuccess
    }

    try {
        TTZip.extract(
            this@ttzipExtractFlow.absolutePath,
            destinationDirectory.absolutePath,
            password,
            threads,
            listener
        )
        close()
    } catch (e: Exception) {
        close(e)
    }

    awaitClose { /* Cleanup resources */ }
}

/**
 * Path extension for streaming compression progress.
 */
fun Path.ttzipCompressFlow(
    destination: Path,
    format: TTZip.ArchiveFormat = TTZip.ArchiveFormat.AUTO,
    level: TTZip.CompressionLevel = TTZip.CompressionLevel.NORMAL,
    password: String? = null,
    threads: Int = 0
): Flow<TTZip.ArchiveProgress> = this.toFile().ttzipCompressFlow(destination.toFile(), format, level, password, threads)

/**
 * Path extension for streaming extraction progress.
 */
fun Path.ttzipExtractFlow(
    destinationDirectory: Path,
    password: String? = null,
    threads: Int = 0
): Flow<TTZip.ArchiveProgress> = this.toFile().ttzipExtractFlow(destinationDirectory.toFile(), password, threads)

/**
 * Suspending non-blocking compression offloaded to Dispatchers.IO.
 */
suspend fun File.ttzipCompress(
    destination: File,
    format: TTZip.ArchiveFormat = TTZip.ArchiveFormat.AUTO,
    level: TTZip.CompressionLevel = TTZip.CompressionLevel.NORMAL,
    password: String? = null,
    threads: Int = 0
) = withContext(Dispatchers.IO) {
    TTZip.compress(
        listOf(this@ttzipCompress.absolutePath),
        destination.absolutePath,
        format,
        level,
        password,
        threads,
        null
    )
}

/**
 * Suspending non-blocking extraction offloaded to Dispatchers.IO.
 */
suspend fun File.ttzipExtract(
    destinationDirectory: File,
    password: String? = null,
    threads: Int = 0
) = withContext(Dispatchers.IO) {
    TTZip.extract(
        this@ttzipExtract.absolutePath,
        destinationDirectory.absolutePath,
        password,
        threads,
        null
    )
}

/**
 * Inspects archive metadata entries without disk extraction.
 */
fun File.ttzipInspect(password: String? = null): List<TTZip.EntryMetadata> {
    return TTZip.inspect(this.absolutePath, password)
}

/**
 * Computes SIMD-accelerated CRC-32 on byte array.
 */
fun ByteArray.ttzipCrc32(): Int = TTZip.crc32(this)
