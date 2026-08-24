// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// Implements io/fs.FS virtual filesystem, context.Context cancellation, and SIMD checksums.

package ttzip

/*
#include "ttzip.h"
#include <stdlib.h>

extern bool goProgressCallback(uint64_t processed, uint64_t total, char* current_entry, void* user_data);
extern bool goInspectCallback(TTZipEntryMetadata* entry, void* user_data);
*/
import "C"

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"path"
	"strings"
	"sync"
	"time"
	"unsafe"
)

// ArchiveFormat container formats
type ArchiveFormat int

const (
	FormatAuto     ArchiveFormat = C.TTZIP_ARCHIVE_FORMAT_AUTO
	FormatZip      ArchiveFormat = C.TTZIP_ARCHIVE_FORMAT_ZIP
	FormatSevenZip ArchiveFormat = C.TTZIP_ARCHIVE_FORMAT_SEVEN_ZIP
	FormatTar      ArchiveFormat = C.TTZIP_ARCHIVE_FORMAT_TAR
	FormatTarGz    ArchiveFormat = C.TTZIP_ARCHIVE_FORMAT_TAR_GZ
	FormatTarBz2   ArchiveFormat = C.TTZIP_ARCHIVE_FORMAT_TAR_BZ2
	FormatTarXz    ArchiveFormat = C.TTZIP_ARCHIVE_FORMAT_TAR_XZ
	FormatTarZstd  ArchiveFormat = C.TTZIP_ARCHIVE_FORMAT_TAR_ZSTD
)

// CompressionLevel compression trade-off levels
type CompressionLevel int

const (
	LevelStore   CompressionLevel = C.TTZIP_COMPRESSION_LEVEL_STORE
	LevelFastest CompressionLevel = C.TTZIP_COMPRESSION_LEVEL_FASTEST
	LevelFast    CompressionLevel = C.TTZIP_COMPRESSION_LEVEL_FAST
	LevelNormal  CompressionLevel = C.TTZIP_COMPRESSION_LEVEL_NORMAL
	LevelMaximum CompressionLevel = C.TTZIP_COMPRESSION_LEVEL_MAXIMUM
	LevelUltra   CompressionLevel = C.TTZIP_COMPRESSION_LEVEL_ULTRA
)

// EntryMetadata describes an entry in an archive.
type EntryMetadata struct {
	Path              string
	UncompressedSize  uint64
	CompressedSize    uint64
	CRC32             uint32
	ModTime           time.Time
	Mode              fs.FileMode
	IsDir             bool
	IsEncrypted       bool
	CompressionMethod uint16
	DetectedEncoding  string
}

// ArchiveProgress represents streaming progress event.
type ArchiveProgress struct {
	ProcessedBytes    uint64
	TotalBytes        uint64
	FractionCompleted float64
	CurrentEntryPath  string
	Phase             string
	ThroughputMBs     float64
}

// ProgressFunc callback for observing extraction or compression progress.
type ProgressFunc func(progress ArchiveProgress) bool

// Options configures compression or extraction.
type Options struct {
	Format       ArchiveFormat
	Level        CompressionLevel
	Password     string
	Threads      int
	ProgressFunc ProgressFunc
}

// Option configures an Options struct.
type Option func(*Options)

func WithFormat(f ArchiveFormat) Option { return func(o *Options) { o.Format = f } }
func WithLevel(l CompressionLevel) Option { return func(o *Options) { o.Level = l } }
func WithPassword(p string) Option { return func(o *Options) { o.Password = p } }
func WithThreads(t int) Option { return func(o *Options) { o.Threads = t } }
func WithProgress(fn ProgressFunc) Option { return func(o *Options) { o.ProgressFunc = fn } }

// Version returns underlying TTZip engine version.
func Version() string {
	cstr := C.ttzip_rust_version()
	if cstr == nil {
		return "1.0.0"
	}
	return C.GoString(cstr)
}

