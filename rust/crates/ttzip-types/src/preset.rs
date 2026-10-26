// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Standard compression presets and automatic dictionary/algorithm parameter tuning.

use serde::{Deserialize, Serialize};
use super::formats::TTZipArchiveFormat;

/// Canonical compression preset levels.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TTZipCompressionPreset {
    Store = 0,
    Fast = 1,
    Balanced = 2,
    Maximum = 3,
    Ultra = 4,
}

impl TTZipCompressionPreset {
    /// Returns the canonical string identifier for this preset.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Store => "store",
            Self::Fast => "fast",
            Self::Balanced => "balanced",
            Self::Maximum => "maximum",
            Self::Ultra => "ultra",
        }
    }

    /// Parses preset identifier from case-insensitive string.
    #[must_use]
    pub fn parse_preset(s: &str) -> Option<Self> {
        let lower = s.trim().to_ascii_lowercase();
        match lower.as_str() {
            "store" | "none" | "0" => Some(Self::Store),
            "fast" | "fastest" | "1" | "speed" => Some(Self::Fast),
            "balanced" | "normal" | "standard" | "medium" | "default" | "6" => Some(Self::Balanced),
            "maximum" | "max" | "high" | "9" => Some(Self::Maximum),
            "ultra" | "extreme" | "best" | "12" | "22" => Some(Self::Ultra),
            _ => None,
        }
    }

    /// Returns the recommended integer compression level for the given archive format.
    #[must_use]
    pub fn recommended_level(self, format: TTZipArchiveFormat) -> i32 {
        match self {
            Self::Store => 0,
            Self::Fast => match format {
                TTZipArchiveFormat::Tar
                | TTZipArchiveFormat::Dmg
                | TTZipArchiveFormat::Iso
                | TTZipArchiveFormat::Aar
                | TTZipArchiveFormat::Cpio
                | TTZipArchiveFormat::Ar
                | TTZipArchiveFormat::Rar
                | TTZipArchiveFormat::Squashfs
                | TTZipArchiveFormat::Lzh => 0,
                TTZipArchiveFormat::Zstd | TTZipArchiveFormat::TarZstd => 1,
                TTZipArchiveFormat::Brotli | TTZipArchiveFormat::TarBrotli => 1,
                TTZipArchiveFormat::Lz4 | TTZipArchiveFormat::TarLz4 => 1,
                _ => 1,
            },
            Self::Balanced => match format {
                TTZipArchiveFormat::Tar
                | TTZipArchiveFormat::Dmg
                | TTZipArchiveFormat::Iso
                | TTZipArchiveFormat::Aar
                | TTZipArchiveFormat::Cpio
                | TTZipArchiveFormat::Ar
                | TTZipArchiveFormat::Rar
                | TTZipArchiveFormat::Squashfs
                | TTZipArchiveFormat::Lzh => 0,
                TTZipArchiveFormat::Zstd | TTZipArchiveFormat::TarZstd => 3,
                TTZipArchiveFormat::Brotli | TTZipArchiveFormat::TarBrotli => 4,
                TTZipArchiveFormat::Lz4 | TTZipArchiveFormat::TarLz4 => 1,
                _ => 6,
            },
            Self::Maximum => match format {
                TTZipArchiveFormat::Tar
                | TTZipArchiveFormat::Dmg
                | TTZipArchiveFormat::Iso
                | TTZipArchiveFormat::Aar
                | TTZipArchiveFormat::Cpio
                | TTZipArchiveFormat::Ar
                | TTZipArchiveFormat::Rar
                | TTZipArchiveFormat::Squashfs
                | TTZipArchiveFormat::Lzh => 0,
                TTZipArchiveFormat::Zstd | TTZipArchiveFormat::TarZstd => 9,
                TTZipArchiveFormat::Brotli | TTZipArchiveFormat::TarBrotli => 9,
                TTZipArchiveFormat::Lz4 | TTZipArchiveFormat::TarLz4 => 9,
                _ => 9,
            },
            Self::Ultra => match format {
                TTZipArchiveFormat::Tar
                | TTZipArchiveFormat::Dmg
                | TTZipArchiveFormat::Iso
                | TTZipArchiveFormat::Aar
                | TTZipArchiveFormat::Cpio
                | TTZipArchiveFormat::Ar
                | TTZipArchiveFormat::Rar
                | TTZipArchiveFormat::Squashfs
                | TTZipArchiveFormat::Lzh => 0,
                TTZipArchiveFormat::Zstd | TTZipArchiveFormat::TarZstd => 19,
                TTZipArchiveFormat::Brotli | TTZipArchiveFormat::TarBrotli => 11,
                TTZipArchiveFormat::Lz4 | TTZipArchiveFormat::TarLz4 => 9,
                TTZipArchiveFormat::Zip => 12,
                _ => 9,
            },
        }
    }

    /// Resolves detailed tuning parameters including dictionary sizes and match finder.
    #[must_use]
    pub fn resolve_parameters(
        self,
        format: TTZipArchiveFormat,
        total_ram_mb: Option<u64>,
        cpu_cores: Option<u32>,
    ) -> TTZipResolvedPresetParameters {
        let ram_mb = total_ram_mb.unwrap_or(8192);
        let cores = cpu_cores.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get() as u32)
                .unwrap_or(4)
        });

        let level = self.recommended_level(format);

        let (dict_mb, solid_mb, match_finder) = match format {
            TTZipArchiveFormat::SevenZip => match self {
                Self::Store => (0, 0, "None"),
                Self::Fast => (16, 64, "HC4"),
                Self::Balanced => {
                    let dict = if ram_mb >= 8192 { 64 } else { 32 };
                    (dict, 256, "BT4")
                }
                Self::Maximum => {
                    let dict = if ram_mb >= 16384 { 128 } else if ram_mb >= 8192 { 64 } else { 32 };
                    (dict, 1024, "BT4")
                }
                Self::Ultra => {
                    let dict = if ram_mb >= 32768 { 256 } else if ram_mb >= 16384 { 128 } else { 64 };
                    (dict, 4096, "BT4")
                }
            },
            TTZipArchiveFormat::Zstd | TTZipArchiveFormat::TarZstd => match self {
                Self::Store => (0, 0, "None"),
                Self::Fast => (4, 0, "Fast"),
                Self::Balanced => (8, 0, "Dfast"),
                Self::Maximum => (32, 0, "Btopt"),
                Self::Ultra => (128, 0, "Btultra"),
            },
            _ => (0, 0, "Standard"),
        };

        TTZipResolvedPresetParameters {
            level,
            dictionary_size_mb: dict_mb,
            solid_block_size_mb: solid_mb,
            num_threads: cores.clamp(1, 64),
            match_finder,
        }
    }
}

