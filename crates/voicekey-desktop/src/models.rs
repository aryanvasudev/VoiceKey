//! GGML Whisper Model Manager: background downloads, SHA-256 verification, and hardware recommendations.

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tracing::{error, info};

/// Specification of an official GGML Whisper model preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPreset {
    pub id: String,
    pub name: String,
    pub filename: String,
    pub url: String,
    pub expected_sha256: String,
    pub size_bytes: u64,
    pub description: String,
}

/// Information returned to the UI about a model's local status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfoDto {
    pub id: String,
    pub name: String,
    pub filename: String,
    pub is_downloaded: bool,
    pub size_mb: f64,
    pub description: String,
    pub is_active: bool,
    pub is_recommended: bool,
}

/// Recommended model based on CPU profiling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedModelDto {
    pub model_id: String,
    pub reason: String,
    pub cpu_threads: usize,
    pub has_avx2: bool,
}

/// Progress event emitted over Tauri event bus during model download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadProgress {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f32,
    pub status: String,
}

/// Official verified GGML Whisper models from HuggingFace / ggerganov.
pub fn get_available_presets() -> Vec<ModelPreset> {
    vec![
        ModelPreset {
            id: "tiny.en".into(),
            name: "Whisper Tiny (English)".into(),
            filename: "ggml-tiny.en.bin".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin"
                .into(),
            expected_sha256: "bd577a113a864445d4c299885e83797195274dbed8d2426916ac333e6c0a0636"
                .into(),
            size_bytes: 77_691_728,
            description: "Fastest inference (~50ms) with minimal memory footprint (< 100MB)".into(),
        },
        ModelPreset {
            id: "base.en".into(),
            name: "Whisper Base (English)".into(),
            filename: "ggml-base.en.bin".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin"
                .into(),
            expected_sha256: "465707469f7c0208fc1e1bd187bb6144eb5a9bf87d7f22bde5774022f641dcdd"
                .into(),
            size_bytes: 147_951_408,
            description: "Default balanced model (~120ms) with high accuracy for daily dictation"
                .into(),
        },
        ModelPreset {
            id: "small.en".into(),
            name: "Whisper Small (English)".into(),
            filename: "ggml-small.en.bin".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin"
                .into(),
            expected_sha256: "55356645c2b361a969dfd0ef2c5a50d530abb21a16f4cad4736f9bcb800cc441"
                .into(),
            size_bytes: 487_602_688,
            description:
                "Highest accuracy (~400ms) for complex coding terms and specialized jargon".into(),
        },
    ]
}

/// Target directory for persistent Whisper models.
pub fn get_models_dir() -> PathBuf {
    if let Some(data_dir) = dirs::data_dir() {
        return data_dir.join("voicekey").join("models");
    }
    PathBuf::from("/tmp/voicekey/models")
}

/// Check if a model file exists on disk and return its size in MB.
pub fn check_model_on_disk(filename: &str) -> Option<(PathBuf, f64)> {
    let mut search_dirs = vec![get_models_dir()];
    if let Some(c) = dirs::cache_dir() {
        search_dirs.push(c.join("voicekey").join("models"));
    }
    search_dirs.push(PathBuf::from("/tmp/voicekey/models"));

    for dir in search_dirs {
        let p = dir.join(filename);
        if p.exists() {
            if let Ok(meta) = fs::metadata(&p) {
                let size_mb = meta.len() as f64 / 1_048_576.0;
                return Some((p, size_mb));
            }
        }
    }
    None
}

/// Hardware-based model recommender profiling CPU instruction sets and threads.
pub fn recommend_model() -> RecommendedModelDto {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    #[cfg(target_arch = "x86_64")]
    let has_avx2 = is_x86_feature_detected!("avx2");
    #[cfg(not(target_arch = "x86_64"))]
    let has_avx2 = false;

    let (model_id, reason) = if threads >= 8 && has_avx2 {
        ("base.en".into(), "High-performance CPU with AVX2 acceleration detected. base.en provides instant, highly accurate dictation.".into())
    } else if threads >= 4 {
        (
            "base.en".into(),
            "Multi-core CPU detected. base.en provides an optimal speed/accuracy balance.".into(),
        )
    } else {
        ("tiny.en".into(), "Constrained CPU detected. tiny.en provides lightweight, ultra-low latency transcription.".into())
    };

    RecommendedModelDto {
        model_id,
        reason,
        cpu_threads: threads,
        has_avx2,
    }
}

