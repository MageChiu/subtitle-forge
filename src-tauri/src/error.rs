// ============================================================
// error.rs — Unified error types
// ============================================================

use thiserror::Error;

/// Top-level application error
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Audio extraction failed: {0}")]
    Audio(#[from] AudioError),

    #[error("ASR failed: {0}")]
    Asr(#[from] AsrError),

    #[error("Translation failed: {0}")]
    Translate(#[from] TranslateError),

    #[error("Subtitle error: {0}")]
    Subtitle(#[from] SubtitleError),

    #[error("Pipeline error: {0}")]
    Pipeline(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("No audio stream found in file")]
    NoAudioStream,

    #[error("FFmpeg error: {0}")]
    Ffmpeg(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum AsrError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Model load failed: {0}")]
    ModelLoad(String),

    #[error("Transcription failed: {0}")]
    Transcription(String),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),
}

#[derive(Error, Debug)]
pub enum TranslateError {
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("Network error: {0}")]
    Network(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Initialization error: {0}")]
    Initialization(String),

    #[error("Rate limited, retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("Unsupported language pair: {from} -> {target}")]
    UnsupportedPair { from: String, target: String },
}

#[derive(Error, Debug)]
pub enum SubtitleError {
    #[error("Parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    #[error("Segment count mismatch: ASR={asr}, Translation={translation}")]
    SegmentMismatch { asr: usize, translation: usize },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
