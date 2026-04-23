// ============================================================
// translate/offline.rs — Offline translation via ONNX Runtime + Opus-MT
// ============================================================

#[cfg(feature = "offline-translate")]
pub mod opus_mt {
    use super::super::engine::*;
    use crate::error::TranslateError;
    use async_trait::async_trait;
    use tokio::sync::mpsc;

    /// Offline translation engine using Helsinki-NLP/Opus-MT models via ONNX Runtime
    pub struct OpusMtEngine {
        model_dir: std::path::PathBuf,
        // ort_session: ort::Session,
        // tokenizer: tokenizers::Tokenizer,
    }

    impl OpusMtEngine {
        pub fn new(model_dir: std::path::PathBuf) -> Result<Self, TranslateError> {
            // Implementation:
            //
            // let model_path = model_dir.join("model.onnx");
            // let tokenizer_path = model_dir.join("tokenizer.json");
            //
            // let session = ort::Session::builder()?
            //     .with_optimization_level(ort::GraphOptimizationLevel::Level3)?
            //     .with_model_from_file(&model_path)?;
            //
            // let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            //     .map_err(|e| TranslateError::Network(e.to_string()))?;

            Ok(Self { model_dir })
        }
    }

    #[async_trait]
    impl TranslateEngine for OpusMtEngine {
        async fn translate(
            &self,
            request: &TranslateRequest,
            progress_tx: mpsc::Sender<TranslateProgress>,
        ) -> Result<TranslateResult, TranslateError> {
            // Opus-MT translates one text at a time (or small batches)
            //
            // for (i, text) in request.texts.iter().enumerate() {
            //     let encoding = self.tokenizer.encode(text, true)?;
            //     let input_ids = encoding.get_ids();
            //     // Run ONNX inference
            //     // Decode output tokens
            //     // Send progress
            // }

            todo!("Implement with ort + tokenizers crates")
        }

        fn name(&self) -> &str {
            "Opus-MT (Offline)"
        }

        fn requires_network(&self) -> bool {
            false
        }

        fn supported_pairs(&self) -> Vec<(String, String)> {
            // Depends on which Opus-MT model is loaded
            // e.g., opus-mt-en-zh, opus-mt-en-ja, etc.
            vec![]
        }
    }
}

/// Placeholder when offline-translate feature is disabled
#[cfg(not(feature = "offline-translate"))]
pub fn offline_translate_available() -> bool {
    false
}

#[cfg(feature = "offline-translate")]
pub fn offline_translate_available() -> bool {
    true
}
