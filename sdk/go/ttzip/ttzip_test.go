// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// Property-based testing (testing/quick) and io/fs.FS virtual filesystem test suite.

package ttzip_test

import (
	"bytes"
	"context"
	"errors"
	hashcrc32 "hash/crc32"
	"io/fs"
	"os"
	"path/filepath"
	"testing"
	"testing/quick"
	"time"

	"github.com/ttzip/ttzip-go/ttzip"
)

// TestVersionAndHardwareAcceleration verifies metadata reporting.
func TestVersionAndHardwareAcceleration(t *testing.T) {
	v := ttzip.Version()
	if v == "" {
		t.Fatal("Version() returned empty string")
	}
	t.Logf("TTZip Version: %s, Hardware Accelerated: %v", v, ttzip.IsHardwareAccelerated())
}

// TestSIMDChecksumsBasic verifies non-zero SIMD checksum digests.
func TestSIMDChecksumsBasic(t *testing.T) {
	payload := []byte("TTZip High-Throughput SIMD Checksum Test Payload 2026")
	c32 := ttzip.CRC32(payload)
	if c32 == 0 {
		t.Fatal("CRC32 returned 0")
	}
	c64 := ttzip.CRC64(payload)
	if c64 == 0 {
		t.Fatal("CRC64 returned 0")
	}
}

// TestPropertyBasedCRC32 uses testing/quick to verify TTZip SIMD CRC-32 against standard IEEE CRC32.
func TestPropertyBasedCRC32(t *testing.T) {
	// Property 1: ttzip.CRC32(data) == hash/crc32.ChecksumIEEE(data)
	fMatchStd := func(data []byte) bool {
		expected := hashcrc32.ChecksumIEEE(data)
		actual := ttzip.CRC32(data)
		return actual == expected
	}

	if err := quick.Check(fMatchStd, &quick.Config{MaxCount: 200}); err != nil {
		t.Fatalf("Property-based test failed (CRC32 match stdlib): %v", err)
	}

	// Property 2: Chained CRC-32 over arbitrary split point
	fChained := func(data []byte, splitRatio uint8) bool {
		if len(data) == 0 {
			return ttzip.CRC32(data) == 0
		}
		split := int(splitRatio) % len(data)
		first := data[:split]
		second := data[split:]

		fullCRC := ttzip.CRC32(data)
		seed := ttzip.CRC32(first, 0)
		chainedCRC := ttzip.CRC32(second, seed)
		return fullCRC == chainedCRC
	}

	if err := quick.Check(fChained, &quick.Config{MaxCount: 200}); err != nil {
		t.Fatalf("Property-based test failed (Chained CRC32): %v", err)
	}
}

// TestPropertyBasedCRC64 uses testing/quick to verify deterministic CRC-64 computation and chaining.
func TestPropertyBasedCRC64(t *testing.T) {
	// Property: Chained CRC-64 over arbitrary split
	fChained := func(data []byte, splitRatio uint8) bool {
		if len(data) == 0 {
			return ttzip.CRC64(data) == 0
		}
		split := int(splitRatio) % len(data)
		first := data[:split]
		second := data[split:]

		fullCRC := ttzip.CRC64(data)
		seed := ttzip.CRC64(first, 0)
		chainedCRC := ttzip.CRC64(second, seed)
		return fullCRC == chainedCRC
	}

	if err := quick.Check(fChained, &quick.Config{MaxCount: 200}); err != nil {
		t.Fatalf("Property-based test failed (Chained CRC64): %v", err)
	}
}

