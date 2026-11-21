package com.voicekey.app.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.util.AttributeSet
import android.view.View

/**
 * Animated real-time waveform visualizer displaying audio frequency/amplitude bars
 * during active voice dictation.
 */
class WaveformView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : View(context, attrs, defStyleAttr) {

    private val barCount = 32
    private val amplitudes = FloatArray(barCount) { 0.05f }
    private var isRecording = false

    private val activeBarPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#818CF8") // Indigo-400
        strokeCap = Paint.Cap.ROUND
    }

    private val idleBarPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#3F3F46") // Zinc-700
        strokeCap = Paint.Cap.ROUND
    }

    fun setRecording(recording: Boolean) {
        this.isRecording = recording
        if (!recording) {
            // Decay back to resting state
            for (i in amplitudes.indices) {
                amplitudes[i] = 0.05f
            }
        }
        postInvalidateOnAnimation()
    }

    /**
     * Feed real-time normalized amplitude (0.0 to 1.0) from audio capture.
     */
    fun addAmplitude(amp: Float) {
        val clamped = amp.coerceIn(0.02f, 1.0f)
        // Shift bars left
        System.arraycopy(amplitudes, 1, amplitudes, 0, barCount - 1)
        amplitudes[barCount - 1] = clamped
        postInvalidateOnAnimation()
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)

        val w = width.toFloat()
        val h = height.toFloat()
        if (w <= 0f || h <= 0f) return

        val centerY = h / 2f
        val totalSpacingRatio = 0.4f
        val slotWidth = w / barCount
        val barWidth = slotWidth * (1f - totalSpacingRatio)
        val maxBarHeight = (h - 12f).coerceAtLeast(4f)

        activeBarPaint.strokeWidth = barWidth
        idleBarPaint.strokeWidth = barWidth

        val paint = if (isRecording) activeBarPaint else idleBarPaint

        for (i in 0 until barCount) {
            val x = i * slotWidth + slotWidth / 2f
            val amp = amplitudes[i]
            val barHeight = (maxBarHeight * amp).coerceAtLeast(4f)
            val top = centerY - barHeight / 2f
            val bottom = centerY + barHeight / 2f

            canvas.drawLine(x, top, x, bottom, paint)
        }
    }
}
