// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// TTZip Zero-Config Native Library Loader for Java 22+ Panama FFM.

package com.ttzip;

import java.io.File;
import java.io.InputStream;
import java.lang.foreign.Arena;
import java.lang.foreign.Linker;
import java.lang.foreign.SymbolLookup;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.Objects;

/**
 * Zero-configuration native shared library loader for TTZip.
 *
 * <p>Supports automatic platform classifier detection, embedded JAR resource
 * extraction ({@code META-INF/natives/{os}-{arch}/}), SHA-256 content-addressable
 * caching in {@code ~/.ttzip/natives/{version}/}, atomic file replacement, and
 * multi-tier fallback resolution with comprehensive diagnostics.
 */
public final class NativeLoader {

    public static final String VERSION = "1.0.0";
    public static final String LIBRARY_BASE_NAME = "ttzip_glue";

    public record Platform(
        String os,
        String arch,
        String classifier,
        String libraryFileName
    ) {}

    public record LoadReport(
        String version,
        Platform platform,
        String sourceType,
        String resolvedPath,
        boolean isCached,
        String status,
        List<String> diagnosticsLog
    ) {
        public String toJson() {
            StringBuilder sb = new StringBuilder();
            sb.append("{\n");
            sb.append("  \"version\": \"").append(escapeJson(version)).append("\",\n");
            sb.append("  \"platform\": {\n");
            sb.append("    \"os\": \"").append(escapeJson(platform.os())).append("\",\n");
            sb.append("    \"arch\": \"").append(escapeJson(platform.arch())).append("\",\n");
            sb.append("    \"classifier\": \"").append(escapeJson(platform.classifier())).append("\",\n");
            sb.append("    \"libraryFileName\": \"").append(escapeJson(platform.libraryFileName())).append("\"\n");
            sb.append("  },\n");
            sb.append("  \"sourceType\": \"").append(escapeJson(sourceType)).append("\",\n");
            sb.append("  \"resolvedPath\": \"").append(escapeJson(resolvedPath)).append("\",\n");
            sb.append("  \"isCached\": ").append(isCached).append(",\n");
            sb.append("  \"status\": \"").append(escapeJson(status)).append("\",\n");
            sb.append("  \"diagnosticsLog\": [\n");
            for (int i = 0; i < diagnosticsLog.size(); i++) {
                sb.append("    \"").append(escapeJson(diagnosticsLog.get(i))).append("\"");
                if (i < diagnosticsLog.size() - 1) {
                    sb.append(",");
                }
                sb.append("\n");
            }
            sb.append("  ]\n");
            sb.append("}");
            return sb.toString();
        }

        private static String escapeJson(String s) {
            if (s == null) return "";
            return s.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n").replace("\r", "\\r");
        }
    }

    private static final Object LOCK = new Object();
    private static volatile SymbolLookup CACHED_LOOKUP = null;
    private static volatile LoadReport LAST_REPORT = null;

    private NativeLoader() {}

    /**
     * Detects current OS and CPU architecture to determine the platform classifier
     * and platform-specific shared library filename.
     */
    public static Platform detectPlatform() {
        String rawOs = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        String rawArch = System.getProperty("os.arch", "").toLowerCase(Locale.ROOT);

        String os;
        String ext;
        String prefix = "lib";

        if (rawOs.contains("mac") || rawOs.contains("darwin") || rawOs.contains("os x")) {
            os = "darwin";
            ext = ".dylib";
        } else if (rawOs.contains("linux")) {
            os = "linux";
            ext = ".so";
        } else if (rawOs.contains("windows") || rawOs.contains("win")) {
            os = "windows";
            ext = ".dll";
            prefix = "";
        } else {
            os = rawOs.replaceAll("[^a-zA-Z0-9]", "_");
            ext = System.mapLibraryName("").replaceAll("^lib|\\.[^.]*$", "");
        }

        String arch;
        if (rawArch.equals("aarch64") || rawArch.equals("arm64")) {
            arch = "aarch64";
        } else if (rawArch.equals("x86_64") || rawArch.equals("amd64") || rawArch.equals("x86-64") || rawArch.equals("x64")) {
            arch = "x86_64";
        } else {
            arch = rawArch.replaceAll("[^a-zA-Z0-9]", "_");
        }

        String classifier = os + "-" + arch;
        String libraryFileName = prefix + LIBRARY_BASE_NAME + ext;

        return new Platform(os, arch, classifier, libraryFileName);
    }

