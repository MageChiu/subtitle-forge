// ============================================================
// asr/models.rs — Model management (download, list, verify)
// ============================================================

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Available Whisper model sizes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum WhisperModelSize {
    Tiny,
    Base,
    Small,
    Medium,
    Large,
}

impl WhisperModelSize {
    pub fn filename(&self) -> &str {
        match self {
            Self::Tiny => "ggml-tiny.bin",
            Self::Base => "ggml-base.bin",
            Self::Small => "ggml-small.bin",
            Self::Medium => "ggml-medium.bin",
            Self::Large => "ggml-large-v3.bin",
        }
    }

    pub fn size_mb(&self) -> u64 {
        match self {
            Self::Tiny => 75,
            Self::Base => 142,
            Self::Small => 466,
            Self::Medium => 1500,
            Self::Large => 3100,
        }
    }

    pub fn download_url(&self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.filename()
        )
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Tiny => "Tiny (75 MB) — Fastest, lower accuracy",
            Self::Base => "Base (142 MB) — Good balance",
            Self::Small => "Small (466 MB) — Better accuracy",
            Self::Medium => "Medium (1.5 GB) — High accuracy",
            Self::Large => "Large V3 (3.1 GB) — Best accuracy",
        }
    }
}

/// Model info for frontend display
#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: WhisperModelSize,
    pub size_mb: u64,
    pub path: String,
    pub downloaded: bool,
}

/// Model manager
pub struct ModelManager {
    models_dir: PathBuf,
}

impl ModelManager {
    pub fn new(models_dir: PathBuf) -> Self {
        Self { models_dir }
    }

    /// List all available models (downloaded + available for download)
    pub fn list_models(&self) -> Vec<ModelInfo> {
        let sizes = [
            WhisperModelSize::Tiny,
            WhisperModelSize::Base,
            WhisperModelSize::Small,
            WhisperModelSize::Medium,
            WhisperModelSize::Large,
        ];

        sizes
            .iter()
            .map(|size| {
                let path = self.models_dir.join("whisper").join(size.filename());
                ModelInfo {
                    name: format!("Whisper {}", format!("{:?}", size)),
                    size: *size,
                    size_mb: size.size_mb(),
                    path: path.to_string_lossy().to_string(),
                    downloaded: path.exists(),
                }
            })
            .collect()
    }

    /// Get path for a specific model
    pub fn model_path(&self, size: WhisperModelSize) -> PathBuf {
        self.models_dir.join("whisper").join(size.filename())
    }

    /// Check if a model is downloaded
    pub fn is_downloaded(&self, size: WhisperModelSize) -> bool {
        self.model_path(size).exists()
    }

    /// Download a model (with progress callback)
    pub async fn download_model(
        &self,
        size: WhisperModelSize,
        progress_callback: impl Fn(f64) + Send + 'static,
    ) -> Result<PathBuf, String> {
        let url = size.download_url();
        let dest = self.model_path(size);

        // Ensure directory exists
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        tracing::info!("Downloading model from {} to {:?}", url, dest);

        // Implementation:
        //
        // let client = reqwest::Client::new();
        // let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
        // let total = resp.content_length().unwrap_or(0);
        // let mut downloaded: u64 = 0;
        // let mut file = tokio::fs::File::create(&dest).await.map_err(|e| e.to_string())?;
        // let mut stream = resp.bytes_stream();
        //
        // while let Some(chunk) = stream.next().await {
        //     let chunk = chunk.map_err(|e| e.to_string())?;
        //     file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        //     downloaded += chunk.len() as u64;
        //     if total > 0 {
        //         progress_callback(downloaded as f64 / total as f64);
        //     }
        // }

        todo!("Implement download with reqwest streaming")
    }

    /// Verify model integrity (file size check)
    pub fn verify_model(&self, size: WhisperModelSize) -> bool {
        let path = self.model_path(size);
        if let Ok(metadata) = std::fs::metadata(&path) {
            // Simple size check (within 10% of expected)
            let expected = size.size_mb() * 1024 * 1024;
            let actual = metadata.len();
            actual > expected * 9 / 10
        } else {
            false
        }
    }
}
