use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::error::TranslateError;
use crate::translate::core::{
    ConfigField, ConfigFieldType, HealthStatus, ServiceConfig, ServiceDescriptor, TranslateMode,
    TranslationService, ValidationIssue,
};
use crate::translate::engine::{TranslateProgress, TranslateRequest, TranslateResult};
use crate::translate::shared::send_progress;

pub struct DeepLService {
    descriptor: ServiceDescriptor,
    client: Client,
}

impl DeepLService {
    pub fn new() -> Self {
        Self {
            descriptor: ServiceDescriptor {
                key: "deepl".to_string(),
                name: "DeepL".to_string(),
                description: "使用 DeepL 翻译 API 执行高质量翻译。".to_string(),
                mode: TranslateMode::OnlineTranslate,
                requires_network: true,
                config_schema: vec![
                    ConfigField {
                        key: "api_key".to_string(),
                        label: "API Key".to_string(),
                        field_type: ConfigFieldType::Password,
                        default: String::new(),
                        required: true,
                        placeholder: Some("可使用 :fx 结尾的 Free Key".to_string()),
                        description: Some("DeepL Free 和 Pro 均可。".to_string()),
                    },
                ],
            },
            client: Client::new(),
        }
    }

    fn api_base(api_key: &str) -> &str {
        if api_key.ends_with(":fx") {
            "https://api-free.deepl.com/v2"
        } else {
            "https://api.deepl.com/v2"
        }
    }

    fn map_lang(lang: &str) -> String {
        match lang {
            "en" => "EN".to_string(),
            "zh" => "ZH".to_string(),
            "ja" => "JA".to_string(),
            "ko" => "KO".to_string(),
            "fr" => "FR".to_string(),
            "de" => "DE".to_string(),
            "es" => "ES".to_string(),
            "pt" => "PT-BR".to_string(),
            other => other.to_uppercase(),
        }
    }
}

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

#[async_trait]
impl TranslationService for DeepLService {
    fn descriptor(&self) -> &ServiceDescriptor {
        &self.descriptor
    }

    fn validate_config(&self, config: &ServiceConfig) -> Result<(), Vec<ValidationIssue>> {
        if config.get("api_key").trim().is_empty() {
            Err(vec![ValidationIssue {
                field: "api_key".to_string(),
                message: "API Key不能为空".to_string(),
            }])
        } else {
            Ok(())
        }
    }

    async fn initialize(&self, config: &ServiceConfig) -> Result<(), TranslateError> {
        self.validate_config(config)
            .map_err(|issues| TranslateError::Config(issues[0].message.clone()))?;
        Ok(())
    }

    async fn health_check(&self, config: &ServiceConfig) -> HealthStatus {
        let endpoint = format!("{}/usage", Self::api_base(config.get("api_key")));
        match self
            .client
            .get(&endpoint)
            .header("Authorization", format!("DeepL-Auth-Key {}", config.get("api_key")))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(resp) if resp.status().as_u16() == 403 => {
                HealthStatus::Unhealthy("API Key 无效".to_string())
            }
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
        let total = request.texts.len();
        let mut results = Vec::with_capacity(total);
        let batch_size = 50usize;

        for chunk in request.texts.chunks(batch_size) {
            let body = DeepLRequest {
                text: chunk.to_vec(),
                source_lang: Self::map_lang(&request.source_lang),
                target_lang: Self::map_lang(&request.target_lang),
            };

            let response = self
                .client
                .post(format!("{}/translate", Self::api_base(config.get("api_key"))))
                .header("Authorization", format!("DeepL-Auth-Key {}", config.get("api_key")))
                .json(&body)
                .send()
                .await
                .map_err(|e| TranslateError::Network(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let message = response.text().await.unwrap_or_default();
                if status == 403 {
                    return Err(TranslateError::InvalidApiKey);
                }
                return Err(TranslateError::Api { status, message });
            }

            let payload: DeepLResponse = response
                .json()
                .await
                .map_err(|e| TranslateError::Network(format!("Failed to parse response: {}", e)))?;

            results.extend(payload.translations.into_iter().map(|item| item.text));
            send_progress(&progress_tx, results.len(), total).await;
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

    #[test]
    fn validates_api_key() {
        let service = DeepLService::new();
        let config = ServiceConfig::new("deepl");
        assert!(service.validate_config(&config).is_err());
    }
}
