package com.voicekey.app.ime

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.util.Log
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.ImageButton
import android.widget.TextView
import androidx.core.content.ContextCompat
import com.voicekey.app.R
import com.voicekey.app.audio.VoiceKeyAudioRecorder
import com.voicekey.app.model.VoiceKeyModelManager
import com.voicekey.app.service.VoiceKeyAudioRecordingService
import com.voicekey.app.settings.SettingsActivity
import com.voicekey.app.settings.VoiceKeyPreferences
import com.voicekey.app.ui.WaveformView
import com.voicekey.core.VoiceKeyCore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

/**
 * VoiceKey Input Method Service.
 * Provides on-device voice dictation keyboard with real-time waveform visualization,
 * AudioRecord streaming, and direct InputConnection text insertion.
 */
class VoiceKeyInputMethodService : InputMethodService() {

    private val TAG = "VoiceKeyIME"

    private val serviceJob = SupervisorJob()
    private val serviceScope = CoroutineScope(Dispatchers.Main + serviceJob)
    private val mainHandler = Handler(Looper.getMainLooper())

    private val audioRecorder = VoiceKeyAudioRecorder()
    private lateinit var preferences: VoiceKeyPreferences
    private lateinit var modelManager: VoiceKeyModelManager
    private var voiceKeyCore: VoiceKeyCore? = null
    private var loadedModelPath: String? = null

    // UI View references
    private var rootView: View? = null
    private var statusTextView: TextView? = null
    private var waveformView: WaveformView? = null
    private var micButton: ImageButton? = null
    private var switchKeyButtonTop: ImageButton? = null
    private var switchKeyButtonBottom: ImageButton? = null
    private var spaceButton: Button? = null
    private var backspaceButton: ImageButton? = null
    private var enterButton: ImageButton? = null

    override fun onCreate() {
        super.onCreate()
        preferences = VoiceKeyPreferences(this)
        modelManager = VoiceKeyModelManager(this)
        loadVoiceKeyEngine()
    }

    override fun onCreateInputView(): View {
        val view = layoutInflater.inflate(R.layout.voicekey_keyboard_view, null)
        rootView = view

        statusTextView = view.findViewById(R.id.status_text)
        waveformView = view.findViewById(R.id.waveform_view)
        micButton = view.findViewById(R.id.btn_mic)
        switchKeyButtonTop = view.findViewById(R.id.btn_switch_keyboard_top)
        switchKeyButtonBottom = view.findViewById(R.id.btn_switch_keyboard)
        spaceButton = view.findViewById(R.id.btn_space)
        backspaceButton = view.findViewById(R.id.btn_backspace)
        enterButton = view.findViewById(R.id.btn_enter)

        setupListeners()
        return view
    }

    override fun onStartInputView(info: EditorInfo?, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        resetUiState()
        loadVoiceKeyEngine()
    }

    override fun onFinishInputView(finishingInput: Boolean) {
        if (audioRecorder.isRecording()) {
            stopAudioRecording(shouldTranscribe = false)
        }
        super.onFinishInputView(finishingInput)
    }

    override fun onDestroy() {
        if (audioRecorder.isRecording()) {
            stopAudioRecording(shouldTranscribe = false)
        }
        serviceJob.cancel()
        super.onDestroy()
    }

    private fun setupListeners() {
        micButton?.setOnClickListener {
            vibrateFeedback()
            if (audioRecorder.isRecording()) {
                stopAudioRecording(shouldTranscribe = true)
            } else {
                startAudioRecording()
            }
        }

        val switchListener = View.OnClickListener {
            vibrateFeedback()
            switchKeyboard()
        }
        switchKeyButtonTop?.setOnClickListener(switchListener)
        switchKeyButtonBottom?.setOnClickListener(switchListener)

        spaceButton?.setOnClickListener {
            vibrateFeedback()
            currentInputConnection?.commitText(" ", 1)
        }

        backspaceButton?.setOnClickListener {
            vibrateFeedback()
            handleBackspace()
        }

        enterButton?.setOnClickListener {
            vibrateFeedback()
            handleEnter()
        }
    }

