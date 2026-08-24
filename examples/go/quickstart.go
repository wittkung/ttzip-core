// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// Standalone runnable quickstart example.

package main

import (
	"context"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"time"

	"github.com/ttzip/ttzip-go/ttzip"
)

func main() {
	fmt.Printf("⚡️ TTZip Go SDK Quickstart (v%s)\n", ttzip.Version())
	fmt.Printf("Hardware Accelerated: %v\n", ttzip.IsHardwareAccelerated())

	// 1. Hardware SIMD Checksums
	payload := []byte("TTZip Go SDK High-Performance Archiving and Checksum Pipeline")
	crc32Val := ttzip.CRC32(payload)
	crc64Val := ttzip.CRC64(payload)
	fmt.Printf("SIMD CRC-32: 0x%08X\n", crc32Val)
	fmt.Printf("SIMD CRC-64: 0x%016X\n", crc64Val)

	// 2. Setup temporary demo files
	tmpDir, err := os.MkdirTemp("", "ttzip_go_quickstart_*")
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error creating temp dir: %v\n", err)
		os.Exit(1)
	}
	defer os.RemoveAll(tmpDir)

	sample1 := filepath.Join(tmpDir, "hello.txt")
	sample2 := filepath.Join(tmpDir, "config.json")
	if err := os.WriteFile(sample1, []byte("Hello from TTZip Go Quickstart!"), 0644); err != nil {
		fmt.Fprintf(os.Stderr, "Failed to write sample1: %v\n", err)
		os.Exit(1)
	}
	if err := os.WriteFile(sample2, []byte(`{"sdk": "Go", "version": 2026, "simd": true}`), 0644); err != nil {
		fmt.Fprintf(os.Stderr, "Failed to write sample2: %v\n", err)
		os.Exit(1)
	}

	zipPath := filepath.Join(tmpDir, "quickstart.zip")
	extractDir := filepath.Join(tmpDir, "extracted")

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	// 3. Compress files into Zip archive
	fmt.Println("\nCreating archive...")
	err = ttzip.Compress(
		ctx,
		[]string{sample1, sample2},
		zipPath,
		ttzip.WithFormat(ttzip.FormatZip),
		ttzip.WithLevel(ttzip.LevelNormal),
	)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Archive creation failed: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Created: %s\n", zipPath)

	// 4. Inspect archive metadata
	fmt.Println("\nInspecting archive entries:")
	entries, err := ttzip.Inspect(ctx, zipPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Inspection failed: %v\n", err)
		os.Exit(1)
	}
	for _, entry := range entries {
		fmt.Printf("  - %s (%d bytes, compressed: %d bytes, CRC32: 0x%08X)\n",
			entry.Path, entry.UncompressedSize, entry.CompressedSize, entry.CRC32)
	}

	// 5. Mount as io/fs.FS Virtual Filesystem
	fmt.Println("\nMounting archive as virtual filesystem (io/fs.FS):")
	vfs, err := ttzip.OpenFS(zipPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "OpenFS failed: %v\n", err)
		os.Exit(1)
	}
	defer vfs.Close()

	err = fs.WalkDir(vfs, ".", func(p string, d fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if p != "." {
			fmt.Printf("  VFS Node: %s (dir=%v)\n", p, d.IsDir())
		}
		return nil
	})
	if err != nil {
		fmt.Fprintf(os.Stderr, "VFS traversal failed: %v\n", err)
	}

	// 6. Extract archive
	fmt.Println("\nExtracting archive...")
	err = ttzip.Extract(ctx, zipPath, extractDir)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Extraction failed: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Extracted to: %s\n", extractDir)

	fmt.Println("\n✅ TTZip Go Quickstart completed successfully.")
}
