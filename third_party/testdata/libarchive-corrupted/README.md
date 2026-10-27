# Required libarchive malformed corpus

These 26 raw UU fixtures are tracked upstream blobs from official
libarchive/libarchive commit `03b426003adf12c1eb4c34e72bedb4c2bf1ba8d3`.
The manifest records original path, Git object ID, raw/decoded SHA256, size and
associated upstream test source. COPYING is upstream's exact file; LICENSES.md
retains the associated test-source copyright/license notices. They are test data
only; the linked libarchive dependency stays pinned to3.8.1.bcr.3.

The original harness had three misspelled paths. Upstream test source specifies
`test_read_format_iso_rockridge_zf_overflow.iso`,
`test_read_format_iso_zisofs_overflow.iso`, and
`test_read_format_zip_malformed1.zip`. This pack keeps those actual source names;
it does not fabricate `.iso.Z`/`zip_malformed.zip` aliases. The corresponding
upstream `.c` files explicitly reference these names at the same pinned commit.

`libarchive_corrupted_archive_tests` owns this exact required26-file set. Its
named corpus and specialized scenarios fail when an input is missing; discovery
checks exactly the managed manifest, reports source and count, and sorts paths.
It makes no claim to sweep every unregistered fixture in a developer's full
upstream checkout. Previous HOME-based discovery was ambient and unpinned;
additional full-checkout coverage remains available only as explicit manual
investigation and is reported as manual, never managed acceptance evidence.

BUILD supplies `TTZIP_LIBARCHIVE_FIXTURE_DIR=third_party/testdata/libarchive-corrupted`
through this filegroup's runfiles. No HOME, vendor or compile-sandbox directory is
probed. Manual investigation may explicitly set
`--test_env=TTZIP_MANUAL_LIBARCHIVE_FIXTURE_DIR=/path/to/libarchive/test`;
that opt-in fails if unreadable or required fixtures are absent and never falls
back to managed or another personal directory. A normal acceptance run leaves it
unset. The managed manifest pins every raw UU file before parallel tests use it.

Fixture ownership does not resolve a native invalid free observed after the old
23-test summary. Keep that independent failure and diagnostic evidence open until
the allocator producer is identified; do not add serial annotations to hide it.