    private fun startAudioRecording() {
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO)
            != PackageManager.PERMISSION_GRANTED
        ) {
            statusTextView?.text = getString(R.string.recording_notification_title) + ": Permission Required"
            Log.w(TAG, "RECORD_AUDIO permission not granted.")
            return
        }

        try {
            VoiceKeyAudioRecordingService.start(this)

            waveformView?.setRecording(true)
            micButton?.setColorFilter(Color.parseColor("#EF4444")) // Crimson recording indicator
            statusTextView?.text = getString(R.string.status_recording)

            audioRecorder.start { normalizedAmp ->
                mainHandler.post {
                    waveformView?.addAmplitude(normalizedAmp)
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to start audio recording", e)
            statusTextView?.text = "Error starting recording"
            resetUiState()
        }
    }

    private fun stopAudioRecording(shouldTranscribe: Boolean) {
        waveformView?.setRecording(false)
        micButton?.clearColorFilter()
        VoiceKeyAudioRecordingService.stop(this)

        val pcmBytes = try {
            audioRecorder.stop()
        } catch (e: Exception) {
            Log.e(TAG, "Error stopping recorder", e)
            ByteArray(0)
        }

        if (!shouldTranscribe || pcmBytes.isEmpty()) {
            resetUiState()
            return
        }

        statusTextView?.text = getString(R.string.status_transcribing)

        serviceScope.launch {
            try {
                val transcription = withContext(Dispatchers.Default) {
                    processAudioTranscription(pcmBytes)
                }

                if (!transcription.isNullOrBlank()) {
                    currentInputConnection?.commitText(transcription, 1)
                    statusTextView?.text = getString(R.string.status_done)
                } else {
                    statusTextView?.text = getString(R.string.status_idle)
                }
            } catch (e: Exception) {
                Log.e(TAG, "Transcription failed", e)
                statusTextView?.text = "Transcription failed"
            } finally {
                mainHandler.postDelayed({
                    if (!audioRecorder.isRecording()) {
                        resetUiState()
                    }
                }, 1500)
            }
        }
    }

    private fun processAudioTranscription(pcmBytes: ByteArray): String? {
        val core = voiceKeyCore ?: run {
            loadVoiceKeyEngine()
            voiceKeyCore
        }

        val prompt = preferences.customVocabulary.ifBlank { null }
        return if (core != null) {
            try {
                val result = core.transcribePcm16(
                    pcmBytes = pcmBytes,
                    sampleRate = 16000,
                    channels = 1,
                    initialPrompt = prompt
                )
                result.text.trim()
            } catch (e: Exception) {
                Log.e(TAG, "Core transcribe failed: ${e.message}", e)
                null
            }
        } else {
            Log.w(TAG, "No VoiceKeyCore engine or model file loaded.")
            null
        }
    }

    private fun loadVoiceKeyEngine() {
        // Check active model from preferences / modelManager first
        val activeModel = modelManager.getActiveModelFile()
        val targetPath = activeModel?.takeIf { it.exists() && it.canRead() }?.absolutePath

        if (voiceKeyCore != null && targetPath != null && loadedModelPath == targetPath) {
            return
        }

        if (activeModel != null && activeModel.exists() && activeModel.canRead()) {
            try {
                Log.i(TAG, "Initializing VoiceKeyCore with active model at ${activeModel.absolutePath}")
                voiceKeyCore = VoiceKeyCore.create(
                    modelPath = activeModel.absolutePath,
                    threads = preferences.threadsCount,
                    language = "en",
                    enableNormalization = preferences.autoNormalizeEnabled,
                    enableSpokenPunctuation = preferences.spokenPunctuationEnabled
                )
                loadedModelPath = activeModel.absolutePath
                return
            } catch (e: Exception) {
                Log.e(TAG, "Failed initializing VoiceKeyCore with active model ${activeModel.absolutePath}", e)
            }
        }

        // Look for model files in standard fallback application directories
        val possibleModelDirs = listOfNotNull(
            getExternalFilesDir(null),
            filesDir,
            File("/data/local/tmp")
        )

        val modelNames = listOf(
            "ggml-tiny.en.bin",
            "ggml-tiny.bin",
            "ggml-base.en.bin",
            "ggml-base.bin",
            "model.bin"
        )

        for (dir in possibleModelDirs) {
            for (name in modelNames) {
                val candidate = File(dir, name)
                if (candidate.exists() && candidate.canRead() && candidate.length() > 0) {
                    if (voiceKeyCore != null && loadedModelPath == candidate.absolutePath) {
                        return
                    }
                    try {
                        Log.i(TAG, "Initializing VoiceKeyCore with model at ${candidate.absolutePath}")
                        voiceKeyCore = VoiceKeyCore.create(
                            modelPath = candidate.absolutePath,
                            threads = preferences.threadsCount,
                            language = "en",
                            enableNormalization = preferences.autoNormalizeEnabled,
                            enableSpokenPunctuation = preferences.spokenPunctuationEnabled
                        )
                        loadedModelPath = candidate.absolutePath
                        return
                    } catch (e: Exception) {
                        Log.e(TAG, "Failed initializing VoiceKeyCore with ${candidate.absolutePath}", e)
                    }
                }
            }
        }
    }

    private fun handleBackspace() {
        val ic = currentInputConnection ?: return
        val selectedText = ic.getSelectedText(0)
        if (selectedText.isNullOrEmpty()) {
            ic.deleteSurroundingText(1, 0)
        } else {
            ic.commitText("", 1)
        }
    }

    private fun handleEnter() {
        val ic = currentInputConnection ?: return
        val info = currentInputEditorInfo
        if (info != null && (info.imeOptions and EditorInfo.IME_MASK_ACTION) != EditorInfo.IME_ACTION_NONE &&
            (info.imeOptions and EditorInfo.IME_FLAG_NO_ENTER_ACTION) == 0
        ) {
            ic.performEditorAction(info.imeOptions and EditorInfo.IME_MASK_ACTION)
        } else {
            ic.sendKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_ENTER))
            ic.sendKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_ENTER))
        }
    }

    private fun switchKeyboard() {
        val imm = getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager ?: return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            try {
                switchToPreviousInputMethod()
            } catch (e: Exception) {
                imm.showInputMethodPicker()
            }
        } else {
            imm.showInputMethodPicker()
        }
    }

    private fun resetUiState() {
        if (voiceKeyCore == null && modelManager.getActiveModelFile() == null) {
            statusTextView?.text = "No model found. Tap to setup"
            statusTextView?.setOnClickListener {
                val intent = Intent(this, SettingsActivity::class.java).apply {
                    flags = Intent.FLAG_ACTIVITY_NEW_TASK
                }
                startActivity(intent)
            }
        } else {
            statusTextView?.text = getString(R.string.status_idle)
            statusTextView?.setOnClickListener(null)
        }
        waveformView?.setRecording(false)
        micButton?.clearColorFilter()
    }

    @SuppressLint("MissingPermission")
    private fun vibrateFeedback() {
        if (!preferences.hapticFeedbackEnabled) return

        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                val vibratorManager = getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as? VibratorManager
                vibratorManager?.defaultVibrator?.vibrate(
                    VibrationEffect.createPredefined(VibrationEffect.EFFECT_CLICK)
                )
            } else {
                @Suppress("DEPRECATION")
                val vibrator = getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    vibrator?.vibrate(VibrationEffect.createOneShot(20, VibrationEffect.DEFAULT_AMPLITUDE))
                } else {
                    @Suppress("DEPRECATION")
                    vibrator?.vibrate(20)
                }
            }
        } catch (e: Exception) {
            Log.d(TAG, "Vibration feedback not available", e)
        }
    }
}