    /**
     * Obtains base cache directory for the current user and platform.
     * Path layout: ~/.ttzip/natives/{version}/{classifier}/
     */
    public static Path getCacheDirectory(String version, String classifier) {
        String userHome = System.getProperty("user.home", ".");
        return Path.of(userHome, ".ttzip", "natives", version, classifier);
    }

    /**
     * Returns the cached SymbolLookup or performs multi-tier fallback loading.
     *
     * @return non-null SymbolLookup
     * @throws UnsatisfiedLinkError if all loading tiers fail
     */
    public static SymbolLookup load() {
        SymbolLookup lookup = CACHED_LOOKUP;
        if (lookup != null) {
            return lookup;
        }

        synchronized (LOCK) {
            if (CACHED_LOOKUP != null) {
                return CACHED_LOOKUP;
            }

            List<String> diagnostics = new ArrayList<>();
            Platform platform = detectPlatform();
            diagnostics.add(String.format("Platform detected: %s (os=%s, arch=%s, libraryFile=%s)",
                platform.classifier(), platform.os(), platform.arch(), platform.libraryFileName()));

            // Tier 1: Explicit System Property (-Dttzip.lib.path=/path/to/lib)
            String sysProp = System.getProperty("ttzip.lib.path");
            if (sysProp != null && !sysProp.isBlank()) {
                Path propPath = Path.of(sysProp.trim());
                diagnostics.add("Tier 1 [system_property]: Checking 'ttzip.lib.path'=" + propPath);
                if (Files.exists(propPath)) {
                    if (tryLoadFile(propPath, "system_property", false, platform, diagnostics)) {
                        return CACHED_LOOKUP;
                    }
                } else {
                    diagnostics.add("Tier 1 [system_property]: File does not exist at " + propPath);
                }
            } else {
                diagnostics.add("Tier 1 [system_property]: Property 'ttzip.lib.path' not specified");
            }

            // Tier 2: Explicit Environment Variable (TTZIP_LIBRARY_PATH=/path/to/lib)
            String envPath = System.getenv("TTZIP_LIBRARY_PATH");
            if (envPath != null && !envPath.isBlank()) {
                Path envFilePath = Path.of(envPath.trim());
                diagnostics.add("Tier 2 [env_variable]: Checking 'TTZIP_LIBRARY_PATH'=" + envFilePath);
                if (Files.exists(envFilePath)) {
                    if (tryLoadFile(envFilePath, "env_variable", false, platform, diagnostics)) {
                        return CACHED_LOOKUP;
                    }
                } else {
                    diagnostics.add("Tier 2 [env_variable]: File does not exist at " + envFilePath);
                }
            } else {
                diagnostics.add("Tier 2 [env_variable]: Environment variable 'TTZIP_LIBRARY_PATH' not set");
            }

            // Tier 3: Embedded JAR Resource (META-INF/natives/{os}-{arch}/{libraryFileName})
            String[] resourcePaths = {
                "/META-INF/natives/" + platform.classifier() + "/" + platform.libraryFileName(),
                "META-INF/natives/" + platform.classifier() + "/" + platform.libraryFileName(),
                "/META-INF/natives/" + platform.libraryFileName(),
                "META-INF/natives/" + platform.libraryFileName()
            };

            InputStream stream = null;
            String matchedResourcePath = null;
            for (String rPath : resourcePaths) {
                stream = NativeLoader.class.getResourceAsStream(rPath);
                if (stream == null && !rPath.startsWith("/")) {
                    stream = NativeLoader.class.getClassLoader().getResourceAsStream(rPath);
                }
                if (stream != null) {
                    matchedResourcePath = rPath;
                    break;
                }
            }

            if (stream != null) {
                diagnostics.add("Tier 3 [embedded_jar_resource]: Found resource at " + matchedResourcePath);
                try (InputStream in = stream) {
                    byte[] resourceBytes = in.readAllBytes();
                    String expectedSha256 = sha256(resourceBytes);
                    Path cacheDir = getCacheDirectory(VERSION, platform.classifier());
                    Path targetFile = cacheDir.resolve(platform.libraryFileName());

                    boolean cacheHit = false;
                    if (Files.exists(targetFile)) {
                        byte[] cachedBytes = Files.readAllBytes(targetFile);
                        String cachedSha256 = sha256(cachedBytes);
                        if (cachedSha256.equalsIgnoreCase(expectedSha256)) {
                            cacheHit = true;
                            diagnostics.add("Tier 3 [embedded_jar_resource]: Cache hit at " + targetFile + " (SHA-256: " + expectedSha256.substring(0, 8) + "...)");
                        } else {
                            diagnostics.add("Tier 3 [embedded_jar_resource]: Cache mismatch, refreshing target: " + targetFile);
                        }
                    }

                    if (!cacheHit) {
                        atomicWriteAndReplace(targetFile, resourceBytes);
                        diagnostics.add("Tier 3 [embedded_jar_resource]: Extracted and cached binary to " + targetFile + " (SHA-256: " + expectedSha256.substring(0, 8) + "...)");
                    }

                    if (tryLoadFile(targetFile, "embedded_jar_resource", cacheHit, platform, diagnostics)) {
                        return CACHED_LOOKUP;
                    }
                } catch (Exception e) {
                    diagnostics.add("Tier 3 [embedded_jar_resource]: Extraction failed: " + e.getMessage());
                }
            } else {
                diagnostics.add("Tier 3 [embedded_jar_resource]: No embedded native library found in JAR for " + platform.classifier());
            }

            // Tier 4: Local Dev Workspace / Relative Path search
            List<Path> devCandidates = getDevCandidatePaths(platform.libraryFileName());
            diagnostics.add("Tier 4 [dev_workspace]: Searching " + devCandidates.size() + " development workspace locations");
            for (Path candidate : devCandidates) {
                if (Files.exists(candidate)) {
                    diagnostics.add("Tier 4 [dev_workspace]: Candidate found at " + candidate.toAbsolutePath());
                    if (tryLoadFile(candidate.toAbsolutePath(), "dev_workspace", false, platform, diagnostics)) {
                        return CACHED_LOOKUP;
                    }
                }
            }

            // Tier 5: System Path / java.library.path fallback (System.loadLibrary)
            diagnostics.add("Tier 5 [system_path]: Attempting System.loadLibrary('" + LIBRARY_BASE_NAME + "')");
            try {
                System.loadLibrary(LIBRARY_BASE_NAME);
                SymbolLookup loaderLookup = SymbolLookup.loaderLookup();
                if (loaderLookup.find("ttzip_rust_version").isPresent()) {
                    CACHED_LOOKUP = loaderLookup.or(Linker.nativeLinker().defaultLookup());
                    LAST_REPORT = new LoadReport(
                        VERSION,
                        platform,
                        "system_path",
                        "system_library_path:" + LIBRARY_BASE_NAME,
                        false,
                        "LOADED",
                        Collections.unmodifiableList(diagnostics)
                    );
                    diagnostics.add("Tier 5 [system_path]: Successfully loaded via System.loadLibrary");
                    return CACHED_LOOKUP;
                }
            } catch (Throwable t) {
                diagnostics.add("Tier 5 [system_path]: System.loadLibrary failed: " + t.getMessage());
            }

            // All tiers failed - build diagnostic report and throw UnsatisfiedLinkError
            LAST_REPORT = new LoadReport(
                VERSION,
                platform,
                "none",
                "unresolved",
                false,
                "FAILED",
                Collections.unmodifiableList(diagnostics)
            );

            StringBuilder msg = new StringBuilder();
            msg.append("Failed to load TTZip native shared library [").append(platform.libraryFileName()).append("] for platform ")
               .append(platform.classifier()).append(".\n")
               .append("--------------------------------------------------------------------------------\n")
               .append("Resolution Steps:\n")
               .append("  1. Ensure the JAR contains '/META-INF/natives/").append(platform.classifier()).append("/").append(platform.libraryFileName()).append("'\n")
               .append("  2. Or set the JVM property: -Dttzip.lib.path=/absolute/path/to/").append(platform.libraryFileName()).append("\n")
               .append("  3. Or set the environment variable: export TTZIP_LIBRARY_PATH=/absolute/path/to/").append(platform.libraryFileName()).append("\n")
               .append("--------------------------------------------------------------------------------\n")
               .append("Diagnostic Trace:\n");
            for (String diag : diagnostics) {
                msg.append("  * ").append(diag).append("\n");
            }
            msg.append("--------------------------------------------------------------------------------");

            throw new UnsatisfiedLinkError(msg.toString());
        }
    }

