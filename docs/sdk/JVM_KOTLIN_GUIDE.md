# ☕️ TTZip JVM & Kotlin Developer Guide

[![Java 22+](https://img.shields.io/badge/Java-22%2B%20Panama%20FFM-orange.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/sdk/jvm/src/main/java/com/ttzip/TTZip.java)
[![Kotlin 2.0+](https://img.shields.io/badge/Kotlin-2.0%2B%20Flow%20%26%20Coroutines-purple.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/sdk/jvm/src/main/kotlin/com/ttzip/TTZipExtensions.kt)
[![Zero-JNI](https://img.shields.io/badge/Architecture-Zero--JNI%20%2F%20Zero--Subprocess-brightgreen.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/BENCHMARK_MATRIX.md)

`com.ttzip:ttzip-core` is the official Java 22+ and Kotlin SDK for TTZip. It completely eliminates legacy JNI overhead and slow subprocess calls by utilizing the **OpenJDK Project Panama Foreign Function & Memory (FFM) API** (`java.lang.foreign`), delivering native microsecond latencies and up to **4.47 GB/s** decompression throughput.

---

## 1. Maven & Gradle Setup

### Gradle (`build.gradle.kts`)

```kotlin
plugins {
    id("java")
    kotlin("jvm") version "2.0.0"
}

repositories {
    mavenCentral()
}

dependencies {
    implementation("com.ttzip:ttzip-core:1.0.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.0")
    testImplementation("org.junit.jupiter:junit-jupiter:5.10.2")
}

tasks.withType<JavaExec> {
    jvmArgs("--enable-native-access=ALL-UNNAMED")
}
```

### Maven (`pom.xml`)

```xml
<dependencies>
    <dependency>
        <groupId>com.ttzip</groupId>
        <artifactId>ttzip-core</artifactId>
        <version>1.0.0</version>
    </dependency>
</dependencies>

<build>
    <plugins>
        <plugin>
            <groupId>org.apache.maven.plugins</groupId>
            <artifactId>maven-compiler-plugin</artifactId>
            <version>3.13.0</version>
            <configuration>
                <release>22</release>
                <compilerArgs>
                    <arg>--enable-native-access=ALL-UNNAMED</arg>
                </compilerArgs>
            </configuration>
        </plugin>
    </plugins>
</build>
```

---

## 2. Project Panama FFM Architecture

TTZip binds downcalls directly to `libttzip_engine.dylib` / `.so` / `.dll` using scoped `Arena` allocators:

```
┌────────────────────────────────────────────────────────┐
│                   JVM / Kotlin Application             │
│            (Spring Boot · Netty · Android / Server)     │
└───────────────────────────┬────────────────────────────┘
                            │ DowncallHandle.invokeExact()
┌───────────────────────────▼────────────────────────────┐
│              Panama Foreign Function & Memory          │
│   - Arena.ofConfined(): Deterministic stack memory     │
│   - StructLayout: MemoryLayout for C structs           │
│   - Linker.nativeLinker(): Zero-JNI direct calling     │
└───────────────────────────┬────────────────────────────┘
                            │ Raw Memory Pointer Exchange
┌───────────────────────────▼────────────────────────────┐
│           Safe Rust Microkernel (libttzip_engine)      │
└────────────────────────────────────────────────────────┘
```

---

## 3. Java 22+ Code Examples

### 3.1 Synchronous & Progress-Monitored Compression

```java
package com.example;

import com.ttzip.TTZip;
import com.ttzip.TTZip.ArchiveFormat;
import com.ttzip.TTZip.CompressionLevel;
import java.util.List;

public class ArchiveService {
    public static void main(String[] args) {
        List<String> sources = List.of(
            "/data/logs/server.log",
            "/data/database/dump.sql"
        );
        String destination = "/data/backup/daily_backup.7z";

        System.out.println("Starting TTZip multi-core compression (Java Panama FFM)...");

        TTZip.compress(
            sources,
            destination,
            ArchiveFormat.SEVEN_ZIP,
            CompressionLevel.NORMAL,
            "SecurePassword2026!",
            0, // Auto-detect threads
            progress -> {
                System.out.printf("[%5.1f%%] Current file: %s (Processed: %d / %d bytes)%n",
                    progress.fractionCompleted() * 100.0,
                    progress.currentEntryPath(),
                    progress.processedBytes(),
                    progress.totalBytes()
                );
                return true; // Return false to cancel
            }
        );

        System.out.println("Backup completed successfully!");
    }
}
```

### 3.2 Safe Archive Extraction (Zip-Slip Immune)

```java
package com.example;

import com.ttzip.TTZip;

public class ExtractionService {
    public static void extractFile(String archivePath, String destinationDir, String password) {
        System.out.println("Extracting archive: " + archivePath);

        TTZip.extract(
            archivePath,
            destinationDir,
            password,
            0,
            progress -> {
                System.out.printf("Extracting: %s%n", progress.currentEntryPath());
                return true;
            }
        );

        System.out.println("Extraction finished to: " + destinationDir);
    }
}
```

### 3.3 Inspecting Archive Metadata

```java
package com.example;

import com.ttzip.TTZip;
import com.ttzip.TTZip.EntryMetadata;
import java.util.List;

public class InspectionService {
    public static void printArchiveContents(String archivePath) {
        List<EntryMetadata> entries = TTZip.inspect(archivePath, null);

        System.out.printf("Found %d entries in %s:%n", entries.size(), archivePath);
        for (EntryMetadata entry : entries) {
            System.out.printf("  - %-30s | %10d bytes | CRC: %08X | Dir: %b%n",
                entry.path(),
                entry.uncompressedSize(),
                entry.crc32(),
                entry.isDirectory()
            );
        }
    }
}
```

---

## 4. Kotlin Flow & Coroutines Extensions

`TTZipExtensions.kt` provides reactive `Flow` and suspending non-blocking functions offloaded to `Dispatchers.IO`:

### 4.1 Reactive Progress Streaming with Kotlin Flow

```kotlin
package com.example

import com.ttzip.TTZip
import com.ttzip.ttzipCompressFlow
import com.ttzip.ttzipExtractFlow
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.runBlocking
import java.io.File

fun main() = runBlocking {
    val sourceFiles = listOf(File("/data/documents"), File("/data/photos"))
    val destinationArchive = File("/data/export/archive.zip")

    // Stream compression progress reactively
    sourceFiles.ttzipCompressFlow(
        destination = destinationArchive,
        format = TTZip.ArchiveFormat.ZIP,
        level = TTZip.CompressionLevel.NORMAL
    ).collect { progress ->
        val pct = (progress.fractionCompleted * 100).toInt()
        println("[$pct%] Compressing: ${progress.currentEntryPath}")
    }

    println("Reactive compression complete!")

    // Stream extraction progress
    val outputDir = File("/data/extracted")
    destinationArchive.ttzipExtractFlow(destinationDirectory = outputDir).collect { progress ->
        println("Extracted: ${progress.currentEntryPath}")
    }
}
```

### 4.2 Non-Blocking Suspending Functions

```kotlin
package com.example

import com.ttzip.ttzipCompress
import com.ttzip.ttzipExtract
import com.ttzip.ttzipInspect
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

suspend fun handleUserUpload(uploadedZip: File, targetDir: File) {
    // 1. Inspect without disk extraction
    val metadata = uploadedZip.ttzipInspect()
    println("Archive contains ${metadata.size} entries.")

    // 2. Non-blocking extraction on Dispatchers.IO
    uploadedZip.ttzipExtract(destinationDirectory = targetDir)
    println("Extracted archive asynchronously.")
}
```

---

## 5. SIMD-Accelerated Checksums on Panama MemorySegments

Directly hash JVM byte arrays or off-heap `MemorySegment` buffers at hardware speeds (>40 GB/s):

```java
import com.ttzip.TTZip;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;

public class ChecksumDemo {
    public static void main(String[] args) {
        byte[] payload = "High-Performance Java 22 Panama FFM Checksum Payload".getBytes();

        // 1. Array-based CRC-32
        int crcVal = TTZip.crc32(payload);
        System.out.printf("CRC-32: %08X%n", crcVal);

        // 2. Off-heap MemorySegment CRC-64
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment segment = arena.allocateFrom("Panama Off-Heap String Buffer");
            long crc64Val = TTZip.crc64(segment, 0L);
            System.out.printf("CRC-64: %016X%n", crc64Val);
        }
    }
}
```

---

## 6. Runtime Diagnostics & Hardware Sensing

```java
import com.ttzip.TTZip;

public class Diagnostics {
    public static void main(String[] args) {
        System.out.println("TTZip Engine Version: " + TTZip.version());
        System.out.println("Hardware SIMD Acceleration Active: " + TTZip.isHardwareAccelerated());
    }
}
```
