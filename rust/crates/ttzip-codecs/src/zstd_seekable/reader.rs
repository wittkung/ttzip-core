// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Transparent random-access streaming reader for Seekable Zstandard archives.

use super::decoder::SeekTableDecoder;
use super::types::SeekableError;
use std::io::{self, Read, Seek, SeekFrom};

/// Streaming random-access reader wrapping an underlying seekable input stream
/// and providing transparent uncompressed reading and seeking.
pub struct ZstdSeekableReader<R: Read + Seek> {
    reader: R,
    decoder: SeekTableDecoder,
    pos: u64,
    current_frame: Option<(usize, Vec<u8>)>,
}

impl<R: Read + Seek> ZstdSeekableReader<R> {
    /// Creates a new `ZstdSeekableReader` by parsing the Seek Table from the end of `reader`.
    pub fn new(mut reader: R) -> Result<Self, SeekableError> {
        let decoder = SeekTableDecoder::parse_from_reader(&mut reader)?;
        Ok(Self {
            reader,
            decoder,
            pos: 0,
            current_frame: None,
        })
    }

    /// Returns a reference to the parsed [`SeekTableDecoder`].
    #[inline]
    pub fn decoder(&self) -> &SeekTableDecoder {
        &self.decoder
    }

    /// Total logical uncompressed size in bytes.
    #[inline]
    pub fn total_size(&self) -> u64 {
        self.decoder.total_decompressed_size()
    }

    /// Current uncompressed read cursor position.
    #[inline]
    pub fn position(&self) -> u64 {
        self.pos
    }

    /// Unwraps and returns the underlying stream reader.
    #[inline]
    pub fn into_inner(self) -> R {
        self.reader
    }
}

impl<R: Read + Seek> Read for ZstdSeekableReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.pos >= self.decoder.total_decompressed_size() {
            return Ok(0);
        }

        let frame_idx = match self.decoder.offset_to_frame_index(self.pos) {
            Some(idx) => idx,
            None => return Ok(0),
        };

        if self.current_frame.as_ref().map(|(idx, _)| *idx) != Some(frame_idx) {
            let info = self.decoder.get_frame(frame_idx).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Invalid frame index")
            })?;
            let mut decomp = vec![0u8; info.d_size as usize];
            self.decoder
                .decompress_frame(&mut self.reader, frame_idx, &mut decomp)
                .map_err(|e| io::Error::other(e.to_string()))?;
            self.current_frame = Some((frame_idx, decomp));
        }

        let info = self.decoder.get_frame(frame_idx).unwrap();
        let (_cached_idx, ref cached_buf) = self.current_frame.as_ref().unwrap();

        let offset_in_frame = (self.pos - info.d_offset) as usize;
        let available = cached_buf.len().saturating_sub(offset_in_frame);
        let to_read = available.min(buf.len());

        buf[..to_read].copy_from_slice(&cached_buf[offset_in_frame..offset_in_frame + to_read]);
        self.pos += to_read as u64;
        Ok(to_read)
    }
}

impl<R: Read + Seek> Seek for ZstdSeekableReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let total = self.decoder.total_decompressed_size() as i64;
        let new_pos = match pos {
            SeekFrom::Start(off) => off as i64,
            SeekFrom::Current(off) => (self.pos as i64) + off,
            SeekFrom::End(off) => total + off,
        };

        if new_pos < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot seek to negative position",
            ));
        }

        self.pos = new_pos as u64;
        Ok(self.pos)
    }
}