/// Download a model with live progress updates and SHA-256 integrity validation.
pub async fn download_model_with_progress(
    app: AppHandle,
    model_id: String,
) -> Result<PathBuf, String> {
    let presets = get_available_presets();
    let preset = presets
        .into_iter()
        .find(|p| p.id == model_id)
        .ok_or_else(|| format!("Unknown model preset '{}'", model_id))?;

    let dest_dir = get_models_dir();
    fs::create_dir_all(&dest_dir).map_err(|e| {
        format!(
            "Failed to create models directory {}: {e}",
            dest_dir.display()
        )
    })?;

    let final_path = dest_dir.join(&preset.filename);
    let temp_path = dest_dir.join(format!("{}.download", preset.filename));

    info!("Starting download of {} from {}", preset.name, preset.url);

    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let response = client
        .get(&preset.url)
        .send()
        .await
        .map_err(|e| format!("HTTP request to {} failed: {e}", preset.url))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download server returned HTTP {}",
            response.status()
        ));
    }

    let total_size = response.content_length().unwrap_or(preset.size_bytes);

    let mut file = File::create(&temp_path).map_err(|e| {
        format!(
            "Failed to create temporary file {}: {e}",
            temp_path.display()
        )
    })?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut hasher = Sha256::new();

    let _ = app.emit(
        "model_download_progress",
        ModelDownloadProgress {
            model_id: preset.id.clone(),
            downloaded_bytes: 0,
            total_bytes: total_size,
            percentage: 0.0,
            status: "downloading".into(),
        },
    );

    while let Some(chunk_res) = stream.next().await {
        let chunk = match chunk_res {
            Ok(c) => c,
            Err(e) => {
                let _ = fs::remove_file(&temp_path);
                let _ = app.emit(
                    "model_download_progress",
                    ModelDownloadProgress {
                        model_id: preset.id.clone(),
                        downloaded_bytes: downloaded,
                        total_bytes: total_size,
                        percentage: 0.0,
                        status: "failed".into(),
                    },
                );
                return Err(format!("Error receiving network stream: {e}"));
            }
        };
        if let Err(e) = file.write_all(&chunk) {
            let _ = fs::remove_file(&temp_path);
            let _ = app.emit(
                "model_download_progress",
                ModelDownloadProgress {
                    model_id: preset.id.clone(),
                    downloaded_bytes: downloaded,
                    total_bytes: total_size,
                    percentage: 0.0,
                    status: "failed".into(),
                },
            );
            return Err(format!("Error writing model chunk to disk: {e}"));
        }

        hasher.update(&chunk);
        downloaded += chunk.len() as u64;

        let percentage = (downloaded as f32 / total_size as f32) * 100.0;
        let _ = app.emit(
            "model_download_progress",
            ModelDownloadProgress {
                model_id: preset.id.clone(),
                downloaded_bytes: downloaded,
                total_bytes: total_size,
                percentage,
                status: "downloading".into(),
            },
        );
    }

    if let Err(e) = file.flush() {
        let _ = fs::remove_file(&temp_path);
        let _ = app.emit(
            "model_download_progress",
            ModelDownloadProgress {
                model_id: preset.id.clone(),
                downloaded_bytes: downloaded,
                total_bytes: total_size,
                percentage: 0.0,
                status: "failed".into(),
            },
        );
        return Err(format!("Failed to flush file to disk: {e}"));
    }
    drop(file);

    // Verify SHA-256 Checksum
    let computed_hash = format!("{:x}", hasher.finalize());
    info!(
        "Download complete. Verifying SHA-256:\nExpected: {}\nComputed: {}",
        preset.expected_sha256, computed_hash
    );

    let _ = app.emit(
        "model_download_progress",
        ModelDownloadProgress {
            model_id: preset.id.clone(),
            downloaded_bytes: downloaded,
            total_bytes: total_size,
            percentage: 100.0,
            status: "verifying".into(),
        },
    );

    if computed_hash.to_lowercase() != preset.expected_sha256.to_lowercase() {
        let _ = fs::remove_file(&temp_path);
        let err_msg = format!(
            "SHA-256 verification failed for {}: expected {}, computed {}",
            preset.name, preset.expected_sha256, computed_hash
        );
        error!("{}", err_msg);
        let _ = app.emit(
            "model_download_progress",
            ModelDownloadProgress {
                model_id: preset.id.clone(),
                downloaded_bytes: downloaded,
                total_bytes: total_size,
                percentage: 100.0,
                status: "failed".into(),
            },
        );
        return Err(err_msg);
    }

    // Move validated file into destination path
    if final_path.exists() {
        let _ = fs::remove_file(&final_path);
    }
    fs::rename(&temp_path, &final_path).map_err(|e| {
        format!(
            "Failed to rename temporary file to {}: {e}",
            final_path.display()
        )
    })?;

    info!(
        "Model {} verified and ready at {}",
        preset.name,
        final_path.display()
    );

    let _ = app.emit(
        "model_download_progress",
        ModelDownloadProgress {
            model_id: preset.id.clone(),
            downloaded_bytes: total_size,
            total_bytes: total_size,
            percentage: 100.0,
            status: "complete".into(),
        },
    );

    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presets_exist_and_valid() {
        let presets = get_available_presets();
        assert_eq!(presets.len(), 3);
        assert!(presets.iter().any(|p| p.id == "tiny.en"));
        assert!(presets.iter().any(|p| p.id == "base.en"));
        assert!(presets.iter().any(|p| p.id == "small.en"));

        for p in presets {
            assert!(!p.url.is_empty());
            assert_eq!(p.expected_sha256.len(), 64);
            assert!(p.size_bytes > 0);
        }
    }

    #[test]
    fn test_recommend_model_returns_sensible_choice() {
        let rec = recommend_model();
        assert!(
            rec.model_id == "tiny.en" || rec.model_id == "base.en" || rec.model_id == "small.en"
        );
        assert!(rec.cpu_threads > 0);
    }
}