/// Resolved parameters for physical compression pipeline execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TTZipResolvedPresetParameters {
    pub level: i32,
    pub dictionary_size_mb: u32,
    pub solid_block_size_mb: u32,
    pub num_threads: u32,
    pub match_finder: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_parsing_and_levels() {
        assert_eq!(TTZipCompressionPreset::parse_preset("fast"), Some(TTZipCompressionPreset::Fast));
        assert_eq!(TTZipCompressionPreset::parse_preset("BALANCED"), Some(TTZipCompressionPreset::Balanced));
        assert_eq!(TTZipCompressionPreset::parse_preset("Ultra"), Some(TTZipCompressionPreset::Ultra));
        assert_eq!(TTZipCompressionPreset::parse_preset("nonexistent"), None);

        let seven_z = TTZipArchiveFormat::SevenZip;
        assert_eq!(TTZipCompressionPreset::Store.recommended_level(seven_z), 0);
        assert_eq!(TTZipCompressionPreset::Fast.recommended_level(seven_z), 1);
        assert_eq!(TTZipCompressionPreset::Balanced.recommended_level(seven_z), 6);
        assert_eq!(TTZipCompressionPreset::Maximum.recommended_level(seven_z), 9);
        assert_eq!(TTZipCompressionPreset::Ultra.recommended_level(seven_z), 9);

        let zip = TTZipArchiveFormat::Zip;
        assert_eq!(TTZipCompressionPreset::Ultra.recommended_level(zip), 12);

        let zstd = TTZipArchiveFormat::Zstd;
        assert_eq!(TTZipCompressionPreset::Ultra.recommended_level(zstd), 19);
    }

    #[test]
    fn test_format_capabilities_matrix() {
        let zip = TTZipArchiveFormat::Zip;
        assert!(zip.supports_password());
        assert!(zip.supports_split_volume());
        assert_eq!(zip.compression_level_range(), (0, 12));
        assert_eq!(zip.default_compression_level(), 6);
        assert!(zip.validate_configuration(6, true, true).is_ok());
        assert!(zip.validate_configuration(13, false, false).is_err());

        let tar = TTZipArchiveFormat::Tar;
        assert!(!tar.supports_password());
        assert!(!tar.supports_split_volume());
        assert!(tar.supports_solid_mode());
        assert_eq!(tar.compression_level_range(), (0, 0));
        assert!(tar.validate_configuration(0, false, false).is_ok());
        assert!(tar.validate_configuration(0, true, false).is_err());
    }
}
