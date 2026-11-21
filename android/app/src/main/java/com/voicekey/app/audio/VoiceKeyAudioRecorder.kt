package com.voicekey.app.audio

import android.annotation.SuppressLint
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.util.Log
import java.io.ByteArrayOutputStream
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.thread
import kotlin.math.abs
import kotlin.math.sqrt

/**
 * High-performance 16kHz 16-bit mono PCM audio capture using Android AudioRecord.
 * Includes a 200ms pre-roll circular buffer to prevent clipping initial consonants.
 */
class VoiceKeyAudioRecorder(
    private val sampleRate: Int = 16000,
    private val preRollMs: Int = 200
) {
    private val TAG = "VoiceKeyAudioRecorder"

    private val channelConfig = AudioFormat.CHANNEL_IN_MONO
    private val audioFormat = AudioFormat.ENCODING_PCM_16BIT

    private val isRecording = AtomicBoolean(false)
    private var audioRecord: AudioRecord? = null
    private var recordingThread: Thread? = null

    // Pre-roll buffer: 16000 samples/sec * 2 bytes/sample * 0.200s = 6400 bytes
    private val preRollCapacityBytes = (sampleRate * 2 * (preRollMs / 1000.0)).toInt()
    private val preRollBuffer = ByteArray(preRollCapacityBytes)
    private var preRollHead = 0
    private var preRollFilled = false

    private val capturedAudio = ByteArrayOutputStream()
    private val lock = Any()

    /**
     * Push audio bytes into the circular pre-roll buffer when idle.
     */
    fun pushPreRoll(data: ByteArray, offset: Int = 0, length: Int = data.size) {
        synchronized(lock) {
            for (i in offset until offset + length) {
                preRollBuffer[preRollHead] = data[i]
                preRollHead = (preRollHead + 1) % preRollCapacityBytes
                if (preRollHead == 0) {
                    preRollFilled = true
                }
            }
        }
    }

    /**
     * Start recording audio from the microphone asynchronously.
     *
     * @param onAmplitude Optional callback for real-time normalized audio amplitude (0.0 to 1.0)
     */
    @SuppressLint("MissingPermission")
    @Synchronized
    fun start(onAmplitude: ((Float) -> Unit)? = null) {
        if (isRecording.get()) {
            Log.w(TAG, "Recording is already in progress.")
            return
        }

        val minBufferSize = AudioRecord.getMinBufferSize(sampleRate, channelConfig, audioFormat)
        val bufferSize = maxOf(minBufferSize, sampleRate * 2 / 10) // At least 100ms buffer

        val record = AudioRecord(
            MediaRecorder.AudioSource.MIC,
            sampleRate,
            channelConfig,
            audioFormat,
            bufferSize
        )

        if (record.state != AudioRecord.STATE_INITIALIZED) {
            record.release()
            throw IllegalStateException("Failed to initialize AudioRecord (state=${record.state})")
        }

        audioRecord = record
        isRecording.set(true)
        synchronized(lock) {
            capturedAudio.reset()
            // Append contents of pre-roll buffer if populated
            if (preRollFilled) {
                capturedAudio.write(preRollBuffer, preRollHead, preRollCapacityBytes - preRollHead)
                if (preRollHead > 0) {
                    capturedAudio.write(preRollBuffer, 0, preRollHead)
                }
            } else if (preRollHead > 0) {
                capturedAudio.write(preRollBuffer, 0, preRollHead)
            }
            preRollHead = 0
            preRollFilled = false
        }

        record.startRecording()

        recordingThread = thread(name = "VoiceKeyAudioRecordThread", priority = Thread.MAX_PRIORITY) {
            val chunk = ByteArray(1024)
            while (isRecording.get()) {
                val bytesRead = record.read(chunk, 0, chunk.size)
                if (bytesRead > 0) {
                    synchronized(lock) {
                        capturedAudio.write(chunk, 0, bytesRead)
                        for (i in 0 until bytesRead) {
                            preRollBuffer[preRollHead] = chunk[i]
                            preRollHead = (preRollHead + 1) % preRollCapacityBytes
                            if (preRollHead == 0) {
                                preRollFilled = true
                            }
                        }
                    }

                    // Calculate amplitude for UI animation
                    if (onAmplitude != null) {
                        var sumSquares = 0.0
                        var maxPeak = 0
                        val samples = bytesRead / 2
                        for (i in 0 until samples) {
                            val sample = (chunk[i * 2].toInt() and 0xFF) or (chunk[i * 2 + 1].toInt() shl 8)
                            val shortVal = sample.toShort()
                            val absVal = abs(shortVal.toInt())
                            if (absVal > maxPeak) maxPeak = absVal
                            sumSquares += shortVal * shortVal
                        }
                        val rms = sqrt(sumSquares / samples) / 32768.0
                        val peak = maxPeak / 32768.0f
                        val normalizedAmp = (rms * 3.0 + peak * 0.5).coerceIn(0.0, 1.0).toFloat()
                        onAmplitude(normalizedAmp)
                    }
                } else if (bytesRead < 0) {
                    Log.e(TAG, "AudioRecord error code: $bytesRead")
                    break
                }
            }
        }
    }

    /**
     * Stop recording and return the complete captured 16kHz 16-bit mono PCM bytes.
     */
    @Synchronized
    fun stop(): ByteArray {
        if (!isRecording.get()) {
            synchronized(lock) {
                return capturedAudio.toByteArray()
            }
        }

        isRecording.set(false)
        try {
            audioRecord?.stop()
        } catch (e: Exception) {
            Log.e(TAG, "Error stopping AudioRecord", e)
        }

        try {
            recordingThread?.join(500)
        } catch (e: InterruptedException) {
            Thread.currentThread().interrupt()
        }

        audioRecord?.release()
        audioRecord = null
        recordingThread = null

        synchronized(lock) {
            return capturedAudio.toByteArray()
        }
    }

    /**
     * Whether audio capture is actively running.
     */
    fun isRecording(): Boolean = isRecording.get()
}
