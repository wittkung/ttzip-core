// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// Platform-specific CGO build flags and linker directives.

package ttzip

/*
#cgo pkg-config: ttzip
#cgo CFLAGS: -I${SRCDIR}/include -I/usr/local/include -I/opt/homebrew/include

#cgo darwin LDFLAGS: -L/usr/local/lib -L/opt/homebrew/lib -L${SRCDIR}/lib/darwin -lttzip_engine -larchive -lbz2 -lz -llzma -framework Security -framework CoreFoundation -framework IOKit
#cgo linux LDFLAGS: -L/usr/local/lib -L/opt/homebrew/lib -L${SRCDIR}/lib/linux -lttzip_engine -larchive -lbz2 -lz -llzma -lm -lpthread -ldl
#cgo windows LDFLAGS: -L/usr/local/lib -L${SRCDIR}/lib/windows -lttzip_engine -larchive -lbz2 -lz -llzma -lws2_32 -luserenv -lbcrypt
*/
import "C"
