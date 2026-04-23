// ============================================================
// translate/engine.rs — Translation engine trait
// ============================================================

use async_trait::async_trait;
use crate::error::TranslateError;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Translation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateRequest {
    /// Source texts (one per segment)
    pub texts: Vec<String>,
    /// Source language (ISO 639-1)
    pub source_lang: String,
    /// Target language (ISO 639-1)
    pub target_lang: String,
    /// Optional context hint for better translation
    pub context_hint: Option<String>,
}

/// Translation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateResult {
    /// Translated texts (1:1 with input)
    pub texts: Vec<String>,
    /// Engine used
    pub engine: String,
}

/// Translation progress
#[derive(Debug, Clone)]
pub struct TranslateProgress {
    pub percent: f32,
    pub translated_count: usize,
    pub total_count: usize,
}

/// Translation engine trait
#[async_trait]
pub trait TranslateEngine: Send + Sync {
    /// Translate a batch of texts
    async fn translate(
        &self,
        request: &TranslateRequest,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError>;

    /// Engine display name
    fn name(&self) -> &str;

    /// Whether this engine requires network
    fn requires_network(&self) -> bool;

    /// Supported language pairs (empty = any pair)
    fn supported_pairs(&self) -> Vec<(String, String)>;
}
