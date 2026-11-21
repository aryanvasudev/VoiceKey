//! Adaptive Voice Activity Detection with silence hysteresis for real-time streaming.
//!
//! Provides a state machine that tracks speech onset, sustained voice,
//! pause buffering (silence hysteresis), and utterance completion.

use crate::vad::EnergyVad;

/// Events emitted by the Adaptive VAD state machine during audio frame processing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VadEvent {
    /// Silence detected (no active speech).
    Silence,
    /// Active speech onset detected (speech started).
    SpeechStarted,
    /// Sustained active speech in progress.
    Speaking { rms: f32, peak: f32 },
    /// Silence detected after speech, but within hysteresis threshold (e.g. natural word pause).
    SilencePending { elapsed_silence_ms: u32 },
    /// Utterance complete: sustained silence exceeded hysteresis timeout after active speech.
    UtteranceComplete { total_speech_duration_ms: u32 },
}

/// Configuration for adaptive streaming VAD with silence hysteresis.
#[derive(Debug, Clone, Copy)]
pub struct AdaptiveVadConfig {
    /// Frame size in samples (e.g. 512 samples = 32ms at 16kHz).
    pub frame_size: usize,
    /// Energy threshold to trigger speech start (default: 0.002 ~ -54 dBFS).
    pub speech_start_threshold: f32,
    /// Lower energy threshold to maintain speech state during soft phonemes (default: 0.0008 ~ -62 dBFS).
    pub speech_hold_threshold: f32,
    /// Consecutive speech frames needed to confirm speech onset (default: 3 frames = ~96ms).
    pub min_speech_frames: usize,
    /// Consecutive silence frames required to declare utterance completion (default: 25 frames = ~800ms).
    pub silence_hysteresis_frames: usize,
    /// Audio sample rate in Hz (default: 16000).
    pub sample_rate: u32,
}

