package com.voicekey.core

import com.voicekey.core.ffi.*

/**
 * Idiomatic Kotlin wrapper for VoiceKey Core speech-to-text and DSP engine.
 */
class VoiceKeyCore private constructor(
    private val engine: VoiceKeyEngine
) {

    /**
     * Transcribe 16kHz mono Float32 audio samples.
     *
     * @param samples Audio samples in range [-1.0, 1.0] at 16000Hz mono.
     * @param initialPrompt Optional vocabulary priming prompt for jargon or hotwords.
     * @return [TranscriptionOutput] containing processed text and timing segments.
     */
    @Throws(VoiceKeyFfiException::class)
    fun transcribe(
        samples: FloatArray,
        initialPrompt: String? = null
    ): TranscriptionOutput {
        return engine.transcribe(samples.toList(), initialPrompt)
    }

    /**
     * Transcribe raw PCM 16-bit little-endian audio bytes (as emitted by Android AudioRecord).
     * Automatically handles downmixing from stereo and resampling to 16kHz.
     *
     * @param pcmBytes Raw 16-bit PCM bytes.
     * @param sampleRate Input sample rate (e.g. 16000, 44100, 48000).
     * @param channels Number of audio channels (1 for mono, 2 for stereo).
     * @param initialPrompt Optional vocabulary priming prompt.
     * @return [TranscriptionOutput] with normalized text.
     */
    @Throws(VoiceKeyFfiException::class)
    fun transcribePcm16(
        pcmBytes: ByteArray,
        sampleRate: Int = 16000,
        channels: Int = 1,
        initialPrompt: String? = null
    ): TranscriptionOutput {
        return engine.transcribePcm16(
            pcmBytes,
            sampleRate.toUInt(),
            channels.toUShort(),
            initialPrompt
        )
    }

    /**
     * Check if speech activity is present in the provided 16kHz audio samples.
     */
    fun hasSpeech(samples: FloatArray): Boolean {
        return engine.hasSpeech(samples.toList())
    }

    /**
     * Trim leading and trailing silence frames from audio samples.
     */
    fun trimSilence(samples: FloatArray): FloatArray {
        return engine.trimSilence(samples.toList()).toFloatArray()
    }

    companion object {
        /**
         * Initialize a new VoiceKeyCore instance with a GGML model file on disk.
         *
         * @param modelPath Absolute file path to quantized GGML/GGUF model (e.g. ggml-tiny.en.bin).
         * @param threads Number of CPU worker threads for inference (recommended 4 on ARM64).
         * @param language ISO language code ("en" or null for auto-detect).
         * @param translate Whether to translate audio to English.
         * @param enableNormalization Clean whitespace and normalize sentence capitalization.
         * @param enableSpokenPunctuation Expand spoken words ("comma", "period", "newline").
         */
        @JvmStatic
        @Throws(VoiceKeyFfiException::class)
        fun create(
            modelPath: String,
            threads: Int = 4,
            language: String? = "en",
            translate: Boolean = false,
            enableNormalization: Boolean = true,
            enableSpokenPunctuation: Boolean = false
        ): VoiceKeyCore {
            val config = EngineConfig(
                threads = threads.toUInt(),
                language = language,
                translate = translate,
                enableNormalization = enableNormalization,
                enableSpokenPunctuation = enableSpokenPunctuation
            )
            val engine = VoiceKeyEngine(modelPath, config)
            return VoiceKeyCore(engine)
        }

        /**
         * Get VoiceKey Core version string.
         */
        @JvmStatic
        fun version(): String = voicekeyVersion()

        /**
         * Check speech activity using energy-based Voice Activity Detection (VAD).
         */
        @JvmStatic
        fun checkSpeech(samples: FloatArray): Boolean =
            voicekeyHasSpeech(samples.toList())

        /**
         * Trim silence from 16kHz audio samples.
         */
        @JvmStatic
        fun trimAudioSilence(samples: FloatArray): FloatArray =
            voicekeyTrimSilence(samples.toList()).toFloatArray()

        /**
         * Calculate RMS energy of audio samples.
         */
        @JvmStatic
        fun calculateRms(samples: FloatArray): Float =
            voicekeyCalculateRms(samples.toList())

        /**
         * Calculate absolute peak amplitude of audio samples.
         */
        @JvmStatic
        fun calculatePeak(samples: FloatArray): Float =
            voicekeyCalculatePeak(samples.toList())

        /**
         * Convert 16-bit PCM bytes to 16kHz mono Float32 samples.
         */
        @JvmStatic
        @Throws(VoiceKeyFfiException::class)
        fun pcm16ToSamples(
            pcmBytes: ByteArray,
            sampleRate: Int = 16000,
            channels: Int = 1
        ): FloatArray {
            return voicekeyPcm16ToSamples(
                pcmBytes,
                sampleRate.toUInt(),
                channels.toUShort()
            ).toFloatArray()
        }

        /**
         * Post-process text with punctuation and normalization.
         */
        @JvmStatic
        fun processText(
            rawText: String,
            enableNormalization: Boolean = true,
            enableSpokenPunctuation: Boolean = true
        ): String {
            return voicekeyProcessText(
                rawText,
                enableNormalization,
                enableSpokenPunctuation
            )
        }

        /**
         * Normalize whitespace and capitalization.
         */
        @JvmStatic
        fun normalizeText(text: String): String = voicekeyNormalizeText(text)

        /**
         * Convert spoken punctuation tokens to punctuation marks.
         */
        @JvmStatic
        fun applySpokenPunctuation(text: String): String =
            voicekeySpokenPunctuation(text)
    }
}
