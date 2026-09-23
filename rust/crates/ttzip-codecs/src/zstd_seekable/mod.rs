// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Seekable Zstandard Format (`ZstdSeekable`) & Skippable Frame Index Table.
//!
//! Conforms to the official Zstandard Seekable Format Specification:
//! - **Magic Number**: `SEEKABLE_MAGIC_NUMBER = 0x8F92EAB1`
//! - **Skippable Frame Magic**: `SKIPPABLE_MAGIC_NUMBER = 0x184D2A5E`
//! - **Footer**: 9 bytes fixed (`Number_Of_Frames: u32`, `Descriptor: u8`, `Seekable_Magic: u32`)
//! - **Entries**: `c_size: u32`, `d_size: u32`, optional `checksum: u32` (when Descriptor bit 7 is set)
//! - **Random Access**: $\mathcal{O}(\log N)$ binary search prefix indexing for sub-millisecond point seeks.

mod decoder;
mod encoder;
mod reader;
mod types;

pub use decoder::SeekTableDecoder;
pub use encoder::{SeekTableEncoder, ZstdSeekableWriter};
pub use reader::ZstdSeekableReader;
pub use types::{
    SeekFrameInfo, SeekTableEntry, SeekableError, DEFAULT_SEEKABLE_FRAME_SIZE,
    SEEKABLE_FOOTER_SIZE, SEEKABLE_MAGIC_NUMBER, SEEK_TABLE_FLAG_CHECKSUM, SKIPPABLE_HEADER_SIZE,
    SKIPPABLE_MAGIC_NUMBER,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_seekable_roundtrip_with_multiple_frames() {
        let mut raw_data = Vec::new();
        for i in 0..10_000 {
            raw_data.extend_from_slice(format!("Line {} of uncompressed text.\n", i).as_bytes());
        }

        let chunk_size = 8192;
        let mut archive_buf = Vec::new();
        let mut writer = ZstdSeekableWriter::new(&mut archive_buf, chunk_size, 3, true);
        writer.write_all(&raw_data).expect("write data");
        let (_, table_size) = writer.finish().expect("finish writer");

        assert!(table_size > SEEKABLE_FOOTER_SIZE);
        assert!(!archive_buf.is_empty());

        let decoder = SeekTableDecoder::parse_from_slice(&archive_buf).expect("parse seek table");
        assert!(decoder.frame_count() > 1);
        assert_eq!(decoder.total_decompressed_size(), raw_data.len() as u64);
        assert!(decoder.has_checksums());

        // Test random-access seek in the middle
        let target_offset = 25_000u64;
        let target_len = 500usize;
        let mut out = vec![0u8; target_len];
        let read_len = decoder
            .decompress_range_from_slice(&archive_buf, target_offset, target_len, &mut out)
            .expect("decompress range");

        assert_eq!(read_len, target_len);
        assert_eq!(
            &out[..],
            &raw_data[target_offset as usize..target_offset as usize + target_len]
        );
    }
}