impl Default for AdaptiveVadConfig {
    fn default() -> Self {
        Self {
            frame_size: 512,
            speech_start_threshold: 0.002,
            speech_hold_threshold: 0.0008,
            min_speech_frames: 3,
            silence_hysteresis_frames: 25, // 25 frames * 32ms = 800ms
            sample_rate: 16000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VadState {
    Idle,
    SpeechStarting {
        consecutive_speech_frames: usize,
    },
    Speaking {
        total_speech_frames: usize,
    },
    SilenceHysteresis {
        silence_frames: usize,
        total_speech_frames: usize,
    },
}

/// Adaptive streaming VAD detector.
#[derive(Debug, Clone)]
pub struct AdaptiveVad {
    config: AdaptiveVadConfig,
    state: VadState,
}

impl AdaptiveVad {
    /// Create a new AdaptiveVad with configuration.
    pub fn new(config: AdaptiveVadConfig) -> Self {
        Self {
            config,
            state: VadState::Idle,
        }
    }

    /// Reset the detector state back to Idle.
    pub fn reset(&mut self) {
        self.state = VadState::Idle;
    }

    /// Access current configuration.
    pub fn config(&self) -> &AdaptiveVadConfig {
        &self.config
    }

    /// Check if currently in an active speech or speech-hold state.
    pub fn is_speaking(&self) -> bool {
        matches!(
            self.state,
            VadState::Speaking { .. } | VadState::SilenceHysteresis { .. }
        )
    }

    /// Calculate frame duration in milliseconds.
    fn frame_ms(&self) -> f32 {
        (self.config.frame_size as f32 / self.config.sample_rate as f32) * 1000.0
    }

    /// Process a single frame of audio samples and return the detected VadEvent.
    pub fn process_frame(&mut self, frame: &[f32]) -> VadEvent {
        let rms = EnergyVad::calculate_rms(frame);
        let peak = EnergyVad::calculate_peak(frame);
        let frame_ms = self.frame_ms();

        match self.state {
            VadState::Idle => {
                if rms >= self.config.speech_start_threshold {
                    if self.config.min_speech_frames <= 1 {
                        self.state = VadState::Speaking {
                            total_speech_frames: 1,
                        };
                        VadEvent::SpeechStarted
                    } else {
                        self.state = VadState::SpeechStarting {
                            consecutive_speech_frames: 1,
                        };
                        VadEvent::Silence
                    }
                } else {
                    VadEvent::Silence
                }
            }
            VadState::SpeechStarting {
                consecutive_speech_frames,
            } => {
                if rms >= self.config.speech_hold_threshold {
                    let count = consecutive_speech_frames + 1;
                    if count >= self.config.min_speech_frames {
                        self.state = VadState::Speaking {
                            total_speech_frames: count,
                        };
                        VadEvent::SpeechStarted
                    } else {
                        self.state = VadState::SpeechStarting {
                            consecutive_speech_frames: count,
                        };
                        VadEvent::Silence
                    }
                } else {
                    // Transient click or cough below hold threshold
                    self.state = VadState::Idle;
                    VadEvent::Silence
                }
            }
            VadState::Speaking {
                total_speech_frames,
            } => {
                if rms >= self.config.speech_hold_threshold {
                    self.state = VadState::Speaking {
                        total_speech_frames: total_speech_frames + 1,
                    };
                    VadEvent::Speaking { rms, peak }
                } else if self.config.silence_hysteresis_frames <= 1 {
                    let total_ms = (total_speech_frames as f32 * frame_ms) as u32;
                    self.state = VadState::Idle;
                    VadEvent::UtteranceComplete {
                        total_speech_duration_ms: total_ms,
                    }
                } else {
                    self.state = VadState::SilenceHysteresis {
                        silence_frames: 1,
                        total_speech_frames,
                    };
                    let elapsed_silence_ms = (1.0 * frame_ms) as u32;
                    VadEvent::SilencePending { elapsed_silence_ms }
                }
            }
            VadState::SilenceHysteresis {
                silence_frames,
                total_speech_frames,
            } => {
                if rms >= self.config.speech_hold_threshold {
                    // User resumed speaking within hysteresis pause window
                    self.state = VadState::Speaking {
                        total_speech_frames: total_speech_frames + silence_frames + 1,
                    };
                    VadEvent::Speaking { rms, peak }
                } else {
                    let new_silence = silence_frames + 1;
                    if new_silence >= self.config.silence_hysteresis_frames {
                        let total_ms = (total_speech_frames as f32 * frame_ms) as u32;
                        self.state = VadState::Idle;
                        VadEvent::UtteranceComplete {
                            total_speech_duration_ms: total_ms,
                        }
                    } else {
                        self.state = VadState::SilenceHysteresis {
                            silence_frames: new_silence,
                            total_speech_frames,
                        };
                        let elapsed_silence_ms = (new_silence as f32 * frame_ms) as u32;
                        VadEvent::SilencePending { elapsed_silence_ms }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_vad_lifecycle() {
        let config = AdaptiveVadConfig {
            frame_size: 100,
            speech_start_threshold: 0.1,
            speech_hold_threshold: 0.05,
            min_speech_frames: 2,
            silence_hysteresis_frames: 3,
            sample_rate: 1000, // 100 samples = 100ms per frame
        };
        let mut vad = AdaptiveVad::new(config);

        let silence = vec![0.0f32; 100];
        let speech = vec![0.2f32; 100];
        let soft_speech = vec![0.07f32; 100];

        // 1. Initial silence
        assert_eq!(vad.process_frame(&silence), VadEvent::Silence);
        assert!(!vad.is_speaking());

        // 2. Speech frame 1 (starting, not confirmed yet)
        assert_eq!(vad.process_frame(&speech), VadEvent::Silence);

        // 3. Speech frame 2 (confirmed -> SpeechStarted)
        assert_eq!(vad.process_frame(&speech), VadEvent::SpeechStarted);
        assert!(vad.is_speaking());

        // 4. Soft speech (above hold threshold -> Speaking)
        match vad.process_frame(&soft_speech) {
            VadEvent::Speaking { rms, .. } => assert!(rms > 0.06),
            other => panic!("Expected Speaking, got {:?}", other),
        }

        // 5. Silence frame 1 (SilencePending)
        match vad.process_frame(&silence) {
            VadEvent::SilencePending { elapsed_silence_ms } => {
                assert_eq!(elapsed_silence_ms, 100);
            }
            other => panic!("Expected SilencePending, got {:?}", other),
        }

        // 6. Speech resumes before hysteresis expires
        match vad.process_frame(&speech) {
            VadEvent::Speaking { .. } => {}
            other => panic!("Expected resumed Speaking, got {:?}", other),
        }

        // 7. Three consecutive silence frames trigger UtteranceComplete
        let _ = vad.process_frame(&silence); // silence 1
        let _ = vad.process_frame(&silence); // silence 2
        let complete = vad.process_frame(&silence); // silence 3 (exceeds hysteresis)
        match complete {
            VadEvent::UtteranceComplete {
                total_speech_duration_ms,
            } => {
                assert!(total_speech_duration_ms > 0);
            }
            other => panic!("Expected UtteranceComplete, got {:?}", other),
        }
        assert!(!vad.is_speaking());
    }

    #[test]
    fn test_adaptive_vad_single_frame_thresholds() {
        let config = AdaptiveVadConfig {
            frame_size: 100,
            speech_start_threshold: 0.1,
            speech_hold_threshold: 0.05,
            min_speech_frames: 1,
            silence_hysteresis_frames: 1,
            sample_rate: 1000,
        };
        let mut vad = AdaptiveVad::new(config);

        let speech = vec![0.2f32; 100];
        let silence = vec![0.0f32; 100];

        // Frame 1 speech -> should immediately emit SpeechStarted
        assert_eq!(vad.process_frame(&speech), VadEvent::SpeechStarted);
        assert!(vad.is_speaking());

        // Frame 2 silence -> hysteresis is 1, should immediately emit UtteranceComplete
        match vad.process_frame(&silence) {
            VadEvent::UtteranceComplete {
                total_speech_duration_ms,
            } => {
                assert_eq!(total_speech_duration_ms, 100);
            }
            other => panic!("Expected immediate UtteranceComplete, got {:?}", other),
        }
        assert!(!vad.is_speaking());
    }
}
