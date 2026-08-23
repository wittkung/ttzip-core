// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.

package com.ttzip

import java.io.File
import java.nio.file.Path

/**
 * Idiomatic Kotlin extensions and DSL builders for TTZip.
 */
fun File.ttzipCompress(destination: File) {
    TTZip.compress(listOf(this.absolutePath), destination.absolutePath)
}

fun File.ttzipExtract(destinationDirectory: File) {
    TTZip.extract(this.absolutePath, destinationDirectory.absolutePath)
}

fun Path.ttzipCompress(destination: Path) {
    TTZip.compress(listOf(this.toAbsolutePath().toString()), destination.toAbsolutePath().toString())
}

fun Path.ttzipExtract(destinationDirectory: Path) {
    TTZip.extract(this.toAbsolutePath().toString(), destinationDirectory.toAbsolutePath().toString())
}

fun ByteArray.ttzipCrc32(): Int = TTZip.crc32(this)
