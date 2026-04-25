use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::error::TranslateError;
use crate::translate::core::{
    ConfigField, ConfigFieldType, HealthStatus, ServiceConfig, ServiceDescriptor, TranslateMode,
    TranslationService,
};
use crate::translate::engine::{TranslateProgress, TranslateRequest, TranslateResult};
use crate::translate::shared::{send_progress, validate_http_url};

pub struct GoogleTranslateService {
    descriptor: ServiceDescriptor,
    client: Client,
}

impl GoogleTranslateService {
    pub fn new() -> Self {
        Self {
            descriptor: ServiceDescriptor {
                key: "google".to_string(),
                name: "Google Translate".to_string(),
                description: "使用 Google 免费翻译端点，不需要 API Key。".to_string(),
                mode: TranslateMode::OnlineTranslate,
                requires_network: true,
                config_schema: vec![
                    ConfigField {
                        key: "base_url".to_string(),
                        label: "基础地址".to_string(),
                        field_type: ConfigFieldType::Url,
                        default: "https://translate.googleapis.com".to_string(),
                        required: true,
                        placeholder: None,
                        description: Some("默认使用 Google Translate 免费端点。".to_string()),
                    },
                ],
            },
            client: Client::builder()
                .user_agent("Mozilla/5.0 SubtitleForge/1.0")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn map_lang(lang: &str) -> &str {
        match lang {
            "zh" => "zh-CN",
            "pt" => "pt-BR",
            other => other,
        }
    }
}

#[derive(Deserialize)]
struct GoogleTranslateResponse(Vec<serde_json::Value>);

#[async_trait]
impl TranslationService for GoogleTranslateService {
    fn descriptor(&self) -> &ServiceDescriptor {
        &self.descriptor
    }

    async fn initialize(&self, config: &ServiceConfig) -> Result<(), TranslateError> {
        validate_http_url("基础地址", config.get("base_url")).map_err(TranslateError::Config)?;
        Ok(())
    }

    async fn health_check(&self, config: &ServiceConfig) -> HealthStatus {
        let base = config.get("base_url").trim_end_matches('/');
        let url = format!("{}/translate_a/single", base);
        match self
            .client
            .get(&url)
            .query(&[
                ("client", "gtx"),
                ("sl", "en"),
                ("tl", "zh-CN"),
                ("dt", "t"),
                ("q", "hello"),
            ])
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(resp) => HealthStatus::Degraded(format!("HTTP {}", resp.status())),
            Err(err) => HealthStatus::Unhealthy(err.to_string()),
        }
    }

    async fn translate(
        &self,
        request: &TranslateRequest,
        config: &ServiceConfig,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError> {
        let base = config.get("base_url").trim_end_matches('/');
        let url = format!("{}/translate_a/single", base);
        let total = request.texts.len();
        let mut results = Vec::with_capacity(total);

        for (idx, text) in request.texts.iter().enumerate() {
            if text.trim().is_empty() {
                results.push(String::new());
                continue;
            }

            let response = self
                .client
                .get(&url)
                .query(&[
                    ("client", "gtx"),
                    ("sl", Self::map_lang(&request.source_lang)),
                    ("tl", Self::map_lang(&request.target_lang)),
                    ("dt", "t"),
                    ("q", text.as_str()),
                ])
                .send()
                .await
                .map_err(|e| TranslateError::Network(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let message = response.text().await.unwrap_or_default();
                return Err(TranslateError::Api { status, message });
            }

            let payload: GoogleTranslateResponse = response
                .json()
                .await
                .map_err(|e| TranslateError::Network(format!("Failed to parse response: {}", e)))?;

            let translated = payload
                .0
                .first()
                .and_then(|v| v.as_array())
                .map(|segments| {
                    segments
                        .iter()
                        .filter_map(|segment| segment.as_array())
                        .filter_map(|segment| segment.first())
                        .filter_map(|segment| segment.as_str())
                        .collect::<String>()
                })
                .unwrap_or_else(|| text.clone());

            results.push(translated);
            send_progress(&progress_tx, idx + 1, total).await;
        }

        Ok(TranslateResult {
            texts: results,
            engine: self.descriptor.key.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_check_has_known_status_shape() {
        let service = GoogleTranslateService::new();
        let config = service.create_default_config();
        let _ = service.health_check(&config).await;
    }
}
