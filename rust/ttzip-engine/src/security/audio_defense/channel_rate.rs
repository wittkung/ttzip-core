// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Audio Channel Count and Sample Rate Boundary Guard.
//!
//! Intercepts illegal or hostile channel configurations (0 channels, > 8 channels)
//! and abnormal sample rates (< 8,000 Hz or > 192,000 Hz), preventing divide-by-zero
//! panics, integer overflow during frame size estimation, and uncontrolled heap exhaustion.

use super::{
    AudioDefenseError, DEFAULT_MAX_AUDIO_CHANNELS, DEFAULT_MAX_SAMPLE_RATE,
    DEFAULT_MIN_AUDIO_CHANNELS, DEFAULT_MIN_SAMPLE_RATE,
};

/// Configuration parameters for channel and sample rate validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelRateConfig {
    /// Minimum allowed channel count (typically 1).
    pub min_channels: u16,
    /// Maximum allowed channel count (typically 8: 7.1 surround).
    pub max_channels: u16,
    /// Minimum allowed sample rate in Hz (typically 8,000 Hz).
    pub min_sample_rate: u32,
    /// Maximum allowed sample rate in Hz (typically 192,000 Hz).
    pub max_sample_rate: u32,
}

impl Default for ChannelRateConfig {
    fn default() -> Self {
        Self {
            min_channels: DEFAULT_MIN_AUDIO_CHANNELS,
            max_channels: DEFAULT_MAX_AUDIO_CHANNELS,
            min_sample_rate: DEFAULT_MIN_SAMPLE_RATE,
            max_sample_rate: DEFAULT_MAX_SAMPLE_RATE,
        }
    }
}

/// Defensive guard validating audio channels and sample rate parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AudioChannelRateGuard {
    config: ChannelRateConfig,
}

impl AudioChannelRateGuard {
    /// Creates a new guard with default boundary parameters.
    pub const fn new() -> Self {
        Self {
            config: ChannelRateConfig {
                min_channels: DEFAULT_MIN_AUDIO_CHANNELS,
                max_channels: DEFAULT_MAX_AUDIO_CHANNELS,
                min_sample_rate: DEFAULT_MIN_SAMPLE_RATE,
                max_sample_rate: DEFAULT_MAX_SAMPLE_RATE,
            },
        }
    }

    /// Creates a new guard with custom boundary configuration.
    pub const fn with_config(config: ChannelRateConfig) -> Self {
        Self { config }
    }

    /// Validates both channel count and sample rate in a single call.
    pub fn validate(&self, channels: u16, sample_rate: u32) -> Result<(), AudioDefenseError> {
        self.validate_channels(channels)?;
        self.validate_sample_rate(sample_rate)?;
        Ok(())
    }

    /// Validates the audio channel count against the configured bounds.
    pub fn validate_channels(&self, channels: u16) -> Result<(), AudioDefenseError> {
        if channels < self.config.min_channels || channels > self.config.max_channels {
            return Err(AudioDefenseError::InvalidChannelCount {
                channels,
                min: self.config.min_channels,
                max: self.config.max_channels,
            });
        }
        Ok(())
    }

    /// Validates the audio sample rate against the configured bounds.
    pub fn validate_sample_rate(&self, sample_rate: u32) -> Result<(), AudioDefenseError> {
        if sample_rate < self.config.min_sample_rate || sample_rate > self.config.max_sample_rate {
            return Err(AudioDefenseError::InvalidSampleRate {
                sample_rate,
                min: self.config.min_sample_rate,
                max: self.config.max_sample_rate,
            });
        }
        Ok(())
    }

    /// Computes the uncompressed frame size in bytes given channels and bits per sample,
    /// ensuring arithmetic multiplication does not overflow `usize`.
    pub fn estimate_frame_size(
        &self,
        channels: u16,
        bits_per_sample: u16,
    ) -> Result<usize, AudioDefenseError> {
        self.validate_channels(channels)?;

        if bits_per_sample == 0 || bits_per_sample > 64 {
            return Err(AudioDefenseError::FrameSizeOverflow {
                channels,
                bits_per_sample,
            });
        }

        let bytes_per_sample = (bits_per_sample.div_ceil(8)) as usize;
        let channels_usize = channels as usize;

        channels_usize
            .checked_mul(bytes_per_sample)
            .ok_or(AudioDefenseError::FrameSizeOverflow {
                channels,
                bits_per_sample,
            })
    }

