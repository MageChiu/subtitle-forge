use crate::error::TranslateError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

use super::engine::{TranslateProgress, TranslateRequest, TranslateResult};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TranslateMode {
    OnlineTranslate,
    OnlineLlm,
    LocalLlm,
    EmbeddedLlm,
}

impl TranslateMode {
    pub fn key(&self) -> &'static str {
        match self {
            Self::OnlineTranslate => "online_translate",
            Self::OnlineLlm => "online_llm",
            Self::LocalLlm => "local_llm",
            Self::EmbeddedLlm => "embedded_llm",
        }
    }

    pub fn label_zh(&self) -> &'static str {
        match self {
            Self::OnlineTranslate => "在线翻译服务",
            Self::OnlineLlm => "在线LLM服务",
            Self::LocalLlm => "本地LLM服务",
            Self::EmbeddedLlm => "内嵌LLM服务",
        }
    }

    pub fn description_zh(&self) -> &'static str {
        match self {
            Self::OnlineTranslate => "通过专用翻译 API 执行翻译，例如 Google、LibreTranslate、DeepL。",
            Self::OnlineLlm => "通过云端大模型服务执行翻译，例如 DeepSeek、方舟。",
            Self::LocalLlm => "通过本地 HTTP API 调用模型服务，例如 Ollama。",
            Self::EmbeddedLlm => "通过 llama.cpp 等框架直接加载本地 GGUF 模型。",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            "online_translate" => Some(Self::OnlineTranslate),
            "online_llm" => Some(Self::OnlineLlm),
            "local_llm" => Some(Self::LocalLlm),
            "embedded_llm" => Some(Self::EmbeddedLlm),
            _ => None,
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::OnlineTranslate,
            Self::OnlineLlm,
            Self::LocalLlm,
            Self::EmbeddedLlm,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateModeInfo {
    pub key: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigFieldType {
    Text,
    Password,
    Url,
    Number,
    Path,
    Select { options: Vec<SelectOption> },
    Toggle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    pub field_type: ConfigFieldType,
    pub default: String,
    pub required: bool,
    pub placeholder: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDescriptor {
    pub key: String,
    pub name: String,
    pub description: String,
    pub mode: TranslateMode,
    pub requires_network: bool,
    pub config_schema: Vec<ConfigField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub descriptor: ServiceDescriptor,
    pub health_status: Option<HealthStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub service_key: String,
    pub fields: HashMap<String, String>,
}

impl ServiceConfig {
    pub fn new(service_key: &str) -> Self {
        Self {
            service_key: service_key.to_string(),
            fields: HashMap::new(),
        }
    }

    pub fn from_schema(service_key: &str, schema: &[ConfigField]) -> Self {
        let mut fields = HashMap::new();
        for field in schema {
            fields.insert(field.key.clone(), field.default.clone());
        }
        Self {
            service_key: service_key.to_string(),
            fields,
        }
    }

    pub fn get(&self, key: &str) -> &str {
        self.fields.get(key).map(|v| v.as_str()).unwrap_or("")
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.fields.insert(key.to_string(), value.to_string());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationSettings {
    pub active_mode: TranslateMode,
    pub active_service: String,
    pub service_configs: HashMap<String, ServiceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub field: String,
    pub message: String,
}

#[async_trait]
pub trait TranslationService: Send + Sync {
    fn descriptor(&self) -> &ServiceDescriptor;

    fn create_default_config(&self) -> ServiceConfig {
        ServiceConfig::from_schema(&self.descriptor().key, &self.descriptor().config_schema)
    }

    fn validate_config(&self, config: &ServiceConfig) -> Result<(), Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        for field in &self.descriptor().config_schema {
            if field.required && config.get(&field.key).trim().is_empty() {
                issues.push(ValidationIssue {
                    field: field.key.clone(),
                    message: format!("{}不能为空", field.label),
                });
            }
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues)
        }
    }

    async fn initialize(&self, config: &ServiceConfig) -> Result<(), TranslateError>;

    async fn health_check(&self, config: &ServiceConfig) -> HealthStatus;

    async fn translate(
        &self,
        request: &TranslateRequest,
        config: &ServiceConfig,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError>;
}

impl TranslationSettings {
    pub fn new(active_mode: TranslateMode, active_service: &str) -> Self {
        Self {
            active_mode,
            active_service: active_service.to_string(),
            service_configs: HashMap::new(),
        }
    }
}