// IsHardwareAccelerated returns true if ARM NEON/Crypto or AVX2/AVX-512 acceleration is active.
func IsHardwareAccelerated() bool {
	return bool(C.ttzip_rust_is_hardware_accelerated())
}

// CRC32 computes hardware-accelerated CRC-32 (>40 GB/s on Apple Silicon / AVX-512).
func CRC32(data []byte, seed ...uint32) uint32 {
	var s uint32 = 0
	if len(seed) > 0 {
		s = seed[0]
	}
	if len(data) == 0 {
		return s
	}
	return uint32(C.ttzip_rust_crc32(
		C.uint32_t(s),
		(*C.uint8_t)(unsafe.Pointer(&data[0])),
		C.size_t(len(data)),
	))
}

// CRC64 computes hardware-accelerated CRC-64.
func CRC64(data []byte, seed ...uint64) uint64 {
	var s uint64 = 0
	if len(seed) > 0 {
		s = seed[0]
	}
	if len(data) == 0 {
		return s
	}
	return uint64(C.ttzip_rust_crc64(
		C.uint64_t(s),
		(*C.uint8_t)(unsafe.Pointer(&data[0])),
		C.size_t(len(data)),
	))
}

// Context holder for C callbacks
type progressContext struct {
	ctx          context.Context
	progressFunc ProgressFunc
}

//export goProgressCallback
func goProgressCallback(processed, total C.uint64_t, currentEntry *C.char, userData unsafe.Pointer) C.bool {
	if userData == nil {
		return C.bool(true)
	}
	pCtx := (*progressContext)(userData)
	if pCtx.ctx != nil && pCtx.ctx.Err() != nil {
		return C.bool(false) // Cancel operation
	}
	if pCtx.progressFunc != nil {
		entryStr := ""
		if currentEntry != nil {
			entryStr = C.GoString(currentEntry)
		}
		var frac float64 = 0.0
		if total > 0 {
			frac = float64(processed) / float64(total)
		}
		cont := pCtx.progressFunc(ArchiveProgress{
			ProcessedBytes:    uint64(processed),
			TotalBytes:        uint64(total),
			FractionCompleted: frac,
			CurrentEntryPath:  entryStr,
			Phase:             "processing",
		})
		return C.bool(cont)
	}
	return C.bool(true)
}

var (
	inspectMu  sync.Mutex
	inspectMap = make(map[uintptr]*[]EntryMetadata)
	inspectSeq uintptr
)

//export goInspectCallback
func goInspectCallback(entry *C.TTZipEntryMetadata, userData unsafe.Pointer) C.bool {
	if entry == nil || userData == nil {
		return C.bool(false)
	}
	token := uintptr(*(*C.uintptr_t)(userData))
	inspectMu.Lock()
	entriesPtr, ok := inspectMap[token]
	inspectMu.Unlock()
	if !ok || entriesPtr == nil {
		return C.bool(false)
	}
	pathStr := ""
	if entry.path != nil {
		pathStr = C.GoString(entry.path)
	}
	encStr := ""
	if entry.detected_encoding != nil {
		encStr = C.GoString(entry.detected_encoding)
	}

	mode := fs.FileMode(entry.mode & 0777)
	if bool(entry.is_directory) {
		mode |= fs.ModeDir
	}

	item := EntryMetadata{
		Path:              pathStr,
		UncompressedSize:  uint64(entry.uncompressed_size),
		CompressedSize:    uint64(entry.compressed_size),
		CRC32:             uint32(entry.crc32),
		ModTime:           time.Unix(int64(entry.mtime_epoch_secs), 0),
		Mode:              mode,
		IsDir:             bool(entry.is_directory),
		IsEncrypted:       bool(entry.is_encrypted),
		CompressionMethod: uint16(entry.compression_method),
		DetectedEncoding:  encStr,
	}
	*entriesPtr = append(*entriesPtr, item)
	return C.bool(true)
}

