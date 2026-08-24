// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Java 22+.
// Production-grade Foreign Function & Memory (FFM) API binding (Zero Subprocess / Zero JNI).

package com.ttzip;

import java.io.File;
import java.lang.foreign.*;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.function.Consumer;

/**
 * TTZip High-Throughput Archiving & Compression Engine for Java 22+.
 * Uses Panama Foreign Function & Memory (FFM) API with confined arenas for zero-copy interop.
 */
public final class TTZip {

    public static final int ABI_VERSION_2 = 2;

    public enum ArchiveFormat {
        AUTO(0),
        ZIP(1),
        SEVEN_ZIP(2),
        TAR(3),
        TAR_GZ(4),
        TAR_BZ2(5),
        TAR_XZ(6),
        TAR_ZSTD(7),
        DMG(8),
        LZFSE(9),
        SNAPPY(10);

        public final int code;
        ArchiveFormat(int code) { this.code = code; }
    }

    public enum CompressionLevel {
        STORE(0),
        FASTEST(1),
        FAST(3),
        NORMAL(6),
        MAXIMUM(9),
        ULTRA(12);

        public final int level;
        CompressionLevel(int level) { this.level = level; }
    }

    public record EntryMetadata(
        String path,
        long uncompressedSize,
        long compressedSize,
        int crc32,
        long mtimeEpochSecs,
        boolean isDirectory,
        boolean isEncrypted
    ) {}

    public record ArchiveProgress(
        long processedBytes,
        long totalBytes,
        double fractionCompleted,
        String currentEntryPath,
        int currentEntryIndex,
        int totalEntries,
        String phase,
        double throughputMbs
    ) {}

    @FunctionalInterface
    public interface ProgressListener {
        boolean onProgress(ArchiveProgress progress);
    }

    // FFM Linker and Downcall Handles
    private static final Linker LINKER = Linker.nativeLinker();
    private static final SymbolLookup LOOKUP;

    private static final MethodHandle MH_VERSION;
    private static final MethodHandle MH_IS_HARDWARE_ACCELERATED;
    private static final MethodHandle MH_CRC32;
    private static final MethodHandle MH_CRC64;
    private static final MethodHandle MH_CREATE_ARCHIVE;
    private static final MethodHandle MH_EXTRACT_ARCHIVE;
    private static final MethodHandle MH_INSPECT_ARCHIVE;

    // Struct Layouts matching ttzip_rust_glue.h & types.rs
    public static final StructLayout CREATE_OPTIONS_LAYOUT = MemoryLayout.structLayout(
        ValueLayout.JAVA_INT.withName("struct_size"),
        ValueLayout.JAVA_INT.withName("abi_version"),
        ValueLayout.JAVA_INT.withName("format"),
        ValueLayout.JAVA_INT.withName("level"),
        ValueLayout.JAVA_INT.withName("encryption"),
        MemoryLayout.paddingLayout(4),
        ValueLayout.ADDRESS.withName("password"),
        ValueLayout.JAVA_INT.withName("thread_budget"),
        ValueLayout.JAVA_INT.withName("solid_block_size_mb"),
        ValueLayout.ADDRESS.withName("progress_callback"),
        ValueLayout.ADDRESS.withName("user_data")
    );

    public static final StructLayout EXTRACT_OPTIONS_LAYOUT = MemoryLayout.structLayout(
        ValueLayout.JAVA_INT.withName("struct_size"),
        ValueLayout.JAVA_INT.withName("abi_version"),
        ValueLayout.ADDRESS.withName("destination_path"),
        ValueLayout.ADDRESS.withName("password"),
        ValueLayout.JAVA_INT.withName("thread_budget"),
        ValueLayout.JAVA_BOOLEAN.withName("overwrite_existing"),
        ValueLayout.JAVA_BOOLEAN.withName("preserve_permissions"),
        ValueLayout.JAVA_BOOLEAN.withName("dry_run"),
        MemoryLayout.paddingLayout(1),
        ValueLayout.ADDRESS.withName("progress_callback"),
        ValueLayout.ADDRESS.withName("user_data")
    );

