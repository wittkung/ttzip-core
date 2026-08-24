// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip Kotlin Coroutines & Flow Extensions for Java 22+ Panama FFM.

package com.ttzip

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.withContext
import java.io.File
import java.lang.foreign.Arena
import java.lang.foreign.MemorySegment
import java.nio.file.Path

/**
 * Emits real-time [TTZip.ArchiveProgress] events during archive compression of a single file.
 */
fun File.ttzipCompressFlow(
    destination: File,
    format: TTZip.ArchiveFormat = TTZip.ArchiveFormat.AUTO,
    level: TTZip.CompressionLevel = TTZip.CompressionLevel.NORMAL,
    password: String? = null,
    threads: Int = 0
): Flow<TTZip.ArchiveProgress> = listOf(this).ttzipCompressFlow(destination, format, level, password, threads)

/**
 * Emits real-time [TTZip.ArchiveProgress] events during archive compression of multiple files.
 */
fun List<File>.ttzipCompressFlow(
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
        val paths = this@ttzipCompressFlow.map { it.absolutePath }
        TTZip.compress(
            paths,
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

    awaitClose { /* Cleanup channel resources */ }
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

    awaitClose { /* Cleanup channel resources */ }
}

/**
 * Path extension for streaming single-file compression progress.
 */
fun Path.ttzipCompressFlow(
    destination: Path,
    format: TTZip.ArchiveFormat = TTZip.ArchiveFormat.AUTO,
    level: TTZip.CompressionLevel = TTZip.CompressionLevel.NORMAL,
    password: String? = null,
    threads: Int = 0
): Flow<TTZip.ArchiveProgress> = this.toFile().ttzipCompressFlow(destination.toFile(), format, level, password, threads)

/**
 * Path extension for streaming multi-path compression progress.
 */
fun List<Path>.ttzipCompressPathsFlow(
    destination: Path,
    format: TTZip.ArchiveFormat = TTZip.ArchiveFormat.AUTO,
    level: TTZip.CompressionLevel = TTZip.CompressionLevel.NORMAL,
    password: String? = null,
    threads: Int = 0
): Flow<TTZip.ArchiveProgress> = this.map { it.toFile() }.ttzipCompressFlow(destination.toFile(), format, level, password, threads)

/**
 * Path extension for streaming extraction progress.
 */
fun Path.ttzipExtractFlow(
    destinationDirectory: Path,
    password: String? = null,
    threads: Int = 0
): Flow<TTZip.ArchiveProgress> = this.toFile().ttzipExtractFlow(destinationDirectory.toFile(), password, threads)

/**
 * Suspending non-blocking single-file compression offloaded to Dispatchers.IO.
 */
suspend fun File.ttzipCompress(
    destination: File,
    format: TTZip.ArchiveFormat = TTZip.ArchiveFormat.AUTO,
    level: TTZip.CompressionLevel = TTZip.CompressionLevel.NORMAL,
    password: String? = null,
    threads: Int = 0,
    listener: TTZip.ProgressListener? = null
) = withContext(Dispatchers.IO) {
    TTZip.compress(
        listOf(this@ttzipCompress.absolutePath),
        destination.absolutePath,
        format,
        level,
        password,
        threads,
        listener
    )
}

/**
 * Suspending non-blocking multi-file compression offloaded to Dispatchers.IO.
 */
suspend fun List<File>.ttzipCompress(
    destination: File,
    format: TTZip.ArchiveFormat = TTZip.ArchiveFormat.AUTO,
    level: TTZip.CompressionLevel = TTZip.CompressionLevel.NORMAL,
    password: String? = null,
    threads: Int = 0,
    listener: TTZip.ProgressListener? = null
) = withContext(Dispatchers.IO) {
    TTZip.compress(
        this@ttzipCompress.map { it.absolutePath },
        destination.absolutePath,
        format,
        level,
        password,
        threads,
        listener
    )
}

/**
 * Suspending non-blocking extraction offloaded to Dispatchers.IO.
 */
suspend fun File.ttzipExtract(
    destinationDirectory: File,
    password: String? = null,
    threads: Int = 0,
    listener: TTZip.ProgressListener? = null
) = withContext(Dispatchers.IO) {
    TTZip.extract(
        this@ttzipExtract.absolutePath,
        destinationDirectory.absolutePath,
        password,
        threads,
        listener
    )
}

/**
 * Suspending non-blocking compression for Path offloaded to Dispatchers.IO.
 */
suspend fun Path.ttzipCompress(
    destination: Path,
    format: TTZip.ArchiveFormat = TTZip.ArchiveFormat.AUTO,
    level: TTZip.CompressionLevel = TTZip.CompressionLevel.NORMAL,
    password: String? = null,
    threads: Int = 0,
    listener: TTZip.ProgressListener? = null
) = this.toFile().ttzipCompress(destination.toFile(), format, level, password, threads, listener)

/**
 * Suspending non-blocking extraction for Path offloaded to Dispatchers.IO.
 */
suspend fun Path.ttzipExtract(
    destinationDirectory: Path,
    password: String? = null,
    threads: Int = 0,
    listener: TTZip.ProgressListener? = null
) = this.toFile().ttzipExtract(destinationDirectory.toFile(), password, threads, listener)

/**
 * Inspects archive metadata entries without disk extraction.
 */
fun File.ttzipInspect(password: String? = null): List<TTZip.EntryMetadata> {
    return TTZip.inspect(this.absolutePath, password)
}

/**
 * Inspects archive metadata entries for Path without disk extraction.
 */
fun Path.ttzipInspect(password: String? = null): List<TTZip.EntryMetadata> {
    return TTZip.inspect(this.toAbsolutePath().toString(), password)
}

/**
 * Computes SIMD-accelerated CRC-32 on byte array.
 */
fun ByteArray.ttzipCrc32(): Int = TTZip.crc32(this)

/**
 * Computes SIMD-accelerated CRC-64 on byte array.
 */
fun ByteArray.ttzipCrc64(): Long {
    Arena.ofConfined().use { arena ->
        val seg = arena.allocate(this.size.toLong())
        MemorySegment.copy(
            MemorySegment.ofArray(this), 0,
            seg, 0, this.size.toLong()
        )
        return TTZip.crc64(seg, 0L)
    }
}