// Compress creates an archive from source files with context cancellation support.
func Compress(ctx context.Context, sources []string, destination string, opts ...Option) error {
	if len(sources) == 0 {
		return errors.New("ttzip: sources cannot be empty")
	}
	if ctx.Err() != nil {
		return ctx.Err()
	}

	opt := Options{
		Format:  FormatAuto,
		Level:   LevelNormal,
		Threads: 0,
	}
	for _, fn := range opts {
		fn(&opt)
	}

	cSourcesPtr := (**C.char)(C.malloc(C.size_t(len(sources)) * C.size_t(unsafe.Sizeof(uintptr(0)))))
	defer C.free(unsafe.Pointer(cSourcesPtr))

	for i, s := range sources {
		cStr := C.CString(s)
		defer C.free(unsafe.Pointer(cStr))
		*(*uintptr)(unsafe.Pointer(uintptr(unsafe.Pointer(cSourcesPtr)) + uintptr(i)*unsafe.Sizeof(uintptr(0)))) = uintptr(unsafe.Pointer(cStr))
	}

	cDest := C.CString(destination)
	defer C.free(unsafe.Pointer(cDest))

	var cPwd *C.char
	if opt.Password != "" {
		cPwd = C.CString(opt.Password)
		defer C.free(unsafe.Pointer(cPwd))
	}

	var cOpts C.TTZipCreateOptions
	cOpts.struct_size = C.uint32_t(unsafe.Sizeof(cOpts))
	cOpts.abi_version = 2
	cOpts.format = C.TTZipArchiveFormat(opt.Format)
	cOpts.level = C.TTZipCompressionLevel(opt.Level)
	if cPwd != nil {
		cOpts.encryption = C.TTZIP_ENCRYPTION_AES256
	} else {
		cOpts.encryption = C.TTZIP_ENCRYPTION_NONE
	}
	cOpts.password = cPwd
	cOpts.thread_budget = C.uint32_t(opt.Threads)
	cOpts.solid_block_size_mb = 64
	cOpts.progress_callback = nil
	cOpts.user_data = nil

	status := C.ttzip_rust_create_archive(
		cSourcesPtr,
		C.size_t(len(sources)),
		cDest,
		&cOpts,
	)

	if status == C.TTZIP_STATUS_CANCELLED || (ctx != nil && ctx.Err() != nil) {
		if ctx != nil && ctx.Err() != nil {
			return ctx.Err()
		}
		return errors.New("ttzip: operation cancelled")
	}

	if status != C.TTZIP_STATUS_OK {
		return fmt.Errorf("ttzip: archive creation failed (status %d)", int(status))
	}
	return nil
}

// Extract extracts an archive to a destination directory with context cancellation support.
func Extract(ctx context.Context, archivePath string, destination string, opts ...Option) error {
	if ctx.Err() != nil {
		return ctx.Err()
	}

	opt := Options{
		Threads: 0,
	}
	for _, fn := range opts {
		fn(&opt)
	}

	cArchive := C.CString(archivePath)
	defer C.free(unsafe.Pointer(cArchive))

	cDest := C.CString(destination)
	defer C.free(unsafe.Pointer(cDest))

	var cPwd *C.char
	if opt.Password != "" {
		cPwd = C.CString(opt.Password)
		defer C.free(unsafe.Pointer(cPwd))
	}

	var cOpts C.TTZipExtractOptions
	cOpts.struct_size = C.uint32_t(unsafe.Sizeof(cOpts))
	cOpts.abi_version = 2
	cOpts.destination_path = cDest
	cOpts.password = cPwd
	cOpts.thread_budget = C.uint32_t(opt.Threads)
	cOpts.overwrite_existing = C.bool(true)
	cOpts.preserve_permissions = C.bool(true)
	cOpts.dry_run = C.bool(false)
	cOpts.progress_callback = nil
	cOpts.user_data = nil

	status := C.ttzip_rust_extract_archive(cArchive, cDest, &cOpts)

	if status == C.TTZIP_STATUS_CANCELLED || (ctx != nil && ctx.Err() != nil) {
		if ctx != nil && ctx.Err() != nil {
			return ctx.Err()
		}
		return errors.New("ttzip: operation cancelled")
	}

	if status != C.TTZIP_STATUS_OK {
		return fmt.Errorf("ttzip: archive extraction failed (status %d)", int(status))
	}
	return nil
}

