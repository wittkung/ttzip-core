// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Universal Character Set Detection Codec.
//!
//! Provides automated charset detection for legacy archives (GB18030, Shift-JIS, Big5, EUC-KR, Windows-1252, etc.).

use crate::types::TTZipStatus;

pub type DetectCharsetHook = fn(data: &[u8]) -> Option<String>;
static DETECT_CHARSET_HOOK: parking_lot::RwLock<Option<DetectCharsetHook>> =
    parking_lot::RwLock::new(None);

/// Registers an external charset detection callback hook (e.g., from ttzip-engine charset module).
pub fn register_detect_charset_hook(hook: DetectCharsetHook) {
    *DETECT_CHARSET_HOOK.write() = Some(hook);
}

fn native_detect_charset(data: &[u8]) -> Option<String> {
    if let Some(hook) = *DETECT_CHARSET_HOOK.read() {
        return hook(data);
    }
    if data.is_empty() || data.is_ascii() {
        return Some("ASCII".to_string());
    }
    if data.starts_with(b"\xEF\xBB\xBF") || std::str::from_utf8(data).is_ok() {
        return Some("UTF-8".to_string());
    }
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(data, true);
    let (encoding, _) = detector.guess_assess(None, true);
    Some(encoding.name().to_string())
}

/// Pure Safe Rust Universal Charset Detector.
#[derive(Debug, Default)]
pub struct CharsetDetector {
    buffer: Vec<u8>,
}

impl CharsetDetector {
    /// Creates a new character set detector instance.
    pub fn new() -> Result<Self, TTZipStatus> {
        Ok(Self {
            buffer: Vec::with_capacity(1024),
        })
    }

    /// Feeds arbitrary binary or text data into the detector.
    pub fn handle_data(&mut self, data: &[u8]) -> Result<(), TTZipStatus> {
        self.buffer.extend_from_slice(data);
        Ok(())
    }

    /// Notifies the detector of the end of input stream.
    pub fn data_end(&mut self) {}

    /// Resets the detector state for another stream.
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    /// Retrieves detected character encoding name, or None if undetermined.
    pub fn detected_charset(&self) -> Option<String> {
        native_detect_charset(&self.buffer)
    }
}

/// One-shot detection helper for raw byte buffers.
pub fn detect_charset(data: &[u8]) -> Option<String> {
    native_detect_charset(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_detection() {
        let utf8_text = "这是 TTZip 原生字符集探测测试，包含中文与 Emoji 🚀".as_bytes();
        let detected = detect_charset(utf8_text);
        assert!(detected.is_some());
        let name = detected.unwrap().to_uppercase();
        assert!(name.contains("UTF-8") || name.contains("UTF8"));
    }

    #[test]
    fn test_detector_reuse_with_reset() {
        let mut detector = CharsetDetector::new().expect("create detector");

        let text1 = "Hello world UTF-8 text".as_bytes();
        detector.handle_data(text1).unwrap();
        detector.data_end();
        let _ = detector.detected_charset();

        detector.reset();

        let text2 = "另一段中文测试数据，用于测试 reset 状态机重用".as_bytes();
        detector.handle_data(text2).unwrap();
        detector.data_end();
        let detected2 = detector.detected_charset();
        assert!(detected2.is_some());
    }
}
