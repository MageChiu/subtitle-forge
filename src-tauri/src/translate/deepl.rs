// ============================================================
// translate/deepl.rs — DeepL API translation
// ============================================================

use super::engine::*;
use crate::error::TranslateError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub struct DeepLTranslateEngine {
    client: reqwest::Client,
    api_key: String,
    is_free: bool,
}

impl DeepLTranslateEngine {
    pub fn new(api_key: String) -> Self {
        let is_free = api_key.ends_with(":fx");
        Self {
            client: reqwest::Client::new(),
            api_key,
            is_free,
        }
    }

    fn base_url(&self) -> &str {
        if self.is_free {
            "https://api-free.deepl.com/v2"
        } else {
            "https://api.deepl.com/v2"
        }
    }

    /// Map ISO 639-1 to DeepL language code
    fn to_deepl_lang(lang: &str) -> String {
        match lang {
            "en" => "EN".to_string(),
            "zh" => "ZH".to_string(),
            "ja" => "JA".to_string(),
            "ko" => "KO".to_string(),
            "fr" => "FR".to_string(),
            "de" => "DE".to_string(),
            "es" => "ES".to_string(),
            "pt" => "PT-BR".to_string(),
            "ru" => "RU".to_string(),
            "it" => "IT".to_string(),
            other => other.to_uppercase(),
        }
    }
}

#[async_trait]
impl TranslateEngine for DeepLTranslateEngine {
    async fn translate(
        &self,
        request: &TranslateRequest,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError> {
        // DeepL supports batch translation natively
        // Max 50 texts per request

        let total = request.texts.len();
        let mut all_translations = Vec::with_capacity(total);

        for chunk in request.texts.chunks(50) {
            #[derive(Serialize)]
            struct DeepLRequest {
                text: Vec<String>,
                source_lang: String,
                target_lang: String,
            }

            #[derive(Deserialize)]
            struct DeepLResponse {
                translations: Vec<DeepLTranslation>,
            }

            #[derive(Deserialize)]
            struct DeepLTranslation {
                text: String,
            }

            let body = DeepLRequest {
                text: chunk.to_vec(),
                source_lang: Self::to_deepl_lang(&request.source_lang),
                target_lang: Self::to_deepl_lang(&request.target_lang),
            };

            let response = self
                .client
                .post(format!("{}/translate", self.base_url()))
                .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
                .json(&body)
                .send()
                .await
                .map_err(|e| TranslateError::Network(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                return Err(TranslateError::Api {
                    status,
                    message: body,
                });
            }

            let deepl_resp: DeepLResponse = response
                .json()
                .await
                .map_err(|e| TranslateError::Network(e.to_string()))?;

            all_translations.extend(deepl_resp.translations.into_iter().map(|t| t.text));

            let _ = progress_tx
                .send(TranslateProgress {
                    percent: (all_translations.len() as f32 / total as f32) * 100.0,
                    translated_count: all_translations.len(),
                    total_count: total,
                })
                .await;
        }

        Ok(TranslateResult {
            texts: all_translations,
            engine: self.name().to_string(),
        })
    }

    fn name(&self) -> &str {
        "DeepL"
    }

    fn requires_network(&self) -> bool {
        true
    }

    fn supported_pairs(&self) -> Vec<(String, String)> {
        // DeepL supports many pairs but not all combinations
        vec![]
    }
}