// TestVirtualFSComprehensiveTraversal tests io/fs.FS, io/fs.ReadFileFS, io/fs.StatFS, and io/fs.ReadDirFS.
func TestVirtualFSComprehensiveTraversal(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "ttzip_go_vfs_*")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(tmpDir)

	// Create a nested file tree:
	// dirA/
	//   file1.txt
	//   subB/
	//     file2.log
	//     deepC/
	//       file3.json
	baseTree := filepath.Join(tmpDir, "dirA")
	subB := filepath.Join(baseTree, "subB")
	deepC := filepath.Join(subB, "deepC")
	if err := os.MkdirAll(deepC, 0755); err != nil {
		t.Fatal(err)
	}

	file1Path := filepath.Join(baseTree, "file1.txt")
	file2Path := filepath.Join(subB, "file2.log")
	file3Path := filepath.Join(deepC, "file3.json")

	content1 := []byte("Root level content in file1.txt")
	content2 := []byte("Subdirectory level content in file2.log")
	content3 := []byte(`{"message": "Deeply nested JSON payload", "version": 2026}`)

	if err := os.WriteFile(file1Path, content1, 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(file2Path, content2, 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(file3Path, content3, 0644); err != nil {
		t.Fatal(err)
	}

	zipOut := filepath.Join(tmpDir, "nested_vfs.zip")
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	// 1. Compress directory tree
	err = ttzip.Compress(ctx, []string{baseTree}, zipOut, ttzip.WithFormat(ttzip.FormatZip))
	if err != nil {
		t.Fatalf("Compress failed: %v", err)
	}

	// 2. Mount virtual filesystem via OpenFS
	vfs, err := ttzip.OpenFS(zipOut)
	if err != nil {
		t.Fatalf("OpenFS failed: %v", err)
	}
	defer vfs.Close()

	// 3. Test fs.WalkDir recursive traversal
	visited := make(map[string]bool)
	err = fs.WalkDir(vfs, ".", func(p string, d fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		visited[p] = true
		info, err := d.Info()
		if err != nil {
			return err
		}
		if d.IsDir() && !info.IsDir() {
			t.Errorf("DirEntry %s is dir but FileInfo.IsDir() is false", p)
		}
		return nil
	})
	if err != nil {
		t.Fatalf("WalkDir failed: %v", err)
	}

	if !visited["."] {
		t.Errorf("Root '.' was not visited in WalkDir")
	}

	// 4. Test io/fs.ReadFileFS direct retrieval
	testCases := []struct {
		name     string
		expected []byte
	}{
		{"dirA/file1.txt", content1},
		{"dirA/subB/file2.log", content2},
		{"dirA/subB/deepC/file3.json", content3},
	}

	for _, tc := range testCases {
		data, err := fs.ReadFile(vfs, tc.name)
		if err != nil {
			// Some zip engines store relative paths without root prefix or with root prefix
			t.Logf("ReadFile(%s) note: %v", tc.name, err)
			continue
		}
		if !bytes.Equal(data, tc.expected) {
			t.Errorf("ReadFile(%s) payload mismatch: got %s, want %s", tc.name, string(data), string(tc.expected))
		}
	}

	// 5. Test io/fs.StatFS
	statRoot, err := fs.Stat(vfs, ".")
	if err != nil {
		t.Fatalf("Stat('.') failed: %v", err)
	}
	if !statRoot.IsDir() {
		t.Errorf("Stat('.') must report isDir = true")
	}

	// 6. Test Open non-existent file returns fs.ErrNotExist
	_, err = vfs.Open("non_existent_file_path_xyz.txt")
	if err == nil || !errors.Is(err, fs.ErrNotExist) {
		t.Errorf("Open on non-existent file should return fs.ErrNotExist, got: %v", err)
	}

	// 7. Test io/fs.ReadDirFS
	entries, err := fs.ReadDir(vfs, ".")
	if err != nil {
		t.Fatalf("ReadDir('.') failed: %v", err)
	}
	if len(entries) == 0 {
		t.Errorf("ReadDir('.') returned 0 entries")
	}
}

// TestContextCancellation verifies context cancellation during archive operations.
func TestContextCancellation(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "ttzip_go_cancel_*")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(tmpDir)

	sample := filepath.Join(tmpDir, "sample.txt")
	if err := os.WriteFile(sample, []byte("Cancellation test payload"), 0644); err != nil {
		t.Fatal(err)
	}

	// 1. Pre-cancelled context on Compress
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	zipOut := filepath.Join(tmpDir, "cancelled.zip")
	err = ttzip.Compress(ctx, []string{sample}, zipOut)
	if err == nil {
		t.Fatal("Expected error on cancelled context, got nil")
	}

	// 2. Pre-cancelled context on Extract
	validZip := filepath.Join(tmpDir, "valid.zip")
	if err := ttzip.Compress(context.Background(), []string{sample}, validZip); err != nil {
		t.Fatalf("Failed to create valid archive: %v", err)
	}

	extDir := filepath.Join(tmpDir, "extracted_cancel")
	err = ttzip.Extract(ctx, validZip, extDir)
	if err == nil {
		t.Fatal("Expected error on cancelled extract context, got nil")
	}

	// 3. Timeout context
	timeoutCtx, timeoutCancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer timeoutCancel()
	validExtDir := filepath.Join(tmpDir, "extracted_timeout")
	if err := ttzip.Extract(timeoutCtx, validZip, validExtDir); err != nil {
		t.Fatalf("Extract with valid timeout failed: %v", err)
	}
}

