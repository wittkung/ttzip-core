# ArchiveSource Contract & Medium Dispatch Invariants

**Interface**: `crate::archive::source::ArchiveSource`

## Trait Definition

```rust
pub trait ArchiveSource: Send + Sync {
    /// Returns the full contiguous slice if backed by an mmap or memory buffer.
    fn as_slice(&self) -> Option<&[u8]>;

    /// Reads up to `buf.len()` bytes at `offset` without mutating internal cursor state.
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, TTZipStatus>;

    /// Returns the total archive length in bytes.
    fn len(&self) -> u64;

    /// Returns the underlying physical storage medium.
    fn medium(&self) -> StorageMedium;
}
```

## Medium Dispatch Invariants

1. **Local NVMe/APFS Volumes** (`sfs.f_flags & MNT_LOCAL != 0` AND `sfs.f_fstypename == "apfs"`):
   - MUST construct `MmapSource` using `memmap2::MmapOptions::new().map(&file)`.
   - `as_slice()` returns `Some(&[u8])`.
2. **Network / Remote Volumes** (`sfs.f_flags & MNT_LOCAL == 0` e.g. SMB, NFS, WebDAV):
   - MUST construct `StreamSource`.
   - `as_slice()` returns `None`.
   - Reads execute via `pread(2)` against the file descriptor into a 64KB thread-local ring buffer.
   - Any I/O disconnect returns `Err(TTZipStatus::ErrOpenFailed)` with errno details in `DiagnosticErrorContext`.
   - MUST NOT invoke `mmap(2)`, completely preventing `SIGBUS` panics.
3. **Split Volumes (`.001`, `.002`, `.z01`)**:
   - Implemented via `SplitVolumeSource: ArchiveSource`.
   - Traverses volume boundary segments virtually without temporary file creation on disk.
