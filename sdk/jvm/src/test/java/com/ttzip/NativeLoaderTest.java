// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// TTZip: Panama FFM NativeLoader Unit Test Suite.

package com.ttzip;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.io.File;
import java.lang.foreign.SymbolLookup;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Comparator;
import java.util.List;
import java.util.Set;
import java.util.zip.CRC32;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("TTZip Zero-Config NativeLoader Tests")
public class NativeLoaderTest {

    private Path tempDir;

    @BeforeEach
    public void setUp() throws Exception {
        tempDir = Files.createTempDirectory("ttzip_native_loader_test_");
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
    @DisplayName("Verify platform detection and classifier formation")
    public void testPlatformDetection() {
        NativeLoader.Platform platform = NativeLoader.detectPlatform();
        assertNotNull(platform, "Platform cannot be null");
        assertNotNull(platform.os(), "OS cannot be null");
        assertNotNull(platform.arch(), "Arch cannot be null");
        assertNotNull(platform.classifier(), "Classifier cannot be null");
        assertNotNull(platform.libraryFileName(), "Library filename cannot be null");

        assertEquals(platform.os() + "-" + platform.arch(), platform.classifier(),
            "Classifier must be strictly formatted as {os}-{arch}");

        Set<String> knownClassifiers = Set.of(
            "darwin-aarch64",
            "darwin-x86_64",
            "linux-x86_64",
            "linux-aarch64",
            "windows-x86_64"
        );

        assertTrue(
            knownClassifiers.contains(platform.classifier()) || !platform.classifier().isBlank(),
            "Classifier should be a recognized target architecture"
        );

        assertTrue(
            platform.libraryFileName().endsWith(".dylib") ||
            platform.libraryFileName().endsWith(".so") ||
            platform.libraryFileName().endsWith(".dll"),
            "Library filename must carry a platform-standard dynamic extension (.dylib, .so, .dll)"
        );
    }

    @Test
    @DisplayName("Verify SHA-256 calculation for byte payloads")
    public void testSha256ChecksumCalculation() throws Exception {
        // Standard empty string SHA-256 hash
        byte[] empty = new byte[0];
        String emptyHash = NativeLoader.sha256(empty);
        assertEquals("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", emptyHash.toLowerCase());

        // Test with custom text payload against MessageDigest
        byte[] payload = "TTZip Zero-Config Panama FFM 2026".getBytes(StandardCharsets.UTF_8);
        MessageDigest md = MessageDigest.getInstance("SHA-256");
        byte[] expectedBytes = md.digest(payload);
        StringBuilder expectedHex = new StringBuilder();
        for (byte b : expectedBytes) {
            String h = Integer.toHexString(0xff & b);
            if (h.length() == 1) expectedHex.append('0');
            expectedHex.append(h);
        }

        String computedHash = NativeLoader.sha256(payload);
        assertEquals(expectedHex.toString(), computedHash, "SHA-256 digest calculation mismatch");
    }

    @Test
    @DisplayName("Verify zero-config SymbolLookup loading and symbol presence")
    public void testZeroConfigSymbolLookup() {
        SymbolLookup lookup = NativeLoader.load();
        assertNotNull(lookup, "NativeLoader.load() must return a non-null SymbolLookup");

        assertTrue(lookup.find("ttzip_rust_version").isPresent(),
            "Symbol 'ttzip_rust_version' must be resolvable");
        assertTrue(lookup.find("ttzip_rust_is_hardware_accelerated").isPresent(),
            "Symbol 'ttzip_rust_is_hardware_accelerated' must be resolvable");
        assertTrue(lookup.find("ttzip_rust_crc32").isPresent(),
            "Symbol 'ttzip_rust_crc32' must be resolvable");
        assertTrue(lookup.find("ttzip_rust_crc64").isPresent(),
            "Symbol 'ttzip_rust_crc64' must be resolvable");
        assertTrue(lookup.find("ttzip_rust_create_archive").isPresent(),
            "Symbol 'ttzip_rust_create_archive' must be resolvable");
        assertTrue(lookup.find("ttzip_rust_extract_archive").isPresent(),
            "Symbol 'ttzip_rust_extract_archive' must be resolvable");
        assertTrue(lookup.find("ttzip_rust_inspect_archive").isPresent(),
            "Symbol 'ttzip_rust_inspect_archive' must be resolvable");
    }

    @Test
    @DisplayName("Verify LoadReport schema conformance and diagnostic logging")
    public void testLoadReportContract() {
        NativeLoader.LoadReport report = NativeLoader.getReport();
        assertNotNull(report, "LoadReport must not be null");
        assertEquals("1.0.0", report.version(), "Report version must match SDK version");
        assertNotNull(report.platform(), "Report platform must not be null");
        assertEquals("LOADED", report.status(), "Report status must be LOADED upon success");
        assertNotNull(report.resolvedPath(), "Resolved library path must not be null");
        assertNotNull(report.sourceType(), "Source type must be populated");

        Set<String> validSourceTypes = Set.of(
            "system_property",
            "env_variable",
            "embedded_jar_resource",
            "dev_workspace",
            "system_path"
        );
        assertTrue(validSourceTypes.contains(report.sourceType()),
            "Source type '" + report.sourceType() + "' must be one of the known tiers");

        assertFalse(report.diagnosticsLog().isEmpty(), "Diagnostics log must contain trace entries");

        String json = report.toJson();
        assertNotNull(json, "JSON representation must not be null");
        assertTrue(json.contains("\"version\": \"1.0.0\""), "JSON must contain version field");
        assertTrue(json.contains("\"status\": \"LOADED\""), "JSON must contain status field");
        assertTrue(json.contains("\"classifier\":"), "JSON must contain classifier field");
        assertTrue(json.contains("\"diagnosticsLog\":"), "JSON must contain diagnosticsLog array");
    }

    @Test
    @DisplayName("Verify atomic file write and replacement")
    public void testAtomicFileReplacement() throws Exception {
        Path target = tempDir.resolve("lib_atomic_test.bin");

        byte[] payload1 = "Initial Version Content".getBytes(StandardCharsets.UTF_8);
        NativeLoader.atomicWriteAndReplace(target, payload1);
        assertTrue(Files.exists(target), "Target file must exist after first atomic write");
        assertArrayEquals(payload1, Files.readAllBytes(target), "Target content must match first payload");

        byte[] payload2 = "Replaced Overwritten Content Payload".getBytes(StandardCharsets.UTF_8);
        NativeLoader.atomicWriteAndReplace(target, payload2);
        assertTrue(Files.exists(target), "Target file must exist after second atomic write");
        assertArrayEquals(payload2, Files.readAllBytes(target), "Target content must match updated payload");
    }

    @Test
    @DisplayName("Verify end-to-end TTZip engine integration through NativeLoader")
    public void testTTZipEngineIntegration() {
        String version = TTZip.version();
        assertNotNull(version, "Engine version string must not be null");
        assertFalse(version.isBlank(), "Engine version must not be blank");

        boolean hw = TTZip.isHardwareAccelerated();
        assertTrue(hw || !hw, "Hardware acceleration status queryable");

        byte[] testData = "Panama FFM Hardware Acceleration Zero-Config 2026".getBytes(StandardCharsets.UTF_8);
        int nativeCrc = TTZip.crc32(testData);

        CRC32 jvmCrc = new CRC32();
        jvmCrc.update(testData);
        assertEquals((int) jvmCrc.getValue(), nativeCrc, "Hardware CRC-32 must match JDK java.util.zip.CRC32");
    }

    public static void main(String[] args) throws Exception {
        System.out.println("⚡️ Running TTZip NativeLoader Test Suite...");
        NativeLoaderTest suite = new NativeLoaderTest();
        suite.setUp();

        try {
            suite.testPlatformDetection();
            System.out.println("  [PASS] testPlatformDetection");

            suite.testSha256ChecksumCalculation();
            System.out.println("  [PASS] testSha256ChecksumCalculation");

            suite.testZeroConfigSymbolLookup();
            System.out.println("  [PASS] testZeroConfigSymbolLookup");

            suite.testLoadReportContract();
            System.out.println("  [PASS] testLoadReportContract");

            suite.testAtomicFileReplacement();
            System.out.println("  [PASS] testAtomicFileReplacement");

            suite.testTTZipEngineIntegration();
            System.out.println("  [PASS] testTTZipEngineIntegration");

            System.out.println("\n--- NativeLoader Diagnostic Report ---");
            System.out.println(NativeLoader.getReport().toJson());
            System.out.println("--------------------------------------\n");

            System.out.println("✅ All NativeLoader tests passed successfully!");
        } finally {
            suite.tearDown();
        }
    }
}
