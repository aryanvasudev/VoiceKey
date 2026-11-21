//! Parity test suite verifying voicekey-core matches Python v1 baseline logic.

use voicekey_core::audio::{downmix_to_mono, AudioResampler, PreRollBuffer};
use voicekey_core::text::{
    apply_spoken_punctuation, normalize_text, process_transcription, VocabularyPrompt,
};
use voicekey_core::vad::EnergyVad;

#[test]
fn test_spoken_punctuation_parity_with_python_v1() {
    let cases = vec![
        ("hello comma world period", "hello, world."),
        ("new line newline new paragraph", "\n\n\n\n"),
        ("what is this question mark", "what is this?"),
        ("warning exclamation mark", "warning!"),
        ("key colon value semicolon next hyphen item", "key: value; next- item"),
        (
            "this is a sentence period next sentence question mark yes exclamation point",
            "this is a sentence. Next sentence? Yes!",
        ),
    ];

    for (input, expected) in cases {
        let output = apply_spoken_punctuation(input);
        assert_eq!(
            output, expected,
            "Failed spoken punctuation parity for input: {input}"
        );
    }
}

#[test]
fn test_normalization_parity_with_python_v1() {
    let cases = vec![
        ("   leading and trailing   ", "Leading and trailing"),
        ("multiple     spaces     here", "Multiple spaces here"),
        ("line one \n   line two\n\n\n\nline three", "Line one\nline two\n\nline three"),
        ("lower case start", "Lower case start"),
        ("123 numbers unaffected", "123 numbers unaffected"),
        ("", ""),
        ("     ", ""),
    ];

    for (input, expected) in cases {
        let output = normalize_text(input);
        assert_eq!(
            output, expected,
            "Failed normalization parity for input: {input}"
        );
    }
}

#[test]
fn test_full_pipeline_parity_with_python_v1() {
    let raw = "   hello comma world newline this is voicekey period   ";
    // With both spoken punct and normalization:
    let output = process_transcription(raw, true, true);
    assert_eq!(output, "Hello, world\nThis is voicekey.");

    // With normalization only:
    let norm_only = process_transcription(raw, true, false);
    assert_eq!(
        norm_only,
        "Hello comma world newline this is voicekey period"
    );

    // With neither:
    let verbatim = process_transcription(raw, false, false);
    assert_eq!(verbatim, raw);
}

#[test]
fn test_vocabulary_biasing_prompt_construction() {
    let mut vocab = VocabularyPrompt::new().with_base_prompt("VoiceKey dictation context");
    vocab.add_hotwords(["Whisper", "GGML", "Tauri", "Wayland"]);

    let prompt = vocab.build_prompt();
    assert_eq!(
        prompt,
        "VoiceKey dictation context. GGML, Tauri, Wayland, Whisper"
    );
}

#[test]
fn test_end_to_end_dsp_pipeline() {
    // 1. Generate 1 second of 48,000 Hz Stereo audio
    // Left channel: 440 Hz tone with speech burst
    // Right channel: identical tone
    let sample_rate = 48000;
    let duration_secs = 1.0;
    let total_frames = (sample_rate as f64 * duration_secs) as usize;

    let mut stereo_interleaved = Vec::with_capacity(total_frames * 2);
    for i in 0..total_frames {
        let t = i as f32 / sample_rate as f32;
        // Silent first 300ms, speech 300ms-700ms, silent last 300ms
        let amplitude = if (0.3..0.7).contains(&t) {
            0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
        } else {
            0.0
        };
        stereo_interleaved.push(amplitude); // Left
        stereo_interleaved.push(amplitude); // Right
    }

    // 2. Downmix stereo -> mono
    let mono_48k = downmix_to_mono(&stereo_interleaved, 2);
    assert_eq!(mono_48k.len(), total_frames);

    // 3. Resample 48,000 Hz -> 16,000 Hz
    let resampler = AudioResampler::new(48000, 16000).expect("Resampler creation failed");
    let mono_16k = resampler.resample(&mono_48k).expect("Resampling failed");

    // Expected length ~16000 samples
    let expected_16k_len = 16000;
    let diff = (mono_16k.len() as isize - expected_16k_len as isize).abs();
    assert!(diff < 350, "Length was {} vs expected ~16000", mono_16k.len());

    // 4. PreRollBuffer buffering
    let mut pre_roll = PreRollBuffer::new(16000, 200); // 200ms = 3200 samples
    pre_roll.push_samples(&mono_16k[0..3200]);
    assert_eq!(pre_roll.len(), 3200);

    // 5. Dual VAD: Energy gating & speech detection
    let vad = EnergyVad::default();
    assert!(vad.has_speech(&mono_16k), "Should detect speech in tone burst");

    // 6. Silence trimming
    let trimmed = vad.trim_silence(&mono_16k);
    assert!(!trimmed.is_empty(), "Trimmed audio should not be empty");
    assert!(
        trimmed.len() < mono_16k.len(),
        "Trimmed audio should be shorter than original full second (was {}, original {})",
        trimmed.len(),
        mono_16k.len()
    );
}
