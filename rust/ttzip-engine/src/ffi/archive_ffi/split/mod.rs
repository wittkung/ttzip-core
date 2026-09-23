// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! High-level C-ABI / FFI entries for multi-volume split containers.

pub mod ops;
pub mod reader;
pub mod writer;

pub use ops::{ttzip_rust_join_split_volumes, ttzip_rust_split_file};
pub use reader::{
    ttzip_rust_split_reader_free, ttzip_rust_split_reader_get_total_size,
    ttzip_rust_split_reader_get_volume_count, ttzip_rust_split_reader_get_volume_path,
    ttzip_rust_split_reader_open, ttzip_rust_split_reader_read, ttzip_rust_split_reader_seek,
    TTZipSplitReaderHandle,
};
pub use writer::{
    ttzip_rust_split_writer_cancel, ttzip_rust_split_writer_close,
    ttzip_rust_split_writer_flush, ttzip_rust_split_writer_free,
    ttzip_rust_split_writer_get_total_bytes, ttzip_rust_split_writer_get_volume_count,
    ttzip_rust_split_writer_get_volume_path, ttzip_rust_split_writer_new,
    ttzip_rust_split_writer_write, TTZipSplitWriterHandle,
};
