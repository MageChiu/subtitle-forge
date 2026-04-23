use super::engine::*;
use super::plugin::*;
use crate::error::TranslateError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub struct LibreTranslatePlugin {
    metadata: PluginMetadata,
    client: reqwest::Client,
}

impl LibreTranslatePlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                namespace: "libre/v1".to_string(),
                display_name: "LibreTranslate".to_string(),
                description: "Open-source translation API — self-hosted or public instance".to_string(),
                version: "1.0.0".to_string(),
                category: PluginCategory::RemoteApi,
                requires_network: true,
                config_schema: vec![
                    ConfigField {
                        key: "base_url".to_string(),
                        label: "LibreTranslate URL".to_string(),
                        field_type: ConfigFieldType::Url,
                        default: "https://libretranslate.de".to_string(),
                        required: true,
                        placeholder: Some("https://libretranslate.de".to_string()),
                        description: Some("Self-host or use a public instance".to_string()),
                    },
                    ConfigField {
                        key: "api_key".to_string(),
                        label: "API Key (optional)".to_string(),
                        field_type: ConfigFieldType::Password,
                        default: String::new(),
                        required: false,
                        placeholder: Some("Required for some instances".to_string()),
                        description: None,
                    },
                ],
            },
            client: reqwest::Client::new(),
        }
    }

    fn to_libre_lang(lang: &str) -> &str {
        match lang {
            "zh" => "zh",
            "pt" => "pt",
            other => other,
        }
    }
}

#[async_trait]
impl TranslationPlugin for LibreTranslatePlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn translate(
        &self,
        request: &TranslateRequest,
        config: &PluginConfig,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError> {
        let base_url = config.get("base_url");
        let api_key = config.get("api_key");
        let total = request.texts.len();
        let mut all_translations = Vec::with_capacity(total);

        for (idx, text) in request.texts.iter().enumerate() {
            if text.trim().is_empty() {
                all_translations.push(String::new());
                continue;
            }

            #[derive(Serialize)]
            struct LibreRequest {
                q: String,
                source: String,
                target: String,
                format: String,
                #[serde(skip_serializing_if = "String::is_empty")]
                api_key: String,
            }

            #[derive(Deserialize)]
            struct LibreResponse {
                translated_text: String,
            }

            let body = LibreRequest {
                q: text.clone(),
                source: Self::to_libre_lang(&request.source_lang).to_string(),
                target: Self::to_libre_lang(&request.target_lang).to_string(),
                format: "text".to_string(),
                api_key: api_key.to_string(),
            };

            let url = format!("{}/translate", base_url);

            let response = self
                .client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| TranslateError::Network(format!("LibreTranslate connection failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                if status == 403 {
                    return Err(TranslateError::InvalidApiKey);
                }
                return Err(TranslateError::Api { status, message: body });
            }

            let libre_resp: LibreResponse = response
                .json()
                .await
                .map_err(|e| TranslateError::Network(format!("Failed to parse LibreTranslate response: {}", e)))?;

            all_translations.push(libre_resp.translated_text);

            if idx % 5 == 0 || idx == total - 1 {
                let _ = progress_tx.send(TranslateProgress {
                    percent: ((idx + 1) as f32 / total as f32) * 100.0,
                    translated_count: idx + 1,
                    total_count: total,
                }).await;
            }

            if idx % 5 == 4 {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
        }

        Ok(TranslateResult {
            texts: all_translations,
            engine: self.metadata.namespace.clone(),
        })
    }

    async fn health_check(&self, config: &PluginConfig) -> HealthStatus {
        let base_url = config.get("base_url");
        match self
            .client
            .get(format!("{}/languages", base_url))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(resp) => HealthStatus::Degraded(format!("HTTP {}", resp.status())),
            Err(e) => HealthStatus::Unhealthy(format!("Cannot connect: {}", e)),
        }
    }
}
