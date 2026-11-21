//! WAV file encoding and decoding utilities using `hound`.

use std::path::Path;
use hound::{WavReader, WavSpec, WavWriter};
use crate::error::CoreError;

/// Save mono float32 audio samples to a 16-bit PCM WAV file.
pub fn write_wav<P: AsRef<Path>>(path: P, samples: &[f32], sample_rate: u32) -> Result<(), CoreError> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path.as_ref(), spec)
        .map_err(|e| CoreError::Audio(format!("Failed to create WAV file: {e}")))?;

    for &sample in samples {
        // Clamp to [-1.0, 1.0] and scale to 16-bit signed integer range [-32768, 32767]
        let clamped = sample.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0).round() as i16;
        writer
            .write_sample(sample_i16)
            .map_err(|e| CoreError::Audio(format!("Failed to write WAV sample: {e}")))?;
    }

    writer
        .finalize()
        .map_err(|e| CoreError::Audio(format!("Failed to finalize WAV file: {e}")))?;

    Ok(())
}

/// Read a WAV file and return mono normalized float32 samples with the recorded sample rate.
///
/// Automatically downmixes multi-channel WAVs to mono.
pub fn read_wav<P: AsRef<Path>>(path: P) -> Result<(Vec<f32>, u32), CoreError> {
    let mut reader = WavReader::open(path.as_ref())
        .map_err(|e| CoreError::Audio(format!("Failed to open WAV file: {e}")))?;

    let spec = reader.spec();
    let channels = spec.channels as usize;
    let sample_rate = spec.sample_rate;

    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|s| s.unwrap_or(0.0))
            .collect(),
        hound::SampleFormat::Int => {
            let max_val = match spec.bits_per_sample {
                8 => 128.0,
                16 => 32768.0,
                24 => 8388608.0,
                32 => 2147483648.0,
                _ => 32768.0,
            };
            reader
                .samples::<i32>()
                .map(|s| (s.unwrap_or(0) as f32 / max_val).clamp(-1.0, 1.0))
                .collect()
        }
    };

    let mono_samples = if channels > 1 {
        crate::audio::downmix_to_mono(&interleaved, channels)
    } else {
        interleaved
    };

    Ok((mono_samples, sample_rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_write_and_read_wav_roundtrip() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let original_samples = vec![0.0, 0.25, -0.5, 0.75, -1.0, 1.0];
        let sample_rate = 16000;

        write_wav(path, &original_samples, sample_rate).unwrap();

        let (read_samples, read_rate) = read_wav(path).unwrap();
        assert_eq!(read_rate, sample_rate);
        assert_eq!(read_samples.len(), original_samples.len());

        for (orig, read) in original_samples.iter().zip(read_samples.iter()) {
            assert!(
                (orig - read).abs() < 0.001,
                "Sample mismatch: orig={orig}, read={read}"
            );
        }
    }
}