    /**
     * Atomically writes payload to target file using a unique temporary file and atomic replace.
     */
    public static void atomicWriteAndReplace(Path targetFile, byte[] bytes) throws Exception {
        Path parent = targetFile.getParent();
        if (parent != null) {
            Files.createDirectories(parent);
        }

        Path tempFile = Files.createTempFile(
            parent != null ? parent : Path.of("."),
            ".ttzip_extract_",
            ".tmp"
        );

        try {
            Files.write(tempFile, bytes);
            trySetExecutable(tempFile);

            try {
                Files.move(tempFile, targetFile, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
            } catch (AtomicMoveNotSupportedException e) {
                Files.move(tempFile, targetFile, StandardCopyOption.REPLACE_EXISTING);
            }
            trySetExecutable(targetFile);
        } finally {
            if (Files.exists(tempFile)) {
                try {
                    Files.deleteIfExists(tempFile);
                } catch (Exception ignored) {}
            }
        }
    }

    private static void trySetExecutable(Path path) {
        try {
            File f = path.toFile();
            f.setExecutable(true, false);
            f.setReadable(true, false);
        } catch (Exception ignored) {}
    }

    private static boolean tryLoadFile(Path path, String sourceType, boolean isCached, Platform platform, List<String> diagnostics) {
        try {
            String absPath = path.toAbsolutePath().normalize().toString();
            System.load(absPath);
            SymbolLookup libLookup = SymbolLookup.libraryLookup(Path.of(absPath), Arena.global());
            SymbolLookup combined = libLookup.or(SymbolLookup.loaderLookup()).or(Linker.nativeLinker().defaultLookup());

            if (combined.find("ttzip_rust_version").isEmpty()) {
                diagnostics.add(sourceType + ": Library loaded from " + absPath + " but 'ttzip_rust_version' symbol was not found");
                return false;
            }

            CACHED_LOOKUP = combined;
            LAST_REPORT = new LoadReport(
                VERSION,
                platform,
                sourceType,
                absPath,
                isCached,
                "LOADED",
                Collections.unmodifiableList(diagnostics)
            );
            diagnostics.add(sourceType + ": Successfully loaded and verified from " + absPath);
            return true;
        } catch (Throwable t) {
            diagnostics.add(sourceType + ": Failed loading from " + path + ": " + t.getMessage());
            return false;
        }
    }

    private static List<Path> getDevCandidatePaths(String libName) {
        List<Path> candidates = new ArrayList<>();
        String[] relativeBases = {
            "core/rust/target/release",
            "rust/target/release",
            "../rust/target/release",
            "../../rust/target/release",
            "../../../rust/target/release",
            "../core/rust/target/release",
            "../../core/rust/target/release",
            "core/rust/target/debug",
            "rust/target/debug",
            "../rust/target/debug",
            "../../rust/target/debug"
        };

        Path cwd = Path.of("").toAbsolutePath();
        for (String base : relativeBases) {
            candidates.add(cwd.resolve(base).resolve(libName));
        }

        // Also search user.dir if different
        String userDir = System.getProperty("user.dir");
        if (userDir != null && !userDir.isBlank()) {
            Path uDir = Path.of(userDir).toAbsolutePath();
            for (String base : relativeBases) {
                Path p = uDir.resolve(base).resolve(libName);
                if (!candidates.contains(p)) {
                    candidates.add(p);
                }
            }
        }

        return candidates;
    }

    /**
     * Calculates SHA-256 hexadecimal checksum of byte array.
     */
    public static String sha256(byte[] data) {
        Objects.requireNonNull(data, "data cannot be null");
        try {
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            byte[] hash = digest.digest(data);
            StringBuilder hexString = new StringBuilder(hash.length * 2);
            for (byte b : hash) {
                String hex = Integer.toHexString(0xff & b);
                if (hex.length() == 1) {
                    hexString.append('0');
                }
                hexString.append(hex);
            }
            return hexString.toString();
        } catch (NoSuchAlgorithmException e) {
            throw new RuntimeException("SHA-256 digest algorithm not available", e);
        }
    }

    /**
     * Returns the latest LoadReport generated during loading.
     */
    public static LoadReport getReport() {
        if (LAST_REPORT == null) {
            load();
        }
        return LAST_REPORT;
    }
}
