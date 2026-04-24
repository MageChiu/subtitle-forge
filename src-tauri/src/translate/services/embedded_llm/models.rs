use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum EmbeddedModelPreset {
    Qwen25_15B,
    Qwen25_3B,
    Llama32_3B,
}

impl EmbeddedModelPreset {
    pub fn key(&self) -> &str {
        match self {
            Self::Qwen25_15B => "qwen2.5-1.5b-instruct-q4km",
            Self::Qwen25_3B => "qwen2.5-3b-instruct-q4km",
            Self::Llama32_3B => "llama-3.2-3b-instruct-q4km",
        }
    }

    pub fn filename(&self) -> &str {
        match self {
            Self::Qwen25_15B => "Qwen2.5-1.5B-Instruct-Q4_K_M.gguf",
            Self::Qwen25_3B => "Qwen2.5-3B-Instruct-Q4_K_M.gguf",
            Self::Llama32_3B => "Llama-3.2-3B-Instruct-Q4_K_M.gguf",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Qwen25_15B => "Qwen2.5 1.5B Instruct Q4_K_M",
            Self::Qwen25_3B => "Qwen2.5 3B Instruct Q4_K_M",
            Self::Llama32_3B => "Llama 3.2 3B Instruct Q4_K_M",
        }
    }

    pub fn size_mb(&self) -> u64 {
        match self {
            Self::Qwen25_15B => 980,
            Self::Qwen25_3B => 1900,
            Self::Llama32_3B => 2010,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Qwen25_15B => "体积较小，适合本地双语字幕翻译调试",
            Self::Qwen25_3B => "效果和速度更均衡，适合作为默认本地翻译模型",
            Self::Llama32_3B => "通用能力较强，适合英文与多语种翻译场景",
        }
    }

    pub fn download_url(&self) -> &str {
        match self {
            Self::Qwen25_15B => "https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf",
            Self::Qwen25_3B => "https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf",
            Self::Llama32_3B => "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
        }
    }

    pub fn default_model_id(&self) -> &str {
        match self {
            Self::Qwen25_15B => "qwen2.5-1.5b-instruct",
            Self::Qwen25_3B => "qwen2.5-3b-instruct",
            Self::Llama32_3B => "llama-3.2-3b-instruct",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "qwen2.5-1.5b-instruct-q4km" => Some(Self::Qwen25_15B),
            "qwen2.5-3b-instruct-q4km" => Some(Self::Qwen25_3B),
            "llama-3.2-3b-instruct-q4km" => Some(Self::Llama32_3B),
            _ => None,
        }
    }

    pub fn all() -> [Self; 3] {
        [Self::Qwen25_15B, Self::Qwen25_3B, Self::Llama32_3B]
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddedModelInfo {
    pub key: String,
    pub name: String,
    pub size_mb: u64,
    pub description: String,
    pub path: String,
    pub downloaded: bool,
    pub download_url: String,
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddedDownloadProgress {
    pub model_key: String,
    pub percent: f32,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

pub struct EmbeddedModelManager {
    models_dir: PathBuf,
}

impl EmbeddedModelManager {
    pub fn new(models_dir: PathBuf) -> Self {
        Self { models_dir }
    }

    pub fn model_path(&self, preset: EmbeddedModelPreset) -> PathBuf {
        self.models_dir
            .join("embedded_llm")
            .join(preset.filename())
    }

    pub fn list_models(&self) -> Vec<EmbeddedModelInfo> {
        EmbeddedModelPreset::all()
            .iter()
            .map(|preset| {
                let path = self.model_path(*preset);
                EmbeddedModelInfo {
                    key: preset.key().to_string(),
                    name: preset.display_name().to_string(),
                    size_mb: preset.size_mb(),
                    description: preset.description().to_string(),
                    path: path.to_string_lossy().to_string(),
                    downloaded: path.exists(),
                    download_url: preset.download_url().to_string(),
                    model_id: preset.default_model_id().to_string(),
                }
            })
            .collect()
    }

    pub fn check_model(&self, model_key: &str) -> Result<PathBuf, String> {
        let preset = EmbeddedModelPreset::from_key(model_key)
            .ok_or_else(|| format!("Unknown embedded model: {}", model_key))?;
        let path = self.model_path(preset);
        if path.exists() {
            Ok(path)
        } else {
            Err(format!(
                "Embedded LLM model '{}' not found. Please download it in Settings.\nPath: {:?}",
                preset.filename(),
                path
            ))
        }
    }

    pub async fn download_model(
        &self,
        model_key: &str,
        progress_tx: tokio::sync::mpsc::Sender<EmbeddedDownloadProgress>,
    ) -> Result<String, String> {
        let preset = EmbeddedModelPreset::from_key(model_key)
            .ok_or_else(|| format!("Unknown embedded model: {}", model_key))?;
        let url = preset.download_url();
        let dest = self.model_path(preset);

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        if dest.exists() {
            return Ok(format!("Model {} already downloaded", preset.filename()));
        }

        let client = reqwest::Client::new();
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Download request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Download failed: HTTP {}", resp.status()));
        }

        let total = resp.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let temp_path = dest.with_extension("gguf.downloading");
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

            let _ = progress_tx.try_send(EmbeddedDownloadProgress {
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

        Ok(format!(
            "Embedded model {} downloaded ({} MB)",
            preset.filename(),
            downloaded / 1024 / 1024
        ))
    }
}
