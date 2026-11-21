package com.voicekey.app.model

import android.content.Context
import com.voicekey.app.settings.VoiceKeyPreferences
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest

/**
 * Metadata definition for a GGML Whisper quantized model preset.
 */
data class ModelPreset(
    val id: String,
    val name: String,
    val filename: String,
    val url: String,
    val expectedSha256: String,
    val sizeBytes: Long,
    val description: String
)

/**
 * On-device GGML Whisper Model Manager for Android.
 * Handles model discovery, background downloading, SHA-256 integrity validation,
 * and active model configuration.
 */
class VoiceKeyModelManager(private val context: Context) {

    private val preferences = VoiceKeyPreferences(context)

    companion object {
        val PRESETS = listOf(
            ModelPreset(
                id = "tiny.en",
                name = "Whisper Tiny (English)",
                filename = "ggml-tiny.en.bin",
                url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
                expectedSha256 = "bd577a113a864445d4c299885e83797195274dbed8d2426916ac333e6c0a0636",
                sizeBytes = 77_691_728L,
                description = "Fastest on-device inference (~50ms) with minimal battery & memory usage."
            ),
            ModelPreset(
                id = "base.en",
                name = "Whisper Base (English)",
                filename = "ggml-base.en.bin",
                url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
                expectedSha256 = "465707469f7c0208fc1e1bd187bb6144eb5a9bf87d7f22bde5774022f641dcdd",
                sizeBytes = 147_951_408L,
                description = "Recommended balanced model (~120ms) with high accuracy for daily dictation."
            ),
            ModelPreset(
                id = "small.en",
                name = "Whisper Small (English)",
                filename = "ggml-small.en.bin",
                url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin",
                expectedSha256 = "55356645c2b361a969dfd0ef2c5a50d530abb21a16f4cad4736f9bcb800cc441",
                sizeBytes = 487_602_688L,
                description = "Highest accuracy for complex programming syntax and domain jargon."
            )
        )

        /**
         * Calculate SHA-256 hex string of a file.
         */
        @JvmStatic
        fun calculateSha256(file: File): String {
            val digest = MessageDigest.getInstance("SHA-256")
            file.inputStream().use { input ->
                val buffer = ByteArray(8192)
                var bytesRead: Int
                while (input.read(buffer).also { bytesRead = it } != -1) {
                    digest.update(buffer, 0, bytesRead)
                }
            }
            return digest.digest().joinToString("") { "%02x".format(it) }
        }
    }

    /**
     * Get directory where models are stored in app private storage.
     */
    fun getModelsDir(): File {
        val dir = File(context.filesDir, "models")
        if (!dir.exists()) {
            dir.mkdirs()
        }
        return dir
    }

    /**
     * Get destination file on disk for a given preset.
     */
    fun getModelFile(preset: ModelPreset): File {
        return File(getModelsDir(), preset.filename)
    }

    /**
     * Check if a model is downloaded and non-empty.
     */
    fun isModelDownloaded(preset: ModelPreset): Boolean {
        val file = getModelFile(preset)
        return file.exists() && file.length() > 0
    }

    /**
     * Verify SHA-256 checksum of an existing model file.
     */
    fun verifyModelIntegrity(preset: ModelPreset): Boolean {
        val file = getModelFile(preset)
        if (!file.exists()) return false
        val hash = calculateSha256(file)
        return hash.equals(preset.expectedSha256, ignoreCase = true)
    }

    /**
     * Download a model asynchronously with live progress updates.
     */
    suspend fun downloadModel(
        preset: ModelPreset,
        onProgress: (downloadedBytes: Long, totalBytes: Long, percentage: Float) -> Unit
    ): Result<File> = withContext(Dispatchers.IO) {
        val targetFile = getModelFile(preset)
        val tempFile = File(getModelsDir(), "${preset.filename}.tmp")

        try {
            val url = URL(preset.url)
            val connection = url.openConnection() as HttpURLConnection
            connection.connectTimeout = 15000
            connection.readTimeout = 30000
            connection.instanceFollowRedirects = true
            connection.connect()

            val responseCode = connection.responseCode
            if (responseCode !in 200..299) {
                return@withContext Result.failure(
                    IllegalStateException("HTTP error $responseCode while downloading model")
                )
            }

            val totalBytes = if (connection.contentLengthLong > 0) {
                connection.contentLengthLong
            } else {
                preset.sizeBytes
            }

            var downloadedBytes = 0L
            val buffer = ByteArray(16384)

            connection.inputStream.use { input: InputStream ->
                FileOutputStream(tempFile).use { output: FileOutputStream ->
                    var bytesRead: Int
                    while (input.read(buffer).also { bytesRead = it } != -1) {
                        output.write(buffer, 0, bytesRead)
                        downloadedBytes += bytesRead
                        val percentage = if (totalBytes > 0) {
                            (downloadedBytes.toFloat() / totalBytes * 100f).coerceIn(0f, 100f)
                        } else {
                            0f
                        }
                        onProgress(downloadedBytes, totalBytes, percentage)
                    }
                    output.flush()
                }
            }

            // Verify integrity
            val actualHash = calculateSha256(tempFile)
            if (!actualHash.equals(preset.expectedSha256, ignoreCase = true)) {
                tempFile.delete()
                return@withContext Result.failure(
                    SecurityException("SHA-256 checksum mismatch for ${preset.filename}: expected ${preset.expectedSha256}, got $actualHash")
                )
            }

            // Atomically replace target
            if (targetFile.exists()) {
                targetFile.delete()
            }
            if (!tempFile.renameTo(targetFile)) {
                tempFile.copyTo(targetFile, overwrite = true)
                tempFile.delete()
            }

            // Update active model preference if this is the first or currently selected
            if (preferences.selectedModelPath.isNullOrBlank() || preferences.selectedModelId == preset.id) {
                preferences.selectedModelId = preset.id
                preferences.selectedModelPath = targetFile.absolutePath
            }

            Result.success(targetFile)
        } catch (e: Exception) {
            if (tempFile.exists()) tempFile.delete()
            Result.failure(e)
        }
    }

    /**
     * Delete a downloaded model from local storage.
     */
    fun deleteModel(preset: ModelPreset): Boolean {
        val file = getModelFile(preset)
        val success = if (file.exists()) file.delete() else true
        if (preferences.selectedModelId == preset.id) {
            preferences.selectedModelPath = null
            // Select fallback downloaded model if any
            val fallback = PRESETS.firstOrNull { isModelDownloaded(it) }
            if (fallback != null) {
                preferences.selectedModelId = fallback.id
                preferences.selectedModelPath = getModelFile(fallback).absolutePath
            }
        }
        return success
    }

    /**
     * Locate the active model file to be loaded by VoiceKeyEngine.
     */
    fun getActiveModelFile(): File? {
        val customPath = preferences.selectedModelPath
        if (!customPath.isNullOrBlank()) {
            val file = File(customPath)
            if (file.exists() && file.length() > 0) return file
        }

        // Check preferred presets
        val selectedPreset = PRESETS.firstOrNull { it.id == preferences.selectedModelId }
        if (selectedPreset != null && isModelDownloaded(selectedPreset)) {
            return getModelFile(selectedPreset)
        }

        // Fallback to any downloaded preset
        for (preset in PRESETS) {
            if (isModelDownloaded(preset)) {
                return getModelFile(preset)
            }
        }

        return null
    }
}
