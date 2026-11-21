package com.voicekey.app.settings

import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import android.provider.Settings
import android.text.Editable
import android.text.TextWatcher
import android.view.View
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.EditText
import android.widget.ProgressBar
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.lifecycleScope
import com.google.android.material.materialswitch.MaterialSwitch
import com.voicekey.app.R
import com.voicekey.app.model.ModelPreset
import com.voicekey.app.model.VoiceKeyModelManager
import kotlinx.coroutines.launch

/**
 * Settings & Model Manager Activity for VoiceKey Android.
 * Provides on-device model downloading with live progress and SHA-256 verification,
 * vocabulary prompting, haptic toggles, and system IME activation.
 */
class SettingsActivity : AppCompatActivity() {

    private lateinit var preferences: VoiceKeyPreferences
    private lateinit var modelManager: VoiceKeyModelManager

    // Views
    private lateinit var txtTinyStatus: TextView
    private lateinit var btnActionTiny: Button
    private lateinit var progressTiny: ProgressBar

    private lateinit var txtBaseStatus: TextView
    private lateinit var btnActionBase: Button
    private lateinit var progressBase: ProgressBar

    private lateinit var editCustomVocabulary: EditText
    private lateinit var switchHaptic: MaterialSwitch
    private lateinit var switchSpokenPunctuation: MaterialSwitch
    private lateinit var switchAutoNormalize: MaterialSwitch

    private lateinit var btnEnableIme: Button
    private lateinit var btnSelectIme: Button

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_settings)

        preferences = VoiceKeyPreferences(this)
        modelManager = VoiceKeyModelManager(this)

        bindViews()
        setupListeners()
        refreshModelStatuses()
        loadPreferences()
    }

    private fun bindViews() {
        txtTinyStatus = findViewById(R.id.txt_tiny_status)
        btnActionTiny = findViewById(R.id.btn_action_tiny)
        progressTiny = findViewById(R.id.progress_tiny)

        txtBaseStatus = findViewById(R.id.txt_base_status)
        btnActionBase = findViewById(R.id.btn_action_base)
        progressBase = findViewById(R.id.progress_base)

        editCustomVocabulary = findViewById(R.id.edit_custom_vocabulary)
        switchHaptic = findViewById(R.id.switch_haptic)
        switchSpokenPunctuation = findViewById(R.id.switch_spoken_punctuation)
        switchAutoNormalize = findViewById(R.id.switch_auto_normalize)

        btnEnableIme = findViewById(R.id.btn_enable_ime)
        btnSelectIme = findViewById(R.id.btn_select_ime)
    }

    private fun setupListeners() {
        btnEnableIme.setOnClickListener {
            startActivity(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS))
        }

        btnSelectIme.setOnClickListener {
            val imm = getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager
            imm?.showInputMethodPicker()
        }

        val tinyPreset = VoiceKeyModelManager.PRESETS.first { it.id == "tiny.en" }
        btnActionTiny.setOnClickListener {
            handleModelAction(tinyPreset, btnActionTiny, progressTiny, txtTinyStatus)
        }

        val basePreset = VoiceKeyModelManager.PRESETS.first { it.id == "base.en" }
        btnActionBase.setOnClickListener {
            handleModelAction(basePreset, btnActionBase, progressBase, txtBaseStatus)
        }

        editCustomVocabulary.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                preferences.customVocabulary = s?.toString() ?: ""
            }
            override fun afterTextChanged(s: Editable?) {}
        })

        switchHaptic.setOnCheckedChangeListener { _, isChecked ->
            preferences.hapticFeedbackEnabled = isChecked
        }

        switchSpokenPunctuation.setOnCheckedChangeListener { _, isChecked ->
            preferences.spokenPunctuationEnabled = isChecked
        }

        switchAutoNormalize.setOnCheckedChangeListener { _, isChecked ->
            preferences.autoNormalizeEnabled = isChecked
        }
    }

    private fun loadPreferences() {
        editCustomVocabulary.setText(preferences.customVocabulary)
        switchHaptic.isChecked = preferences.hapticFeedbackEnabled
        switchSpokenPunctuation.isChecked = preferences.spokenPunctuationEnabled
        switchAutoNormalize.isChecked = preferences.autoNormalizeEnabled
    }

    private fun refreshModelStatuses() {
        val tinyPreset = VoiceKeyModelManager.PRESETS.first { it.id == "tiny.en" }
        updateCardUi(tinyPreset, btnActionTiny, progressTiny, txtTinyStatus)

        val basePreset = VoiceKeyModelManager.PRESETS.first { it.id == "base.en" }
        updateCardUi(basePreset, btnActionBase, progressBase, txtBaseStatus)
    }

    private fun updateCardUi(
        preset: ModelPreset,
        actionButton: Button,
        progressBar: ProgressBar,
        statusText: TextView
    ) {
        val isDownloaded = modelManager.isModelDownloaded(preset)
        val isActive = preferences.selectedModelId == preset.id && isDownloaded

        progressBar.visibility = View.GONE
        actionButton.isEnabled = true

        if (isActive) {
            statusText.text = "Active"
            statusText.setTextColor(Color.parseColor("#4ADE80")) // Green
            actionButton.text = "Selected"
            actionButton.isEnabled = false
        } else if (isDownloaded) {
            statusText.text = "Downloaded"
            statusText.setTextColor(Color.parseColor("#A1A1AA"))
            actionButton.text = "Activate"
            actionButton.isEnabled = true
        } else {
            statusText.text = "Not Downloaded"
            statusText.setTextColor(Color.parseColor("#71717A"))
            actionButton.text = "Download"
            actionButton.isEnabled = true
        }
    }

    private fun handleModelAction(
        preset: ModelPreset,
        actionButton: Button,
        progressBar: ProgressBar,
        statusText: TextView
    ) {
        val isDownloaded = modelManager.isModelDownloaded(preset)
        if (isDownloaded) {
            // Activate model
            preferences.selectedModelId = preset.id
            preferences.selectedModelPath = modelManager.getModelFile(preset).absolutePath
            refreshModelStatuses()
            Toast.makeText(this, "${preset.name} selected as active model", Toast.LENGTH_SHORT).show()
        } else {
            // Trigger download
            actionButton.isEnabled = false
            progressBar.visibility = View.VISIBLE
            progressBar.progress = 0
            statusText.text = "Downloading..."
            statusText.setTextColor(Color.parseColor("#818CF8"))

            lifecycleScope.launch {
                val result = modelManager.downloadModel(preset) { downloadedBytes, totalBytes, pct ->
                    runOnUiThread {
                        progressBar.progress = pct.toInt()
                        statusText.text = "Downloading (${pct.toInt()}%)"
                    }
                }

                if (result.isSuccess) {
                    preferences.selectedModelId = preset.id
                    preferences.selectedModelPath = result.getOrNull()?.absolutePath
                    Toast.makeText(this@SettingsActivity, "Download complete & verified!", Toast.LENGTH_SHORT).show()
                } else {
                    val err = result.exceptionOrNull()?.message ?: "Unknown error"
                    Toast.makeText(this@SettingsActivity, "Download failed: $err", Toast.LENGTH_LONG).show()
                }
                refreshModelStatuses()
            }
        }
    }
}
