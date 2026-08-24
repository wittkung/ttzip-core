// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Go.
// Go CGO Headless Interop CLI Runner.

package main

import (
	"context"
	"fmt"
	"os"
	"strings"

	"github.com/ttzip/ttzip-go/ttzip"
)

func parseFormat(fmtStr string) ttzip.ArchiveFormat {
	switch strings.ToLower(fmtStr) {
	case "zip":
		return ttzip.FormatZip
	case "7z", "7zip", "sevenzip":
		return ttzip.FormatSevenZip
	case "tar":
		return ttzip.FormatTar
	case "tar.gz", "targz", "tgz", "gz":
		return ttzip.FormatTarGz
	case "tar.bz2", "tarbz2", "tbz2", "bz2":
		return ttzip.FormatTarBz2
	case "tar.xz", "tarxz", "txz", "xz":
		return ttzip.FormatTarXz
	case "tar.zst", "tarzst", "tar.zstd", "zst":
		return ttzip.FormatTarZstd
	default:
		return ttzip.FormatZip
	}
}

func printUsage(prog string) {
	fmt.Fprintf(os.Stderr, "Usage:\n")
	fmt.Fprintf(os.Stderr, "  %s --create <format> <src> <dst> [--password <pwd>]\n", prog)
	fmt.Fprintf(os.Stderr, "  %s --extract <src> <dst> [--password <pwd>]\n", prog)
	fmt.Fprintf(os.Stderr, "  %s --version\n", prog)
}

func main() {
	args := os.Args
	if len(args) < 2 {
		printUsage(args[0])
		os.Exit(2)
	}

	if args[1] == "--version" {
		fmt.Println(ttzip.Version())
		os.Exit(0)
	}

	var mode, formatStr, src, dst, password string

	i := 1
	for i < len(args) {
		arg := args[i]
		switch arg {
		case "--create":
			mode = "create"
			if i+3 >= len(args) {
				fmt.Fprintf(os.Stderr, "Error: --create requires <format> <src> <dst>\n")
				os.Exit(2)
			}
			formatStr = args[i+1]
			src = args[i+2]
			dst = args[i+3]
			i += 4
		case "--extract":
			mode = "extract"
			if i+2 >= len(args) {
				fmt.Fprintf(os.Stderr, "Error: --extract requires <src> <dst>\n")
				os.Exit(2)
			}
			src = args[i+1]
			dst = args[i+2]
			i += 3
		case "--password":
			if i+1 >= len(args) {
				fmt.Fprintf(os.Stderr, "Error: --password requires an argument\n")
				os.Exit(2)
			}
			password = args[i+1]
			i += 2
		default:
			fmt.Fprintf(os.Stderr, "Unknown argument: %s\n", arg)
			printUsage(args[0])
			os.Exit(2)
		}
	}

	if mode == "" {
		printUsage(args[0])
		os.Exit(2)
	}

	ctx := context.Background()

	if mode == "create" {
		format := parseFormat(formatStr)
		opts := []ttzip.Option{
			ttzip.WithFormat(format),
			ttzip.WithLevel(ttzip.LevelNormal),
		}
		if password != "" {
			opts = append(opts, ttzip.WithPassword(password))
		}
		err := ttzip.Compress(ctx, []string{src}, dst, opts...)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Archive creation failed: %v\n", err)
			os.Exit(1)
		}
		os.Exit(0)
	} else if mode == "extract" {
		var opts []ttzip.Option
		if password != "" {
			opts = append(opts, ttzip.WithPassword(password))
		}
		err := ttzip.Extract(ctx, src, dst, opts...)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Archive extraction failed: %v\n", err)
			os.Exit(1)
		}
		os.Exit(0)
	}

	os.Exit(2)
}
