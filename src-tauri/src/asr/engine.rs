// ============================================================
// asr/engine.rs — ASR engine trait definition
// ============================================================

use async_trait::async_trait;
use crate::error::AsrError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::sync::mpsc;

/// A single recognized segment with timestamps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// Segment index (0-based)
    pub index: usize,
    /// Start time in milliseconds
    pub start_ms: u64,
    /// End time in milliseconds
    pub end_ms: u64,
    /// Recognized text
    pub text: String,
    /// Detected language (ISO 639-1)
    pub language: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
}

/// ASR configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrConfig {
    /// Path to the model file
    pub model_path: std::path::PathBuf,
    /// Source language (None = auto-detect)
    pub language: Option<String>,
    /// Use Whisper's built-in translate-to-English
    pub translate_to_english: bool,
    /// Number of threads for inference
    pub n_threads: u32,
    /// Enable GPU acceleration
    pub use_gpu: bool,
    /// Temperature for sampling (0.0 = greedy)
    pub temperature: f32,
    /// Maximum segment length in characters (for splitting long segments)
    pub max_segment_length: Option<usize>,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            model_path: std::path::PathBuf::new(),
            language: None,
            translate_to_english: false,
            n_threads: num_cpus::get() as u32,
            use_gpu: false,
            temperature: 0.0,
            max_segment_length: None,
        }
    }
}

/// Progress update during ASR
#[derive(Debug, Clone)]
pub struct AsrProgress {
    pub percent: f32,
    pub current_segment: Option<Segment>,
}

/// Language info
#[derive(Debug, Clone, Serialize)]
pub struct LanguageInfo {
    pub code: String,
    pub name: String,
}

/// ASR engine trait — extensible for different backends
#[async_trait]
pub trait AsrEngine: Send + Sync {
    /// Perform speech recognition on audio file
    async fn transcribe(
        &self,
        audio_path: &Path,
        config: &AsrConfig,
        progress_tx: mpsc::Sender<AsrProgress>,
    ) -> Result<Vec<Segment>, AsrError>;

    /// Get list of supported languages
    fn supported_languages(&self) -> Vec<LanguageInfo>;

    /// Get engine name
    fn name(&self) -> &str;
}
