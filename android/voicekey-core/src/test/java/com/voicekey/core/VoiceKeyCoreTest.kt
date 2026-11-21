package com.voicekey.core

import com.voicekey.core.ffi.VoiceKeyFfiException
import org.junit.Assert.*
import org.junit.Test

class VoiceKeyCoreTest {

    @Test
    fun testVersionRetrieval() {
        val ver = VoiceKeyCore.version()
        assertNotNull(ver)
        assertEquals("0.2.0", ver)
    }

    @Test
    fun testTextNormalization() {
        val input = "   this   is a   spaced   test.  "
        val normalized = VoiceKeyCore.normalizeText(input)
        assertEquals("This is a spaced test.", normalized)
    }

    @Test
    fun testSpokenPunctuation() {
        val input = "hello world period this is voicekey comma ok question mark"
        val processed = VoiceKeyCore.processText(
            rawText = input,
            enableNormalization = true,
            enableSpokenPunctuation = true
        )
        assertEquals("Hello world. This is voicekey, ok?", processed)
    }

    @Test
    fun testPcm16ToSamplesConversion() {
        // Construct 100 samples of 16-bit PCM at 16000Hz mono (amplitude 16384 -> 0.5f)
        val sampleVal: Short = 16384
        val bytes = ByteArray(200)
        for (i in 0 until 100) {
            bytes[i * 2] = (sampleVal.toInt() and 0xFF).toByte()
            bytes[i * 2 + 1] = ((sampleVal.toInt() shr 8) and 0xFF).toByte()
        }

        val samples = VoiceKeyCore.pcm16ToSamples(bytes, 16000, 1)
        assertEquals(100, samples.size)
        assertTrue(kotlin.math.abs(samples[0] - 0.5f) < 0.001f)

        val rms = VoiceKeyCore.calculateRms(samples)
        val peak = VoiceKeyCore.calculatePeak(samples)
        assertTrue(kotlin.math.abs(rms - 0.5f) < 0.01f)
        assertTrue(kotlin.math.abs(peak - 0.5f) < 0.01f)
    }

    @Test
    fun testVadDetection() {
        val silence = FloatArray(16000) { 0.0f }
        assertFalse(VoiceKeyCore.checkSpeech(silence))

        // Create artificial audio with tone burst
        val speech = FloatArray(16000) { 0.0f }
        for (i in 4000 until 12000) {
            speech[i] = (0.5 * kotlin.math.sin(i * 0.1)).toFloat()
        }
        assertTrue(VoiceKeyCore.checkSpeech(speech))

        val trimmed = VoiceKeyCore.trimAudioSilence(speech)
        assertTrue(trimmed.size < speech.size)
    }

    @Test
    fun testInvalidModelPathThrowsModelNotFound() {
        try {
            VoiceKeyCore.create("/nonexistent/path/to/model.bin")
            fail("Expected VoiceKeyFfiException was not thrown")
        } catch (e: VoiceKeyFfiException) {
            assertTrue(e is VoiceKeyFfiException.ModelNotFound)
        }
    }
}