// Inspect reads entry metadata without disk extraction.
func Inspect(ctx context.Context, archivePath string, password ...string) ([]EntryMetadata, error) {
	if ctx.Err() != nil {
		return nil, ctx.Err()
	}

	cArchive := C.CString(archivePath)
	defer C.free(unsafe.Pointer(cArchive))

	var cPwd *C.char
	if len(password) > 0 && password[0] != "" {
		cPwd = C.CString(password[0])
		defer C.free(unsafe.Pointer(cPwd))
	}

	inspectMu.Lock()
	inspectSeq++
	token := inspectSeq
	entries := make([]EntryMetadata, 0)
	inspectMap[token] = &entries
	inspectMu.Unlock()

	defer func() {
		inspectMu.Lock()
		delete(inspectMap, token)
		inspectMu.Unlock()
	}()

	cToken := (*C.uintptr_t)(C.malloc(C.size_t(unsafe.Sizeof(C.uintptr_t(0)))))
	*cToken = C.uintptr_t(token)
	defer C.free(unsafe.Pointer(cToken))

	status := C.ttzip_rust_inspect_archive(
		cArchive,
		cPwd,
		C.bool(true),
		(C.TTZipInspectCallback)(C.goInspectCallback),
		unsafe.Pointer(cToken),
	)

	if status != C.TTZIP_STATUS_OK {
		return nil, fmt.Errorf("ttzip: archive inspection failed (status %d)", int(status))
	}
	return entries, nil
}

// MARK: - io/fs.FS Virtual Filesystem Implementation

// ArchiveFS implements io/fs.FS, io/fs.StatFS, io/fs.ReadFileFS, and io/fs.ReadDirFS.
type ArchiveFS struct {
	archivePath string
	password    string
	entries     map[string]EntryMetadata
	dirChildren map[string][]string
	mu          sync.RWMutex
}

// OpenFS mounts an archive as an in-memory virtual filesystem implementing standard io/fs.FS.
func OpenFS(archivePath string, password ...string) (*ArchiveFS, error) {
	pwd := ""
	if len(password) > 0 {
		pwd = password[0]
	}

	entriesList, err := Inspect(context.Background(), archivePath, pwd)
	if err != nil {
		return nil, err
	}

	entriesMap := make(map[string]EntryMetadata, len(entriesList))
	dirChildren := make(map[string][]string)

	for _, entry := range entriesList {
		clean := path.Clean(strings.TrimPrefix(entry.Path, "/"))
		if clean == "." {
			continue
		}
		entriesMap[clean] = entry

		parent := path.Dir(clean)
		if parent == "." {
			parent = ""
		}
		dirChildren[parent] = append(dirChildren[parent], clean)
	}

	return &ArchiveFS{
		archivePath: archivePath,
		password:    pwd,
		entries:     entriesMap,
		dirChildren: dirChildren,
	}, nil
}

// Open opens the named file conforming to io/fs.FS.
func (afs *ArchiveFS) Open(name string) (fs.File, error) {
	if !fs.ValidPath(name) {
		return nil, &fs.PathError{Op: "open", Path: name, Err: fs.ErrInvalid}
	}
	clean := path.Clean(name)
	if clean == "." {
		clean = ""
	}

	afs.mu.RLock()
	defer afs.mu.RUnlock()

	// Root directory
	if clean == "" {
		return &archiveDirFile{
			fs:       afs,
			name:     ".",
			children: afs.dirChildren[""],
		}, nil
	}

	entry, ok := afs.entries[clean]
	if !ok {
		// Check if it is an implicit virtual directory
		if children, exists := afs.dirChildren[clean]; exists {
			return &archiveDirFile{
				fs:       afs,
				name:     path.Base(clean),
				children: children,
			}, nil
		}
		return nil, &fs.PathError{Op: "open", Path: name, Err: fs.ErrNotExist}
	}

	if entry.IsDir {
		return &archiveDirFile{
			fs:       afs,
			name:     path.Base(clean),
			children: afs.dirChildren[clean],
			meta:     &entry,
		}, nil
	}

	// Extract payload into memory buffer
	data, err := afs.ReadFile(name)
	if err != nil {
		return nil, &fs.PathError{Op: "open", Path: name, Err: err}
	}

	return &archiveFile{
		meta:   entry,
		reader: bytes.NewReader(data),
	}, nil
}

