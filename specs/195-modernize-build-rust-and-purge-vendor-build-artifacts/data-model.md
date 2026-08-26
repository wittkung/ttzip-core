# Data Model: 195-modernize-build-rust-and-purge-vendor-build-artifacts

## 1. Clean Vendor Directory Architecture
```
Vendor/
  ├── TTZipVendor.xcframework/
  │   ├── Info.plist
  │   └── macos-arm64/
  │       ├── Headers/
  │       │   └── ttzip_rust_glue.h
  │       └── libTTZipVendor.a
  ├── libarchive-upstream/
  ├── libdeflate-upstream/
  ├── lz4-upstream/
  ├── snappy-upstream/
  ├── turbobench/
  ├── worktrees/
  ├── xz-upstream/
  ├── zlib-ng-upstream/
  ├── zopfli-upstream/
  └── zstd-upstream/
```
