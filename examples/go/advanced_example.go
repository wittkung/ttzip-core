// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// TTZip: Advanced Go SDK Features Showcase.
// Demonstrates context.WithTimeout cancellation, io/fs.FS virtual filesystem traversal,
// AES-256 password encrypted archive creation, multi-format pipelines, and SIMD checksums.

package main

import (
	"context"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/ttzip/ttzip-go/ttzip"
)

func main() {
	fmt.Println("================================================================================")
	fmt.Printf("⚡️ TTZip Go SDK Advanced Features Showcase (v%s)\n", ttzip.Version())
	fmt.Println("================================================================================")

	// 1. Engine & SIMD Hardware Telemetry
	fmt.Println("1. Querying Native Engine Capabilities...")
	fmt.Printf("   • Engine Version:        %s\n", ttzip.Version())
	fmt.Printf("   • SIMD Acceleration:     %v (ARM NEON / AVX-512 / AES-NI)\n", ttzip.IsHardwareAccelerated())
	fmt.Println("--------------------------------------------------------------------------------")

	// 2. Hardware SIMD Checksums
	fmt.Println("2. Computing Hardware SIMD Checksums...")
	payload := []byte("TTZip Go SDK High-Throughput Archiving & Virtual FS Pipeline 2026")
	crc32Val := ttzip.CRC32(payload)
	crc64Val := ttzip.CRC64(payload)
	fmt.Printf("   • Hardware CRC-32:       0x%08X\n", crc32Val)
	fmt.Printf("   • Hardware CRC-64:       0x%016X\n", crc64Val)
	fmt.Println("--------------------------------------------------------------------------------")

	// 3. Prepare Multi-File Structured Workspace
	tmpDir, err := os.MkdirTemp("", "ttzip_go_adv_*")
	if err != nil {
		fmt.Fprintf(os.Stderr, "❌ Error creating temp dir: %v\n", err)
		os.Exit(1)
	}
	defer os.RemoveAll(tmpDir)

	payloadDir := filepath.Join(tmpDir, "payload")
	nestedDir := filepath.Join(payloadDir, "subservice")
	if err := os.MkdirAll(nestedDir, 0755); err != nil {
		fmt.Fprintf(os.Stderr, "❌ Failed to create nested dirs: %v\n", err)
		os.Exit(1)
	}

	file1 := filepath.Join(payloadDir, "app_config.json")
	file2 := filepath.Join(nestedDir, "metrics.log")
	file3 := filepath.Join(payloadDir, "README.md")

	if err := os.WriteFile(file1, []byte(`{"service": "ttzip-go", "cipher": "AES-256", "vfs": true}`), 0644); err != nil {
		fmt.Fprintf(os.Stderr, "❌ Failed to write file1: %v\n", err)
		os.Exit(1)
	}
	if err := os.WriteFile(file2, []byte("TTZip Go io/fs.FS virtual continuous streaming entry.\n"), 0644); err != nil {
		fmt.Fprintf(os.Stderr, "❌ Failed to write file2: %v\n", err)
		os.Exit(1)
	}
	if err := os.WriteFile(file3, []byte("# TTZip Go Advanced Example\nDemonstrating context cancellation & io/fs.FS traversal.\n"), 0644); err != nil {
		fmt.Fprintf(os.Stderr, "❌ Failed to write file3: %v\n", err)
		os.Exit(1)
	}

	sourcePaths := []string{file1, file2, file3}
	aesPassword := "GoSecurePassword2026!"
	encZipPath := filepath.Join(tmpDir, "encrypted_dataset.zip")
	tarZstPath := filepath.Join(tmpDir, "dataset.tar.zst")
	extractDir := filepath.Join(tmpDir, "extracted_output")

	// 4. AES-256 Password Archive Creation
	fmt.Println("3. Creating AES-256 Password Protected Archive (4 Threads)...")
	ctxCreation, cancelCreation := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancelCreation()

	err = ttzip.Compress(
		ctxCreation,
		sourcePaths,
		encZipPath,
		ttzip.WithFormat(ttzip.FormatZip),
		ttzip.WithLevel(ttzip.LevelNormal),
		ttzip.WithPassword(aesPassword),
		ttzip.WithThreads(4),
	)
	if err != nil {
		fmt.Fprintf(os.Stderr, "❌ AES-256 Archive creation failed: %v\n", err)
		os.Exit(1)
	}
	fi, _ := os.Stat(encZipPath)
	fmt.Printf("   ✓ AES-256 Protected Archive Created: %s (%d bytes)\n", filepath.Base(encZipPath), fi.Size())
	fmt.Println("--------------------------------------------------------------------------------")

	// 5. Creating TAR.ZST Archive
	fmt.Println("4. Creating TAR.ZST Archive with High Compression...")
	err = ttzip.Compress(
		ctxCreation,
		sourcePaths,
		tarZstPath,
		ttzip.WithFormat(ttzip.FormatTarZstd),
		ttzip.WithLevel(ttzip.LevelUltra),
		ttzip.WithThreads(4),
	)
	if err != nil {
		fmt.Fprintf(os.Stderr, "❌ TAR.ZST creation failed: %v\n", err)
		os.Exit(1)
	}
	fiZst, _ := os.Stat(tarZstPath)
	fmt.Printf("   ✓ TAR.ZST Archive Created: %s (%d bytes)\n", filepath.Base(tarZstPath), fiZst.Size())
	fmt.Println("--------------------------------------------------------------------------------")

	// 6. context.WithTimeout Cancellation Showcase
	fmt.Println("5. Demonstrating context.WithTimeout Cancellation Handling...")
	ctxTimeout, cancelTimeout := context.WithTimeout(context.Background(), 1*time.Microsecond)
	defer cancelTimeout()
	time.Sleep(2 * time.Millisecond) // Ensure context deadline has elapsed

	cancelledZip := filepath.Join(tmpDir, "should_cancel.zip")
	errCancel := ttzip.Compress(
		ctxTimeout,
		sourcePaths,
		cancelledZip,
		ttzip.WithFormat(ttzip.FormatZip),
	)
	if errCancel != nil && (errors.Is(errCancel, context.DeadlineExceeded) || errors.Is(errCancel, context.Canceled)) {
		fmt.Printf("   ✓ Operation cancelled gracefully via Context: %v\n", errCancel)
	} else if errCancel != nil {
		fmt.Printf("   ✓ Operation aborted: %v\n", errCancel)
	} else {
		fmt.Println("   • Operation completed before timeout.")
	}
	fmt.Println("--------------------------------------------------------------------------------")

	// 7. Inspect Archive Metadata
	fmt.Println("6. Inspecting Archive Metadata:")
	entries, err := ttzip.Inspect(context.Background(), encZipPath, aesPassword)
	if err != nil {
		fmt.Fprintf(os.Stderr, "❌ Inspection failed: %v\n", err)
		os.Exit(1)
	}
	for _, e := range entries {
		fmt.Printf("   * %-26s | Size: %6d B | CRC: 0x%08X | Encrypted: %v\n",
			e.Path, e.UncompressedSize, e.CRC32, e.IsEncrypted)
	}
	fmt.Println("--------------------------------------------------------------------------------")

	// 8. Virtual Filesystem (io/fs.FS) Traversal & Direct File Read
	fmt.Println("7. Mounting Encrypted Archive as Virtual Filesystem (io/fs.FS)...")
	vfs, err := ttzip.OpenFS(encZipPath, aesPassword)
	if err != nil {
		fmt.Fprintf(os.Stderr, "❌ OpenFS failed: %v\n", err)
		os.Exit(1)
	}
	defer vfs.Close()

	fmt.Println("   • Traversing virtual directory hierarchy via fs.WalkDir:")
	err = fs.WalkDir(vfs, ".", func(p string, d fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if p != "." {
			info, _ := d.Info()
			size := int64(0)
			if info != nil {
				size = info.Size()
			}
			fmt.Printf("     - [%s] %s (%d bytes)\n",
				ternary(d.IsDir(), "DIR ", "FILE"), p, size)
		}
		return nil
	})
	if err != nil {
		fmt.Fprintf(os.Stderr, "❌ VFS WalkDir error: %v\n", err)
	}

	// Direct read of a file through io/fs.FS
	for _, entry := range entries {
		if !entry.IsDir && strings.HasSuffix(entry.Path, "app_config.json") {
			content, readErr := fs.ReadFile(vfs, entry.Path)
			if readErr == nil {
				fmt.Printf("   ✓ Read virtual file directly (%s):\n     %s\n", entry.Path, string(content))
			}
			break
		}
	}
	fmt.Println("--------------------------------------------------------------------------------")

	// 9. Extract Archive to Disk & Verify Integrity
	fmt.Println("8. Extracting Encrypted Archive to Disk...")
	ctxExtract, cancelExtract := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancelExtract()

	err = ttzip.Extract(
		ctxExtract,
		encZipPath,
		extractDir,
		ttzip.WithPassword(aesPassword),
		ttzip.WithThreads(4),
	)
	if err != nil {
		fmt.Fprintf(os.Stderr, "❌ Extraction failed: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("   ✓ Successfully extracted to: %s\n", extractDir)

	fmt.Println("================================================================================")
	fmt.Println("🎉 TTZip Go SDK Advanced Showcase Completed Successfully (Exit Code: 0)")
	fmt.Println("================================================================================")
}

func ternary[T any](cond bool, trueVal, falseVal T) T {
	if cond {
		return trueVal
	}
	return falseVal
}
