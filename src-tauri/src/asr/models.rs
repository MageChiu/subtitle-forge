use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
            Self::Tiny => "Tiny (75 MB)",
            Self::Base => "Base (142 MB)",
            Self::Small => "Small (466 MB)",
            Self::Medium => "Medium (1.5 GB)",
            Self::Large => "Large V3 (3.1 GB)",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Tiny => "Fastest, lower accuracy",
            Self::Base => "Good balance of speed and accuracy",
            Self::Small => "Better accuracy, slower",
            Self::Medium => "High accuracy, requires more RAM",
            Self::Large => "Best accuracy, requires significant RAM & GPU",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "tiny" => Some(Self::Tiny),
            "base" => Some(Self::Base),
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
            _ => None,
        }
    }

    pub fn key(&self) -> &str {
        match self {
            Self::Tiny => "tiny",
            Self::Base => "base",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub key: String,
    pub name: String,
    pub size_mb: u64,
    pub description: String,
    pub path: String,
    pub downloaded: bool,
    pub download_url: String,
}

pub struct ModelManager {
    models_dir: PathBuf,
}

impl ModelManager {
    pub fn new(models_dir: PathBuf) -> Self {
        Self { models_dir }
    }

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
                    key: size.key().to_string(),
                    name: size.display_name().to_string(),
                    size_mb: size.size_mb(),
                    description: size.description().to_string(),
                    path: path.to_string_lossy().to_string(),
                    downloaded: path.exists(),
                    download_url: size.download_url(),
                }
            })
            .collect()
    }

    pub fn model_path(&self, size: WhisperModelSize) -> PathBuf {
        self.models_dir.join("whisper").join(size.filename())
    }

    pub fn is_downloaded(&self, size: WhisperModelSize) -> bool {
        self.model_path(size).exists()
    }

    pub fn check_model(&self, model_key: &str) -> Result<PathBuf, String> {
        let size = WhisperModelSize::from_key(model_key)
            .ok_or_else(|| format!("Unknown model: {}", model_key))?;
        let path = self.model_path(size);
        if path.exists() {
            Ok(path)
        } else {
            Err(format!(
                "Whisper model '{}' not found. Please download it in Settings > Models.\nPath: {:?}",
                size.filename(),
                path
            ))
        }
    }

    pub async fn download_model(
        &self,
        model_key: &str,
        progress_tx: tokio::sync::mpsc::Sender<DownloadProgress>,
    ) -> Result<String, String> {
        let size = WhisperModelSize::from_key(model_key)
            .ok_or_else(|| format!("Unknown model: {}", model_key))?;
        let url = size.download_url();
        let dest = self.model_path(size);

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        if dest.exists() {
            return Ok(format!("Model {} already downloaded", size.filename()));
        }

        tracing::info!("Downloading model from {} to {:?}", url, dest);

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Download request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Download failed: HTTP {}", resp.status()));
        }

        let total = resp.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;

        let temp_path = dest.with_extension("bin.downloading");
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| format!("Failed to create temp file: {}", e))?;

        use futures_util::StreamExt;
        let mut stream = resp.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Download stream error: {}", e))?;
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
                .await
                .map_err(|e| format!("Write error: {}", e))?;
            downloaded += chunk.len() as u64;

            let percent = if total > 0 {
                (downloaded as f32 / total as f32) * 100.0
            } else {
                0.0
            };

            let _ = progress_tx.try_send(DownloadProgress {
                model_key: model_key.to_string(),
                percent,
                downloaded_bytes: downloaded,
                total_bytes: total,
            });
        }

        tokio::io::AsyncWriteExt::flush(&mut file)
            .await
            .map_err(|e| format!("Flush error: {}", e))?;

        drop(file);

        std::fs::rename(&temp_path, &dest)
            .map_err(|e| format!("Failed to rename temp file: {}", e))?;

        tracing::info!("Model {} downloaded successfully", size.filename());

        Ok(format!(
            "Model {} downloaded ({} MB)",
            size.filename(),
            downloaded / 1024 / 1024
        ))
    }

    pub fn verify_model(&self, model_key: &str) -> bool {
        if let Some(size) = WhisperModelSize::from_key(model_key) {
            let path = self.model_path(size);
            if let Ok(metadata) = std::fs::metadata(&path) {
                let expected = size.size_mb() * 1024 * 1024;
                let actual = metadata.len();
                actual > expected * 9 / 10
            } else {
                false
            }
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub model_key: String,
    pub percent: f32,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}
