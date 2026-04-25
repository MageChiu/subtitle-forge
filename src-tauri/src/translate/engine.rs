use crate::error::TranslateError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateRequest {
    pub texts: Vec<String>,
    pub source_lang: String,
    pub target_lang: String,
    pub context_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateResult {
    pub texts: Vec<String>,
    pub engine: String,
}

#[derive(Debug, Clone)]
pub struct TranslateProgress {
    pub percent: f32,
    pub translated_count: usize,
    pub total_count: usize,
}

#[async_trait]
pub trait TranslateEngine: Send + Sync {
    async fn translate(
        &self,
        request: &TranslateRequest,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError>;

    fn name(&self) -> &str;

    fn requires_network(&self) -> bool;

    fn supported_pairs(&self) -> Vec<(String, String)>;
}
