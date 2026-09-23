// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! UniFFI Compression Preset Management and Format Capabilities Matrix Scaffolding.

use super::types::{TTZipArchiveFormat, TTZipError};

/// Comprehensive archive format capabilities record exposed via UniFFI.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIFormatCapabilities {
    pub supports_password: bool,
    pub supports_split_volume: bool,
    pub supports_solid_mode: bool,
    pub min_level: i32,
    pub max_level: i32,
    pub default_level: i32,
}

/// Queries the capabilities and valid compression parameter envelope for the given archive format.
#[uniffi::export]
pub fn get_format_capabilities(format: TTZipArchiveFormat) -> UniFFIFormatCapabilities {
    let core_fmt: crate::types::TTZipArchiveFormat = format.into();
    let (min_level, max_level) = core_fmt.compression_level_range();
    UniFFIFormatCapabilities {
        supports_password: core_fmt.supports_password(),
        supports_split_volume: core_fmt.supports_split_volume(),
        supports_solid_mode: core_fmt.supports_solid_mode(),
        min_level,
        max_level,
        default_level: core_fmt.default_compression_level(),
    }
}

/// Validates compression parameters (level, password, split volume) against the format capability matrix.
#[uniffi::export]
pub fn validate_archive_params(
    format: TTZipArchiveFormat,
    level: i32,
    has_password: bool,
    is_split: bool,
) -> Result<(), TTZipError> {
    let core_fmt: crate::types::TTZipArchiveFormat = format.into();
    core_fmt
        .validate_configuration(level, has_password, is_split)
        .map_err(|status| match status {
            crate::types::TTZipStatus::ErrUnsupportedFeature => {
                if has_password && !core_fmt.supports_password() {
                    TTZipError::SecurityViolation {
                        reason: format!(
                            "Format '{}' does not support password encryption",
                            core_fmt.as_str()
                        ),
                    }
                } else {
                    TTZipError::EngineError {
                        code: crate::types::TTZipStatus::ErrUnsupportedFeature as i32,
                    }
                }
            }
            crate::types::TTZipStatus::ErrInvalidParam => TTZipError::EngineError {
                code: crate::types::TTZipStatus::ErrInvalidParam as i32,
            },
            _ => TTZipError::EngineError {
                code: status as i32,
            },
        })
}

/// Returns the recommended integer compression level for a given preset name and archive format.
#[uniffi::export]
pub fn get_recommended_preset_level(preset: String, format: TTZipArchiveFormat) -> i32 {
    let core_fmt: crate::types::TTZipArchiveFormat = format.into();
    let preset_enum = crate::types::TTZipCompressionPreset::parse_preset(&preset)
        .unwrap_or(crate::types::TTZipCompressionPreset::Balanced);
    preset_enum.recommended_level(core_fmt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniffi_format_capabilities() {
        let zip_caps = get_format_capabilities(TTZipArchiveFormat::Zip);
        assert!(zip_caps.supports_password);
        assert!(zip_caps.supports_split_volume);
        assert_eq!(zip_caps.min_level, 0);
        assert_eq!(zip_caps.max_level, 12);
        assert_eq!(zip_caps.default_level, 6);

        let tar_caps = get_format_capabilities(TTZipArchiveFormat::Tar);
        assert!(!tar_caps.supports_password);
        assert!(!tar_caps.supports_split_volume);
        assert!(tar_caps.supports_solid_mode);
        assert_eq!(tar_caps.min_level, 0);
        assert_eq!(tar_caps.max_level, 0);
        assert_eq!(tar_caps.default_level, 0);
    }

    #[test]
    fn test_uniffi_validate_archive_params() {
        assert!(validate_archive_params(TTZipArchiveFormat::Zip, 6, true, true).is_ok());
        assert!(validate_archive_params(TTZipArchiveFormat::Zip, 15, false, false).is_err());
        assert!(validate_archive_params(TTZipArchiveFormat::Tar, 0, true, false).is_err());
        assert!(validate_archive_params(TTZipArchiveFormat::Tar, 0, false, true).is_err());
        assert!(validate_archive_params(TTZipArchiveFormat::Tar, 0, false, false).is_ok());
    }

    #[test]
    fn test_uniffi_recommended_preset_level() {
        assert_eq!(get_recommended_preset_level("store".to_string(), TTZipArchiveFormat::SevenZip), 0);
        assert_eq!(get_recommended_preset_level("fast".to_string(), TTZipArchiveFormat::SevenZip), 1);
        assert_eq!(get_recommended_preset_level("balanced".to_string(), TTZipArchiveFormat::SevenZip), 6);
        assert_eq!(get_recommended_preset_level("ultra".to_string(), TTZipArchiveFormat::Zip), 12);
        assert_eq!(get_recommended_preset_level("ultra".to_string(), TTZipArchiveFormat::Zstd), 19);
    }
}
