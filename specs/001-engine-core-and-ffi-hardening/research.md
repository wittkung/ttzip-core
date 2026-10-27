# Research & Technical Decisions: TTZip Engine Core & FFI Hardening

This document records the grounded research, decision justifications, and alternative evaluations for all audited architectural, concurrency, security, and memory safety items.

---

## 1. Archive & Decompression Safety Decisions

### Research Item 1: TAR UTF-8 Boundary Truncation (`BUG-01`)
- **Decision**: Truncate strings to $\le 100$ bytes using `truncate_to_char_boundary(s, 100)`, iterating downward with `str::is_char_boundary(end)`.
- **Rationale**: POSIX `ustar` headers allocate exactly 100 bytes for `name` and `linkname`. Truncating to the nearest lower character boundary prevents slicing panic while PAX extended headers (`pax_records`) retain the un-truncated full UTF-8 path.
- **Alternatives Considered**: Direct byte slicing `&s.as_bytes()[..100]` was rejected because invalid UTF-8 bytes cause downstream UB or invalid conversions.
- **Source**: [`core/rust/ttzip-engine/src/archive/tar/writer.rs:154, 163`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/archive/tar/writer.rs#L154).

### Research Item 2: 7z `K_ENCODED_HEADER` (0x17) Stream Decoding (`BUG-02`)
- **Decision**: When encountering `0x17`, parse `StreamsInfo`, decompress the header stream, and recursively execute `parse_7z_header_stream` on the decompressed bytes.
- **Rationale**: Standard 7-Zip enables header compression by default. Without decompressing the packed header stream, metadata tables cannot be located.
- **Source**: [`core/rust/ttzip-engine/src/sevenz/header/metadata.rs:53-65`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/sevenz/header/metadata.rs#L53).

### Research Item 3: Intermediate Symlink Traversal Verification (`BUG-03`)
- **Decision**: Implement `validate_no_intermediate_symlinks(dest_dir, target)`: iterate over all intermediate ancestor directories with `fs::symlink_metadata()` to verify no component is a symlink. Reject symlink targets resolving outside `dest_dir`.
- **Rationale**: POSIX `O_NOFOLLOW` only guards the leaf node. Intermediate symlinks allow escaping the sandbox.
- **Source**: [`core/rust/ttzip-engine/src/fs/safe_extract.rs:175-179`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/fs/safe_extract.rs#L175).

### Research Item 4: Streaming 7z Solid Decompression & Dynamic Dictionary Mapping (`BUG-04`)
- **Decision**:
  1. Build a zero-materialization streaming state machine (`Streaming7zExtractor` backed by `Fl2DStream`) using a fixed 1MB output ring buffer, dispatching chunks directly to open file descriptors.
  2. Dynamically map compression levels to standard dictionary sizes (256KB~64MB) and query `ctx.dict_property()` to emit accurate `dict_prop` headers instead of hardcoded `20u8`.
- **Rationale**: Full-memory solid stream materialization (`vec![0u8; total_size]`) causes OOM on multi-gigabyte archives. Hardcoded 4MB dictionary headers cause decompression corruption in official 7-Zip when archives are packed with Maximum/Ultra levels.
- **Alternatives Considered**: In-memory chunk slicing was rejected for decompression because memory scales with archive size.
- **Source**: [`core/rust/ttzip-engine/src/sevenz/decoder/archive.rs:141-146`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/sevenz/decoder/archive.rs#L141), [`writer.rs:197`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/sevenz/writer.rs#L197).

### Research Item 5: Bounded Channel Parallel ZIP Streaming & Multi-Protocol Decryption (`BUG-05`)
- **Decision**:
  1. Replace `collect::<Vec<CompressedEntryResult>>()` with a bounded MPSC channel (capacity 16 chunks / 64MB) and a dedicated writer thread executing POSIX `pwrite`.
  2. Integrate atomic progress throttling and cooperative `cancel_flag` polling in Rayon work loops.
  3. Dynamic encryption dispatch: parse AES Extra Fields for AES-128/192/256 strength, or fallback to `crypto::zipcrypto` for standard PKZIP encryption.
- **Rationale**: Prevents multi-gigabyte RAM spikes during compression and eliminates UI progress freeze / cancellation dead-code during decompression.
- **Source**: [`core/rust/ttzip-engine/src/zip/writer/streaming_parallel.rs:125-150`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/zip/writer/streaming_parallel.rs#L125), [`zip/reader.rs:98-113, 224-244`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/zip/reader.rs#L98).

---

## 2. Cryptography & Security Decisions

### Research Item 6: AES-256-CBC Decryption State Chaining & Inverse Key Schedule (`BUG-06`)
- **Decision**: Remove `.clone()` on decryptor/encryptor in `cbc.rs` to maintain continuous IV feedback across blocks. Implement `inv_mix_columns_block` and reverse round key ordering for non-AArch64 `round_keys_dec`.
- **Rationale**: Continuous cipher feedback is required for CBC mode.
- **Source**: [`core/rust/ttzip-engine/src/crypto/aes256/cbc.rs:43-54`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/crypto/aes256/cbc.rs#L43).

### Research Item 7: Dynamic Slice Scaling for Reed-Solomon FEC (`BUG-07`)
- **Decision**: Scale `effective_slice_size = max(base_slice, payload_len.div_ceil(200))` aligned to 4096 bytes.
- **Rationale**: $\text{GF}(2^8)$ Galois field constraints enforce $K \le 200$. Fixed 64KB slices fail on files $>12.8\text{MB}$.
- **Source**: [`core/rust/ttzip-engine/src/crypto/rs_fec/record_format.rs:76-79`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/crypto/rs_fec/record_format.rs#L76).

### Research Item 8: Constant-Time Crypto & Hardware Acceleration (`BUG-08`)
- **Decision**: Replace hand-rolled `GHash::mul_h` in `vault.rs` with Apple native `CryptoKit.AES.GCM` or standard `aes-gcm` crate (`PMULL`/`CLMUL` accelerated). Implement constant-time equality comparisons for WinZip MAC and PVV.
- **Rationale**: Secret-dependent branching `if ((x[byte_idx] >> bit_idx) & 1) != 0` leaks subkey bits via branch timing side-channels and forfeits hardware vector acceleration.
- **Source**: [`core/rust/ttzip-engine/src/crypto/vault.rs:106-132`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/crypto/vault.rs#L106), NIST SP 800-38D.

### Research Item 9: 7z Recovery Target Parsing & Non-Destructive Vault Probing (`BUG-09`)
- **Decision**:
  1. Extract real `salt` and `num_cycles_power` from 7z Coder Properties in `password_recovery.rs`.
  2. In `TTZipEngineFacade.quickExtract`, probe Password Vault entries with in-memory non-destructive inspection before triggering disk-modifying extraction.
- **Rationale**: Prevents recovery failures on salted 7z archives and eliminates disk write amplification / dirty remnant files during vault trial unlocking.
- **Source**: [`core/rust/ttzip-engine/src/crypto/password_recovery.rs:155-163`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/crypto/password_recovery.rs#L155), [`TTZipEngineFacade.swift:537-560`](file:///Users/kevintung/Documents/dev/TTZip/core/Sources/TTZipCore/Facades/TTZipEngineFacade.swift#L537).

---

## 3. FFI, Concurrency & Swift Bridge Decisions

### Research Item 10: Deterministic Error Envelope Out-Parameter Protocol (`BUG-10`)
- **Decision**: Define `TTZipErrorInfo` C-ABI struct (`status`, `error_code`, `message[512]`, `entry_path[256]`, `offset`) passed as an out-parameter to FFI functions.
- **Rationale**: `thread_local!` TLS error pointers escape borrow scopes (UB) and are lost or corrupted when Swift async Tasks hop threads.
- **Source**: [`core/rust/ttzip-engine/src/types.rs:280-312`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/types.rs#L280), [`ArchiveReader.swift:25-45`](file:///Users/kevintung/Documents/dev/TTZip/core/Sources/TTZipCore/ArchiveReader.swift#L25).

### Research Item 11: True Swift Concurrency & Cancellation Context Binding (`BUG-11`)
- **Decision**:
  1. Wrap synchronous C-ABI bridge calls inside `Task.detached(priority: .userInitiated)` in `ArchiveEngineBridge.swift`.
  2. Bind Swift `withTaskCancellationHandler` cancellation checks into `ProgressBridgeContext`.
- **Rationale**: Prevents synchronous C-ABI calls from blocking the MainActor or cooperative thread pool, and ensures cancellation signals are received by the Rust microkernel.
- **Source**: [`core/Sources/TTZipCore/Bridge/ArchiveEngineBridge.swift:354-400`](file:///Users/kevintung/Documents/dev/TTZip/core/Sources/TTZipCore/Bridge/ArchiveEngineBridge.swift#L354), [`ProgressBridgeContext.swift:57-59`](file:///Users/kevintung/Documents/dev/TTZip/core/Sources/TTZipCore/Bridge/ProgressBridgeContext.swift#L57).

### Research Item 12: Direct FFI Byte Accounting (`BUG-12`)
- **Decision**: Export `ttzip_rust_archive_extract_unified_v2` with `out_extracted_bytes: *mut u64`, returning uncompressed bytes directly from Rust.
- **Rationale**: Eliminates Swift `calculateDirectorySize` recursive disk scans, saving $>5\text{s}$ and 100,000 system calls on large archives.
- **Source**: [`core/Sources/TTZipCore/Bridge/ArchiveEngineBridge.swift:222`](file:///Users/kevintung/Documents/dev/TTZip/core/Sources/TTZipCore/Bridge/ArchiveEngineBridge.swift#L222), [`archive/unified/extract.rs:103`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/archive/unified/extract.rs#L103).

### Research Item 13: Memory Safe Buffers & Two-Stage Allocation (`BUG-13`)
- **Decision**:
  1. `CUnsafeBufferAdapter`: Use `UnsafeMutablePointer<UInt8>.allocate` with explicit `initialize(to: 0)` and `deinitialize(count:)`.
  2. `extractSingleEntryData`: Stage 1 query exact size with NULL buffer; Stage 2 allocate exact `Data(count: probedLen)`.
- **Rationale**: Fixes Swift UB in uninitialized pointer assignment, eliminates 32MB allocation thrashing on small files, and fixes truncation failures on $>32\text{MB}$ files.
- **Source**: [`core/Sources/TTZipCore/Bridge/CUnsafeBufferAdapter.swift:49-65`](file:///Users/kevintung/Documents/dev/TTZip/core/Sources/TTZipCore/Bridge/CUnsafeBufferAdapter.swift#L49), [`ArchiveExtractor.swift:353-355`](file:///Users/kevintung/Documents/dev/TTZip/core/Sources/TTZipCore/ArchiveExtractor.swift#L353).

---

## 4. VFS, Cache & Toolchain Decisions

### Research Item 14: VFS Arena Slot Reuse & Lock-Free I/O (`BUG-14`)
- **Decision**:
  1. Implement `allocate_node` popping `free_indices` on insertion.
  2. Three-phase cache read: Read lock snapshot -> short write lock LRU promotion -> lock-free disk I/O and LZ4 decompression.
- **Rationale**: Halts unbounded vector growth in VFS shards and eliminates multi-millisecond thread convoy stalls on cache hits.
- **Source**: [`core/rust/ttzip-engine/src/vfs/cache_pool.rs:131-148, 299-320`](file:///Users/kevintung/Documents/dev/TTZip/core/rust/ttzip-engine/src/vfs/cache_pool.rs#L131).

### Research Item 15: Universal Binary Build Script Preservation (`BUG-15`)
- **Decision**: Remove `lipo -extract arm64` in `scripts/build_rust.sh` and preserve `macos-arm64_x86_64` Universal slices.
- **Rationale**: Restores native execution on Intel Macs and Rosetta 2 simulators.
- **Source**: [`scripts/build_rust.sh:127-129`](file:///Users/kevintung/Documents/dev/TTZip/apple/.build/checkouts/ttzip-core/scripts/build_rust.sh#L127).
