use super::engine::*;
use super::plugin::*;
use crate::error::TranslateError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub struct DeepLTranslatePlugin {
    metadata: PluginMetadata,
    client: reqwest::Client,
}

impl DeepLTranslatePlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                namespace: "deepl/v1".to_string(),
                display_name: "DeepL".to_string(),
                description: "High-quality translation via DeepL API (free or pro)".to_string(),
                version: "1.0.0".to_string(),
                category: PluginCategory::RemoteApi,
                requires_network: true,
                config_schema: vec![
                    ConfigField {
                        key: "api_key".to_string(),
                        label: "DeepL API Key".to_string(),
                        field_type: ConfigFieldType::Password,
                        default: String::new(),
                        required: true,
                        placeholder: Some("Use :fx suffix for free tier".to_string()),
                        description: Some("Get your key at deepl.com/pro#developer".to_string()),
                    },
                ],
            },
            client: reqwest::Client::new(),
        }
    }

    fn base_url(api_key: &str) -> &str {
        if api_key.ends_with(":fx") {
            "https://api-free.deepl.com/v2"
        } else {
            "https://api.deepl.com/v2"
        }
    }

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
impl TranslationPlugin for DeepLTranslatePlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn translate(
        &self,
        request: &TranslateRequest,
        config: &PluginConfig,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError> {
        let api_key = config.get("api_key");
        if api_key.is_empty() {
            return Err(TranslateError::InvalidApiKey);
        }

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
                .post(format!("{}/translate", Self::base_url(api_key)))
                .header("Authorization", format!("DeepL-Auth-Key {}", api_key))
                .json(&body)
                .send()
                .await
                .map_err(|e| TranslateError::Network(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                if status == 403 {
                    return Err(TranslateError::InvalidApiKey);
                }
                return Err(TranslateError::Api { status, message: body });
            }

            let deepl_resp: DeepLResponse = response
                .json()
                .await
                .map_err(|e| TranslateError::Network(e.to_string()))?;

            all_translations.extend(deepl_resp.translations.into_iter().map(|t| t.text));

            let _ = progress_tx.send(TranslateProgress {
                percent: (all_translations.len() as f32 / total as f32) * 100.0,
                translated_count: all_translations.len(),
                total_count: total,
            }).await;
        }

        Ok(TranslateResult {
            texts: all_translations,
            engine: self.metadata.namespace.clone(),
        })
    }

    async fn health_check(&self, config: &PluginConfig) -> HealthStatus {
        let api_key = config.get("api_key");
        if api_key.is_empty() {
            return HealthStatus::Unhealthy("API key not configured".to_string());
        }
        let url = format!("{}/usage", Self::base_url(api_key));
        match self
            .client
            .get(&url)
            .header("Authorization", format!("DeepL-Auth-Key {}", api_key))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(resp) if resp.status().as_u16() == 403 => HealthStatus::Unhealthy("Invalid API key".to_string()),
            Ok(resp) => HealthStatus::Degraded(format!("HTTP {}", resp.status())),
            Err(e) => HealthStatus::Unhealthy(e.to_string()),
        }
    }
}
