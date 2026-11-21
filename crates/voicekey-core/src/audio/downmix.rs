//! Multi-channel to mono audio downmixer.

/// Downmix an interleaved multi-channel PCM float audio buffer into a single mono channel.
///
/// Each output frame sample is the arithmetic mean across all channels for that frame:
/// `mono[i] = sum(samples[i * channels .. (i+1) * channels]) / channels`
///
/// Output samples are clamped to the standard `[-1.0, 1.0]` range.
pub fn downmix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 || interleaved.is_empty() {
        return interleaved.to_vec();
    }

    let num_frames = interleaved.len() / channels;
    let mut mono = Vec::with_capacity(num_frames);
    let inv_channels = 1.0 / channels as f32;

    for frame in interleaved.chunks_exact(channels) {
        let sum: f32 = frame.iter().sum();
        let sample = (sum * inv_channels).clamp(-1.0, 1.0);
        mono.push(sample);
    }

    mono
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_downmix() {
        assert_eq!(downmix_to_mono(&[], 2), Vec::<f32>::new());
    }

    #[test]
    fn test_single_channel_passthrough() {
        let input = vec![0.1, -0.2, 0.5];
        assert_eq!(downmix_to_mono(&input, 1), input);
    }

    #[test]
    fn test_stereo_downmix() {
        // Left: 0.5, Right: 0.5 -> 0.5
        // Left: 1.0, Right: -1.0 -> 0.0
        // Left: -0.4, Right: -0.2 -> -0.3
        let stereo = vec![0.5, 0.5, 1.0, -1.0, -0.4, -0.2];
        let mono = downmix_to_mono(&stereo, 2);
        assert_eq!(mono.len(), 3);
        assert!((mono[0] - 0.5).abs() < 1e-6);
        assert!((mono[1] - 0.0).abs() < 1e-6);
        assert!((mono[2] - (-0.3)).abs() < 1e-6);
    }

    #[test]
    fn test_clamping() {
        let stereo = vec![1.0, 1.5]; // average 1.25 -> clamped to 1.0
        let mono = downmix_to_mono(&stereo, 2);
        assert_eq!(mono[0], 1.0);
    }
}