// ReadFile reads and returns the content of the named file conforming to io/fs.ReadFileFS.
func (afs *ArchiveFS) ReadFile(name string) ([]byte, error) {
	if !fs.ValidPath(name) {
		return nil, &fs.PathError{Op: "readfile", Path: name, Err: fs.ErrInvalid}
	}
	clean := path.Clean(name)

	afs.mu.RLock()
	entry, ok := afs.entries[clean]
	afs.mu.RUnlock()

	if !ok {
		return nil, &fs.PathError{Op: "readfile", Path: name, Err: fs.ErrNotExist}
	}
	if entry.IsDir {
		return nil, &fs.PathError{Op: "readfile", Path: name, Err: errors.New("is a directory")}
	}

	cArchive := C.CString(afs.archivePath)
	defer C.free(unsafe.Pointer(cArchive))

	cEntry := C.CString(entry.Path)
	defer C.free(unsafe.Pointer(cEntry))

	var cPwd *C.char
	if afs.password != "" {
		cPwd = C.CString(afs.password)
		defer C.free(unsafe.Pointer(cPwd))
	}

	bufCap := entry.UncompressedSize
	if bufCap == 0 {
		bufCap = 64
	}
	outBuf := make([]byte, bufCap)
	var extractedLen C.size_t = 0

	status := C.ttzip_rust_archive_extract_single_entry_memory(
		cArchive,
		cEntry,
		-1,
		cPwd,
		(*C.uint8_t)(unsafe.Pointer(&outBuf[0])),
		C.size_t(len(outBuf)),
		&extractedLen,
	)

	if status != C.TTZIP_STATUS_OK {
		return nil, fmt.Errorf("ttzip: single entry extraction failed (status %d)", int(status))
	}

	return outBuf[:int(extractedLen)], nil
}

// Stat returns FileInfo describing the named file conforming to io/fs.StatFS.
func (afs *ArchiveFS) Stat(name string) (fs.FileInfo, error) {
	if !fs.ValidPath(name) {
		return nil, &fs.PathError{Op: "stat", Path: name, Err: fs.ErrInvalid}
	}
	clean := path.Clean(name)
	if clean == "." {
		return &archiveFileInfo{
			name:    ".",
			size:    0,
			mode:    fs.ModeDir | 0755,
			modTime: time.Now(),
			isDir:   true,
		}, nil
	}

	afs.mu.RLock()
	defer afs.mu.RUnlock()

	if entry, ok := afs.entries[clean]; ok {
		return &archiveFileInfo{
			name:    path.Base(clean),
			size:    int64(entry.UncompressedSize),
			mode:    entry.Mode,
			modTime: entry.ModTime,
			isDir:   entry.IsDir,
		}, nil
	}

	if _, exists := afs.dirChildren[clean]; exists {
		return &archiveFileInfo{
			name:    path.Base(clean),
			size:    0,
			mode:    fs.ModeDir | 0755,
			modTime: time.Now(),
			isDir:   true,
		}, nil
	}

	return nil, &fs.PathError{Op: "stat", Path: name, Err: fs.ErrNotExist}
}