    /// Computes the memory volume for a buffer holding `frame_count` frames,
    /// ensuring safe checked multiplication.
    pub fn estimate_buffer_size(
        &self,
        channels: u16,
        bits_per_sample: u16,
        frame_count: usize,
    ) -> Result<usize, AudioDefenseError> {
        let frame_size = self.estimate_frame_size(channels, bits_per_sample)?;
        frame_size
            .checked_mul(frame_count)
            .ok_or(AudioDefenseError::FrameSizeOverflow {
                channels,
                bits_per_sample,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_channel_and_sample_rates() {
        let guard = AudioChannelRateGuard::new();

        // Standard stereo 44.1kHz
        assert!(guard.validate(2, 44_100).is_ok());

        // Mono 8kHz
        assert!(guard.validate(1, 8_000).is_ok());

        // 7.1 Surround 192kHz
        assert!(guard.validate(8, 192_000).is_ok());

        // 5.1 Surround 96kHz
        assert!(guard.validate(6, 96_000).is_ok());
    }

    #[test]
    fn test_invalid_channel_counts() {
        let guard = AudioChannelRateGuard::new();

        // 0 channels (divide-by-zero trigger)
        let err_zero = guard.validate_channels(0).unwrap_err();
        assert_eq!(
            err_zero,
            AudioDefenseError::InvalidChannelCount {
                channels: 0,
                min: 1,
                max: 8
            }
        );

        // 9 channels (exceeds 8)
        let err_nine = guard.validate_channels(9).unwrap_err();
        assert_eq!(
            err_nine,
            AudioDefenseError::InvalidChannelCount {
                channels: 9,
                min: 1,
                max: 8
            }
        );

        // 65535 channels (extreme heap exhaustion payload)
        assert!(guard.validate_channels(u16::MAX).is_err());
    }

    #[test]
    fn test_invalid_sample_rates() {
        let guard = AudioChannelRateGuard::new();

        // 0 Hz (divide-by-zero trigger)
        assert!(guard.validate_sample_rate(0).is_err());

        // 4000 Hz (< 8000 Hz lower boundary)
        let err_low = guard.validate_sample_rate(4_000).unwrap_err();
        assert_eq!(
            err_low,
            AudioDefenseError::InvalidSampleRate {
                sample_rate: 4_000,
                min: 8_000,
                max: 192_000
            }
        );

        // 192001 Hz (> 192000 Hz upper boundary)
        let err_high = guard.validate_sample_rate(192_001).unwrap_err();
        assert_eq!(
            err_high,
            AudioDefenseError::InvalidSampleRate {
                sample_rate: 192_001,
                min: 8_000,
                max: 192_000
            }
        );

        // Extreme sample rate (e.g. 11.2 MHz DSD256 raw PCM)
        assert!(guard.validate_sample_rate(11_289_600).is_err());
    }

    #[test]
    fn test_frame_size_and_buffer_estimation() {
        let guard = AudioChannelRateGuard::new();

        // 2 channels * 16-bit (2 bytes) = 4 bytes per frame
        let size = guard.estimate_frame_size(2, 16).unwrap();
        assert_eq!(size, 4);

        // 6 channels * 24-bit (3 bytes) = 18 bytes per frame
        let size_surround = guard.estimate_frame_size(6, 24).unwrap();
        assert_eq!(size_surround, 18);

        // 8 channels * 32-bit (4 bytes) = 32 bytes per frame
        let size_8ch = guard.estimate_frame_size(8, 32).unwrap();
        assert_eq!(size_8ch, 32);

        // 1000 frames buffer size
        let buf_size = guard.estimate_buffer_size(2, 16, 1000).unwrap();
        assert_eq!(buf_size, 4000);

        // 0 bits per sample rejected
        assert!(guard.estimate_frame_size(2, 0).is_err());

        // > 64 bits per sample rejected
        assert!(guard.estimate_frame_size(2, 128).is_err());
    }
}