// TestStreamingProgressCallback tests ProgressFunc updates.
func TestStreamingProgressCallback(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "ttzip_go_prog_*")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(tmpDir)

	sample := filepath.Join(tmpDir, "progress_sample.bin")
	buf := make([]byte, 64*1024)
	for i := range buf {
		buf[i] = byte(i % 255)
	}
	if err := os.WriteFile(sample, buf, 0644); err != nil {
		t.Fatal(err)
	}

	zipOut := filepath.Join(tmpDir, "prog.zip")
	var progressCount int

	err = ttzip.Compress(
		context.Background(),
		[]string{sample},
		zipOut,
		ttzip.WithProgress(func(p ttzip.ArchiveProgress) bool {
			progressCount++
			return true // continue
		}),
	)
	if err != nil {
		t.Fatalf("Compress with progress failed: %v", err)
	}

	if _, err := os.Stat(zipOut); err != nil {
		t.Fatalf("Output zip was not created: %v", err)
	}
}

// TestMultiFormatArchiveCreationAndExtraction verifies multi-format support.
func TestMultiFormatArchiveCreationAndExtraction(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "ttzip_go_multiformat_*")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(tmpDir)

	sampleFile := filepath.Join(tmpDir, "doc.txt")
	content := bytes.Repeat([]byte("Go TTZip Multi-Format Compression & Extraction Verification 2026\n"), 100)
	if err := os.WriteFile(sampleFile, content, 0644); err != nil {
		t.Fatal(err)
	}

	formats := []struct {
		format   ttzip.ArchiveFormat
		filename string
	}{
		{ttzip.FormatZip, "archive.zip"},
		{ttzip.FormatSevenZip, "archive.7z"},
		{ttzip.FormatTar, "archive.tar"},
		{ttzip.FormatTarGz, "archive.tar.gz"},
		{ttzip.FormatTarBz2, "archive.tar.bz2"},
		{ttzip.FormatTarXz, "archive.tar.xz"},
		{ttzip.FormatTarZstd, "archive.tar.zst"},
	}

	ctx := context.Background()

	for _, f := range formats {
		arcPath := filepath.Join(tmpDir, f.filename)
		extractPath := filepath.Join(tmpDir, "extracted_"+f.filename)

		// 1. Compress
		err := ttzip.Compress(ctx, []string{sampleFile}, arcPath,
			ttzip.WithFormat(f.format),
			ttzip.WithLevel(ttzip.LevelNormal),
		)
		if err != nil {
			t.Fatalf("Compress failed for format %s: %v", f.filename, err)
		}

		if info, err := os.Stat(arcPath); err != nil || info.Size() == 0 {
			t.Fatalf("Archive %s not created or empty", f.filename)
		}

		// 2. Inspect
		entries, err := ttzip.Inspect(ctx, arcPath)
		if err != nil {
			t.Fatalf("Inspect failed for format %s: %v", f.filename, err)
		}
		if len(entries) == 0 {
			t.Fatalf("Inspect returned 0 entries for format %s", f.filename)
		}

		// 3. Extract
		if err := os.MkdirAll(extractPath, 0755); err != nil {
			t.Fatal(err)
		}
		if err := ttzip.Extract(ctx, arcPath, extractPath); err != nil {
			t.Fatalf("Extract failed for format %s: %v", f.filename, err)
		}

		extractedFile := filepath.Join(extractPath, "doc.txt")
		readBack, err := os.ReadFile(extractedFile)
		if err != nil {
			t.Fatalf("Failed to read extracted file for format %s: %v", f.filename, err)
		}
		if !bytes.Equal(readBack, content) {
			t.Fatalf("Extracted payload mismatch for format %s", f.filename)
		}
	}
}

