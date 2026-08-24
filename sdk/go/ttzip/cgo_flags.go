// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Go.
// Platform-specific CGO build flags and linker directives.

package ttzip

/*
#cgo CFLAGS: -I${SRCDIR}/include

#cgo darwin LDFLAGS: -L${SRCDIR}/../../../rust/target/release -L${SRCDIR}/lib/darwin -lttzip_engine -larchive -lbz2 -lz -llzma -framework Security
#cgo linux LDFLAGS: -L${SRCDIR}/../../../rust/target/release -L${SRCDIR}/lib/linux -lttzip_engine -larchive -lbz2 -lz -llzma -lm -lpthread -ldl
#cgo windows LDFLAGS: -L${SRCDIR}/../../../rust/target/release -L${SRCDIR}/lib/windows -lttzip_engine -larchive -lbz2 -lz -llzma -lws2_32 -luserenv -lbcrypt
*/
import "C"