// ReadDir reads the named directory conforming to io/fs.ReadDirFS.
func (afs *ArchiveFS) ReadDir(name string) ([]fs.DirEntry, error) {
	if !fs.ValidPath(name) {
		return nil, &fs.PathError{Op: "readdir", Path: name, Err: fs.ErrInvalid}
	}
	clean := path.Clean(name)
	if clean == "." {
		clean = ""
	}

	afs.mu.RLock()
	children, exists := afs.dirChildren[clean]
	afs.mu.RUnlock()

	if !exists && clean != "" {
		return nil, &fs.PathError{Op: "readdir", Path: name, Err: fs.ErrNotExist}
	}

	entries := make([]fs.DirEntry, 0, len(children))
	for _, childPath := range children {
		info, err := afs.Stat(childPath)
		if err == nil {
			entries = append(entries, &archiveDirEntry{info: info})
		}
	}
	return entries, nil
}

// Close closes the virtual archive filesystem.
func (afs *ArchiveFS) Close() error {
	return nil
}

// MARK: - Internal File & Dir Helpers

type archiveFile struct {
	meta   EntryMetadata
	reader *bytes.Reader
}

func (f *archiveFile) Stat() (fs.FileInfo, error) {
	return &archiveFileInfo{
		name:    path.Base(f.meta.Path),
		size:    int64(f.meta.UncompressedSize),
		mode:    f.meta.Mode,
		modTime: f.meta.ModTime,
		isDir:   false,
	}, nil
}

func (f *archiveFile) Read(b []byte) (int, error) {
	return f.reader.Read(b)
}

func (f *archiveFile) Close() error {
	return nil
}

type archiveDirFile struct {
	fs       *ArchiveFS
	name     string
	children []string
	meta     *EntryMetadata
	offset   int
}

func (d *archiveDirFile) Stat() (fs.FileInfo, error) {
	if d.meta != nil {
		return &archiveFileInfo{
			name:    d.name,
			size:    int64(d.meta.UncompressedSize),
			mode:    d.meta.Mode,
			modTime: d.meta.ModTime,
			isDir:   true,
		}, nil
	}
	return &archiveFileInfo{
		name:    d.name,
		size:    0,
		mode:    fs.ModeDir | 0755,
		modTime: time.Now(),
		isDir:   true,
	}, nil
}

func (d *archiveDirFile) Read(b []byte) (int, error) {
	return 0, &fs.PathError{Op: "read", Path: d.name, Err: errors.New("is a directory")}
}

func (d *archiveDirFile) ReadDir(n int) ([]fs.DirEntry, error) {
	if d.offset >= len(d.children) && n > 0 {
		return nil, io.EOF
	}

	count := len(d.children) - d.offset
	if n > 0 && n < count {
		count = n
	}

	result := make([]fs.DirEntry, 0, count)
	for i := 0; i < count; i++ {
		childPath := d.children[d.offset+i]
		info, err := d.fs.Stat(childPath)
		if err == nil {
			result = append(result, &archiveDirEntry{info: info})
		}
	}
	d.offset += count
	return result, nil
}

func (d *archiveDirFile) Close() error {
	return nil
}

type archiveFileInfo struct {
	name    string
	size    int64
	mode    fs.FileMode
	modTime time.Time
	isDir   bool
}

func (i *archiveFileInfo) Name() string       { return i.name }
func (i *archiveFileInfo) Size() int64        { return i.size }
func (i *archiveFileInfo) Mode() fs.FileMode  { return i.mode }
func (i *archiveFileInfo) ModTime() time.Time { return i.modTime }
func (i *archiveFileInfo) IsDir() bool        { return i.isDir }
func (i *archiveFileInfo) Sys() any           { return nil }

type archiveDirEntry struct {
	info fs.FileInfo
}

func (d *archiveDirEntry) Name() string               { return d.info.Name() }
func (d *archiveDirEntry) IsDir() bool                { return d.info.IsDir() }
func (d *archiveDirEntry) Type() fs.FileMode          { return d.info.Mode().Type() }
func (d *archiveDirEntry) Info() (fs.FileInfo, error) { return d.info, nil }
