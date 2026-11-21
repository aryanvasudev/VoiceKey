package com.voicekey.app.settings

import android.content.Context
import android.content.SharedPreferences

/**
 * Manages persistent user preferences and configurations for VoiceKey Android IME.
 */
class VoiceKeyPreferences(context: Context) {

    private val prefs: SharedPreferences =
        context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    var selectedModelId: String
        get() = prefs.getString(KEY_SELECTED_MODEL_ID, DEFAULT_MODEL_ID) ?: DEFAULT_MODEL_ID
        set(value) = prefs.edit().putString(KEY_SELECTED_MODEL_ID, value).apply()

    var selectedModelPath: String?
        get() = prefs.getString(KEY_SELECTED_MODEL_PATH, null)
        set(value) = prefs.edit().putString(KEY_SELECTED_MODEL_PATH, value).apply()

    var customVocabulary: String
        get() = prefs.getString(KEY_CUSTOM_VOCABULARY, "") ?: ""
        set(value) = prefs.edit().putString(KEY_CUSTOM_VOCABULARY, value).apply()

    var hapticFeedbackEnabled: Boolean
        get() = prefs.getBoolean(KEY_HAPTIC_FEEDBACK, true)
        set(value) = prefs.edit().putBoolean(KEY_HAPTIC_FEEDBACK, value).apply()

    var spokenPunctuationEnabled: Boolean
        get() = prefs.getBoolean(KEY_SPOKEN_PUNCTUATION, true)
        set(value) = prefs.edit().putBoolean(KEY_SPOKEN_PUNCTUATION, value).apply()

    var autoNormalizeEnabled: Boolean
        get() = prefs.getBoolean(KEY_AUTO_NORMALIZE, true)
        set(value) = prefs.edit().putBoolean(KEY_AUTO_NORMALIZE, value).apply()

    var threadsCount: Int
        get() = prefs.getInt(KEY_THREADS_COUNT, 4)
        set(value) = prefs.edit().putInt(KEY_THREADS_COUNT, value.coerceIn(1, 8)).apply()

    var isFirstLaunch: Boolean
        get() = prefs.getBoolean(KEY_IS_FIRST_LAUNCH, true)
        set(value) = prefs.edit().putBoolean(KEY_IS_FIRST_LAUNCH, value).apply()

    companion object {
        private const val PREFS_NAME = "voicekey_preferences"
        private const val DEFAULT_MODEL_ID = "tiny.en"

        const val KEY_SELECTED_MODEL_ID = "selected_model_id"
        const val KEY_SELECTED_MODEL_PATH = "selected_model_path"
        const val KEY_CUSTOM_VOCABULARY = "custom_vocabulary"
        const val KEY_HAPTIC_FEEDBACK = "haptic_feedback"
        const val KEY_SPOKEN_PUNCTUATION = "spoken_punctuation"
        const val KEY_AUTO_NORMALIZE = "auto_normalize"
        const val KEY_THREADS_COUNT = "threads_count"
        const val KEY_IS_FIRST_LAUNCH = "is_first_launch"
    }
}