    public static final StructLayout ENTRY_METADATA_LAYOUT = MemoryLayout.structLayout(
        ValueLayout.JAVA_INT.withName("struct_size"),
        ValueLayout.JAVA_INT.withName("abi_version"),
        ValueLayout.ADDRESS.withName("path"),
        ValueLayout.JAVA_LONG.withName("uncompressed_size"),
        ValueLayout.JAVA_LONG.withName("compressed_size"),
        ValueLayout.JAVA_INT.withName("crc32"),
        MemoryLayout.paddingLayout(4),
        ValueLayout.JAVA_LONG.withName("mtime_epoch_secs"),
        ValueLayout.JAVA_INT.withName("mode"),
        ValueLayout.JAVA_BOOLEAN.withName("is_directory"),
        ValueLayout.JAVA_BOOLEAN.withName("is_encrypted"),
        ValueLayout.JAVA_SHORT.withName("compression_method"),
        ValueLayout.ADDRESS.withName("detected_encoding")
    );

    static {
        LOOKUP = NativeLoader.load().or(LINKER.defaultLookup());

        MH_VERSION = findDowncall("ttzip_rust_version", FunctionDescriptor.of(ValueLayout.ADDRESS));
        MH_IS_HARDWARE_ACCELERATED = findDowncall("ttzip_rust_is_hardware_accelerated", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN));
        MH_CRC32 = findDowncall("ttzip_rust_crc32", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_CRC64 = findDowncall("ttzip_rust_crc64", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_CREATE_ARCHIVE = findDowncall("ttzip_rust_create_archive", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_EXTRACT_ARCHIVE = findDowncall("ttzip_rust_extract_archive", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_INSPECT_ARCHIVE = findDowncall("ttzip_rust_inspect_archive", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
    }

    private static MethodHandle findDowncall(String name, FunctionDescriptor descriptor) {
        return LOOKUP.find(name)
            .map(addr -> LINKER.downcallHandle(addr, descriptor))
            .orElseThrow(() -> new UnsatisfiedLinkError("Failed to resolve native TTZip downcall symbol: '" + name + "'. Ensure binary ABI matches version " + NativeLoader.VERSION));
    }

    private TTZip() {}

    private static MemorySegment allocateUtf8(SegmentAllocator allocator, String s) {
        if (s == null) return MemorySegment.NULL;
        byte[] bytes = s.getBytes(StandardCharsets.UTF_8);
        MemorySegment seg = allocator.allocate((long) (bytes.length + 1));
        for (int i = 0; i < bytes.length; i++) {
            seg.set(ValueLayout.JAVA_BYTE, (long) i, bytes[i]);
        }
        seg.set(ValueLayout.JAVA_BYTE, (long) bytes.length, (byte) 0);
        return seg;
    }

    private static String readUtf8String(MemorySegment seg) {
        if (seg == null || seg.equals(MemorySegment.NULL) || seg.address() == 0) return "";
        try {
            return seg.reinterpret(4096).getUtf8String(0);
        } catch (Throwable t) {
            return "";
        }
    }

    /** Returns the underlying TTZip engine version string. */
    public static String version() {
        if (MH_VERSION == null) return "1.0.0";
        try {
            MemorySegment segment = (MemorySegment) MH_VERSION.invokeExact();
            return readUtf8String(segment);
        } catch (Throwable t) {
            return "1.0.0";
        }
    }

    /** Returns true if ARM NEON/Crypto or x86 AVX2/AES-NI acceleration is active. */
    public static boolean isHardwareAccelerated() {
        if (MH_IS_HARDWARE_ACCELERATED == null) return false;
        try {
            return (boolean) MH_IS_HARDWARE_ACCELERATED.invokeExact();
        } catch (Throwable t) {
            return false;
        }
    }

    /** Computes SIMD-accelerated CRC-32 on byte array. */
    public static int crc32(byte[] data) {
        Objects.requireNonNull(data, "data cannot be null");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment segment = arena.allocate((long) data.length);
            MemorySegment.copy(MemorySegment.ofArray(data), 0, segment, 0, (long) data.length);
            return crc32(segment, 0);
        }
    }

    /** Computes SIMD-accelerated CRC-32 on MemorySegment buffer. */
    public static int crc32(MemorySegment segment, int seed) {
        Objects.requireNonNull(segment, "segment cannot be null");
        if (MH_CRC32 == null) {
            // Software fallback if native handle unavailable
            return softwareCrc32(segment);
        }
        try {
            return (int) MH_CRC32.invokeExact(seed, segment, segment.byteSize());
        } catch (Throwable t) {
            return softwareCrc32(segment);
        }
    }

    /** Computes SIMD-accelerated CRC-64 on MemorySegment buffer. */
    public static long crc64(MemorySegment segment, long seed) {
        Objects.requireNonNull(segment, "segment cannot be null");
        if (MH_CRC64 == null) return 0L;
        try {
            return (long) MH_CRC64.invokeExact(seed, segment, segment.byteSize());
        } catch (Throwable t) {
            return 0L;
        }
    }

    /** Compresses sources into destination archive. */
    public static void compress(List<String> sources, String destination) {
        compress(sources, destination, ArchiveFormat.AUTO, CompressionLevel.NORMAL, null, 0, null);
    }

    /** Full parameter archive creation using FFM Arena. */
    public static void compress(
        List<String> sources,
        String destination,
        ArchiveFormat format,
        CompressionLevel level,
        String password,
        int threads,
        ProgressListener listener
    ) {
        Objects.requireNonNull(sources, "sources cannot be null");
        Objects.requireNonNull(destination, "destination cannot be null");
        if (sources.isEmpty()) throw new IllegalArgumentException("sources cannot be empty");
        if (MH_CREATE_ARCHIVE == null) throw new UnsupportedOperationException("Native ttzip_rust_create_archive symbol not loaded");

        try (Arena arena = Arena.ofConfined()) {
            // Allocate array of C-string pointers
            MemorySegment sourceArraySeg = arena.allocateArray(ValueLayout.ADDRESS, (long) sources.size());
            for (int i = 0; i < sources.size(); i++) {
                MemorySegment cStr = allocateUtf8(arena, sources.get(i));
                sourceArraySeg.setAtIndex(ValueLayout.ADDRESS, (long) i, cStr);
            }

            MemorySegment destSeg = allocateUtf8(arena, destination);
            MemorySegment pwdSeg = (password != null && !password.isEmpty())
                ? allocateUtf8(arena, password)
                : MemorySegment.NULL;

            MemorySegment optionsSeg = arena.allocate(CREATE_OPTIONS_LAYOUT);
            optionsSeg.set(ValueLayout.JAVA_INT, CREATE_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("struct_size")), (int) CREATE_OPTIONS_LAYOUT.byteSize());
            optionsSeg.set(ValueLayout.JAVA_INT, CREATE_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("abi_version")), ABI_VERSION_2);
            optionsSeg.set(ValueLayout.JAVA_INT, CREATE_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("format")), format.code);
            optionsSeg.set(ValueLayout.JAVA_INT, CREATE_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("level")), level.level);
            optionsSeg.set(ValueLayout.JAVA_INT, CREATE_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("encryption")), pwdSeg.equals(MemorySegment.NULL) ? 0 : 4);
            optionsSeg.set(ValueLayout.ADDRESS, CREATE_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("password")), pwdSeg);
            optionsSeg.set(ValueLayout.JAVA_INT, CREATE_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("thread_budget")), threads);
            optionsSeg.set(ValueLayout.JAVA_INT, CREATE_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("solid_block_size_mb")), 64);

            MemorySegment cbStub = MemorySegment.NULL;
            if (listener != null) {
                MethodHandle target = MethodHandles.lookup().bind(new ProgressReceiver(listener), "onCallback",
                    MethodType.methodType(boolean.class, long.class, long.class, MemorySegment.class, MemorySegment.class));
                FunctionDescriptor cbDesc = FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.JAVA_LONG, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.ADDRESS);
                cbStub = LINKER.upcallStub(target, cbDesc, arena);
            }
            optionsSeg.set(ValueLayout.ADDRESS, CREATE_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("progress_callback")), cbStub);
            optionsSeg.set(ValueLayout.ADDRESS, CREATE_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("user_data")), MemorySegment.NULL);

            int status = (int) MH_CREATE_ARCHIVE.invokeExact(sourceArraySeg, (long) sources.size(), destSeg, optionsSeg);
            if (status != 0) {
                throw new RuntimeException("Archive creation failed with status code: " + status);
            }
        } catch (RuntimeException re) {
            throw re;
        } catch (Throwable t) {
            throw new RuntimeException("TTZip native invocation failed: " + t.getMessage(), t);
        }
    }

    /** Extracts an archive to destination directory. */
    public static void extract(String archivePath, String destination) {
        extract(archivePath, destination, null, 0, null);
    }

    /** Full parameter archive extraction using FFM Arena. */
    public static void extract(
        String archivePath,
        String destination,
        String password,
        int threads,
        ProgressListener listener
    ) {
        Objects.requireNonNull(archivePath, "archivePath cannot be null");
        Objects.requireNonNull(destination, "destination cannot be null");
        if (MH_EXTRACT_ARCHIVE == null) throw new UnsupportedOperationException("Native ttzip_rust_extract_archive symbol not loaded");

        try (Arena arena = Arena.ofConfined()) {
            MemorySegment archiveSeg = allocateUtf8(arena, archivePath);
            MemorySegment destSeg = allocateUtf8(arena, destination);
            MemorySegment pwdSeg = (password != null && !password.isEmpty())
                ? allocateUtf8(arena, password)
                : MemorySegment.NULL;

            MemorySegment optionsSeg = arena.allocate(EXTRACT_OPTIONS_LAYOUT);
            optionsSeg.set(ValueLayout.JAVA_INT, EXTRACT_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("struct_size")), (int) EXTRACT_OPTIONS_LAYOUT.byteSize());
            optionsSeg.set(ValueLayout.JAVA_INT, EXTRACT_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("abi_version")), ABI_VERSION_2);
            optionsSeg.set(ValueLayout.ADDRESS, EXTRACT_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("destination_path")), destSeg);
            optionsSeg.set(ValueLayout.ADDRESS, EXTRACT_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("password")), pwdSeg);
            optionsSeg.set(ValueLayout.JAVA_INT, EXTRACT_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("thread_budget")), threads);
            optionsSeg.set(ValueLayout.JAVA_BOOLEAN, EXTRACT_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("overwrite_existing")), true);
            optionsSeg.set(ValueLayout.JAVA_BOOLEAN, EXTRACT_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("preserve_permissions")), true);
            optionsSeg.set(ValueLayout.JAVA_BOOLEAN, EXTRACT_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("dry_run")), false);

            MemorySegment cbStub = MemorySegment.NULL;
            if (listener != null) {
                MethodHandle target = MethodHandles.lookup().bind(new ProgressReceiver(listener), "onCallback",
                    MethodType.methodType(boolean.class, long.class, long.class, MemorySegment.class, MemorySegment.class));
                FunctionDescriptor cbDesc = FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.JAVA_LONG, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.ADDRESS);
                cbStub = LINKER.upcallStub(target, cbDesc, arena);
            }
            optionsSeg.set(ValueLayout.ADDRESS, EXTRACT_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("progress_callback")), cbStub);
            optionsSeg.set(ValueLayout.ADDRESS, EXTRACT_OPTIONS_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("user_data")), MemorySegment.NULL);

            int status = (int) MH_EXTRACT_ARCHIVE.invokeExact(archiveSeg, destSeg, optionsSeg);
            if (status != 0) {
                throw new RuntimeException("Archive extraction failed with status code: " + status);
            }
        } catch (RuntimeException re) {
            throw re;
        } catch (Throwable t) {
            throw new RuntimeException("TTZip native invocation failed: " + t.getMessage(), t);
        }
    }

    /** Inspects archive entry metadata without disk extraction. */
    public static List<EntryMetadata> inspect(String archivePath, String password) {
        Objects.requireNonNull(archivePath, "archivePath cannot be null");
        if (MH_INSPECT_ARCHIVE == null) return Collections.emptyList();

        List<EntryMetadata> result = new ArrayList<>();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment archiveSeg = allocateUtf8(arena, archivePath);
            MemorySegment pwdSeg = (password != null && !password.isEmpty())
                ? allocateUtf8(arena, password)
                : MemorySegment.NULL;

            InspectReceiver receiver = new InspectReceiver(result);
            MethodHandle target = MethodHandles.lookup().bind(receiver, "onInspect",
                MethodType.methodType(boolean.class, MemorySegment.class, MemorySegment.class));
            FunctionDescriptor cbDesc = FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS);
            MemorySegment cbStub = LINKER.upcallStub(target, cbDesc, arena);

            int status = (int) MH_INSPECT_ARCHIVE.invokeExact(archiveSeg, pwdSeg, true, cbStub, MemorySegment.NULL);
            if (status != 0) {
                throw new RuntimeException("Archive inspection failed with status code: " + status);
            }
        } catch (RuntimeException re) {
            throw re;
        } catch (Throwable t) {
            throw new RuntimeException("TTZip native inspection failed: " + t.getMessage(), t);
        }
        return result;
    }

    private static class ProgressReceiver {
        private final ProgressListener listener;
        public ProgressReceiver(ProgressListener listener) { this.listener = listener; }
        public boolean onCallback(long processed, long total, MemorySegment pathSeg, MemorySegment userSeg) {
            String path = readUtf8String(pathSeg);
            double frac = total > 0 ? ((double) processed / total) : 0.0;
            ArchiveProgress p = new ArchiveProgress(processed, total, frac, path, 0, 0, "processing", 0.0);
            return listener.onProgress(p);
        }
    }

    private static class InspectReceiver {
        private final List<EntryMetadata> list;
        public InspectReceiver(List<EntryMetadata> list) { this.list = list; }
        public boolean onInspect(MemorySegment metaSeg, MemorySegment userSeg) {
            if (metaSeg.equals(MemorySegment.NULL) || metaSeg.address() == 0) return false;
            MemorySegment bounded = metaSeg.byteSize() < ENTRY_METADATA_LAYOUT.byteSize()
                ? metaSeg.reinterpret(ENTRY_METADATA_LAYOUT.byteSize())
                : metaSeg;
            MemorySegment pathSeg = bounded.get(ValueLayout.ADDRESS, ENTRY_METADATA_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("path")));
            String path = readUtf8String(pathSeg);
            long uSize = bounded.get(ValueLayout.JAVA_LONG, ENTRY_METADATA_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("uncompressed_size")));
            long cSize = bounded.get(ValueLayout.JAVA_LONG, ENTRY_METADATA_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("compressed_size")));
            int crc = bounded.get(ValueLayout.JAVA_INT, ENTRY_METADATA_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("crc32")));
            long mtime = bounded.get(ValueLayout.JAVA_LONG, ENTRY_METADATA_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("mtime_epoch_secs")));
            boolean isDir = bounded.get(ValueLayout.JAVA_BOOLEAN, ENTRY_METADATA_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("is_directory")));
            boolean isEnc = bounded.get(ValueLayout.JAVA_BOOLEAN, ENTRY_METADATA_LAYOUT.byteOffset(MemoryLayout.PathElement.groupElement("is_encrypted")));
            list.add(new EntryMetadata(path, uSize, cSize, crc, mtime, isDir, isEnc));
            return true;
        }
    }

    private static int softwareCrc32(MemorySegment seg) {
        int crc = 0xFFFFFFFF;
        long len = seg.byteSize();
        for (long i = 0; i < len; i++) {
            byte b = seg.get(ValueLayout.JAVA_BYTE, i);
            crc = (crc >>> 8) ^ CRC_TABLE[(crc ^ b) & 0xFF];
        }
        return ~crc;
    }

    private static final int[] CRC_TABLE = new int[256];
    static {
        for (int i = 0; i < 256; i++) {
            int c = i;
            for (int k = 0; k < 8; k++) {
                c = ((c & 1) != 0) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
            }
            CRC_TABLE[i] = c;
        }
    }
}