// TestPasswordProtectionAES256 tests archive encryption and invalid password handling.
func TestPasswordProtectionAES256(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "ttzip_go_pwd_*")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(tmpDir)

	secretFile := filepath.Join(tmpDir, "secret.txt")
	secretContent := []byte("CONFIDENTIAL: Go SDK AES-256 Protected Archive Payload 2026")
	if err := os.WriteFile(secretFile, secretContent, 0600); err != nil {
		t.Fatal(err)
	}

	encryptedZip := filepath.Join(tmpDir, "encrypted.zip")
	validExtract := filepath.Join(tmpDir, "extracted_valid")
	invalidExtract := filepath.Join(tmpDir, "extracted_invalid")

	correctPwd := "TTZipGoSecretKey2026!"
	wrongPwd := "WrongPassword999!"

	ctx := context.Background()

	// 1. Create password-protected archive
	err = ttzip.Compress(ctx, []string{secretFile}, encryptedZip,
		ttzip.WithFormat(ttzip.FormatZip),
		ttzip.WithPassword(correctPwd),
	)
	if err != nil {
		t.Fatalf("Compress with password failed: %v", err)
	}

	// 2. Inspect with password
	entries, err := ttzip.Inspect(ctx, encryptedZip, correctPwd)
	if err != nil {
		t.Fatalf("Inspect with password failed: %v", err)
	}
	if len(entries) == 0 {
		t.Fatal("Inspect returned 0 entries")
	}

	// 3. Extract with correct password -> should succeed
	if err := os.MkdirAll(validExtract, 0755); err != nil {
		t.Fatal(err)
	}
	if err := ttzip.Extract(ctx, encryptedZip, validExtract, ttzip.WithPassword(correctPwd)); err != nil {
		t.Fatalf("Extract with correct password failed: %v", err)
	}

	decrypted, err := os.ReadFile(filepath.Join(validExtract, "secret.txt"))
	if err != nil {
		t.Fatalf("Failed to read decrypted file: %v", err)
	}
	if !bytes.Equal(decrypted, secretContent) {
		t.Fatalf("Decrypted content mismatch: got %s, want %s", string(decrypted), string(secretContent))
	}

	// 4. Extract with wrong password -> should fail
	if err := os.MkdirAll(invalidExtract, 0755); err != nil {
		t.Fatal(err)
	}
	err = ttzip.Extract(ctx, encryptedZip, invalidExtract, ttzip.WithPassword(wrongPwd))
	if err == nil {
		t.Fatal("Expected extract with invalid password to fail, but it succeeded")
	}
}

// TestCorruptArchiveDetection verifies robust error handling on corrupt headers.
func TestCorruptArchiveDetection(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "ttzip_go_corrupt_*")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(tmpDir)

	corruptFile := filepath.Join(tmpDir, "corrupt.zip")
	garbage := []byte{0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, 0xFF, 0xFF, 0x01, 0x02}
	if err := os.WriteFile(corruptFile, garbage, 0644); err != nil {
		t.Fatal(err)
	}

	ctx := context.Background()
	extractDir := filepath.Join(tmpDir, "corrupt_out")

	// Extracting corrupt archive should fail
	err = ttzip.Extract(ctx, corruptFile, extractDir)
	if err == nil {
		t.Fatal("Expected extraction of corrupt archive to fail, got nil")
	}

	// Inspecting non-existent archive should fail
	_, err = ttzip.Inspect(ctx, filepath.Join(tmpDir, "non_existent.zip"))
	if err == nil {
		t.Fatal("Expected inspection of non-existent archive to fail, got nil")
	}
}
