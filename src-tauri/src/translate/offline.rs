use super::engine::*;
use crate::error::TranslateError;
use async_trait::async_trait;
use tokio::sync::mpsc;

pub struct OfflineTranslateEngine {
    _model_dir: String,
}

impl OfflineTranslateEngine {
    pub fn new(model_dir: String) -> Self {
        Self { _model_dir: model_dir }
    }
}

#[async_trait]
impl TranslateEngine for OfflineTranslateEngine {
    async fn translate(
        &self,
        _request: &TranslateRequest,
        _progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError> {
        Err(TranslateError::Network(
            "Offline translation (Opus-MT) is not yet implemented. Requires ort + tokenizers crates.".into(),
        ))
    }

    fn name(&self) -> &str {
        "Opus-MT (Offline)"
    }

    fn requires_network(&self) -> bool {
        false
    }

    fn supported_pairs(&self) -> Vec<(String, String)> {
        vec![]
    }
}
