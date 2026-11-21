package com.voicekey.app

import com.voicekey.app.audio.VoiceKeyAudioRecorder
import org.junit.Assert.*
import org.junit.Test
import kotlin.math.abs
import kotlin.math.sqrt

class VoiceKeyAudioRecorderTest {

    @Test
    fun testAudioRecorderInitialState() {
        val recorder = VoiceKeyAudioRecorder(sampleRate = 16000, preRollMs = 200)
        assertFalse(recorder.isRecording())
        val captured = recorder.stop()
        assertNotNull(captured)
        assertEquals(0, captured.size)
    }

    @Test
    fun testAmplitudeCalculationBounds() {
        // Test amplitude formula on mock audio samples
        val samples = shortArrayOf(0, 1000, -2500, 16000, -32767, 500)
        var sumSquares = 0.0
        var maxPeak = 0
        for (s in samples) {
            val absVal = abs(s.toInt())
            if (absVal > maxPeak) maxPeak = absVal
            sumSquares += s.toDouble() * s.toDouble()
        }
        val rms = sqrt(sumSquares / samples.size) / 32768.0
        val peak = maxPeak / 32768.0f
        val normalizedAmp = (rms * 3.0 + peak * 0.5).coerceIn(0.0, 1.0).toFloat()

        assertTrue(normalizedAmp in 0.0f..1.0f)
        assertTrue(normalizedAmp > 0.0f)
    }

    @Test
    fun testSilentAudioAmplitudeBounds() {
        val samples = shortArrayOf(0, 0, 0, 0, 0)
        var sumSquares = 0.0
        var maxPeak = 0
        for (s in samples) {
            val absVal = abs(s.toInt())
            if (absVal > maxPeak) maxPeak = absVal
            sumSquares += s.toDouble() * s.toDouble()
        }
        val rms = sqrt(sumSquares / samples.size) / 32768.0
        val peak = maxPeak / 32768.0f
        val normalizedAmp = (rms * 3.0 + peak * 0.5).coerceIn(0.0, 1.0).toFloat()

        assertEquals(0.0f, normalizedAmp, 0.0001f)
    }
}
