use super::engine::*;
use crate::error::TranslateError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

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
#[serde(rename_all = "snake_case")]
pub enum ConfigFieldType {
    Text,
    Password,
    Url,
    Number,
    Select { options: Vec<SelectOption> },
    Toggle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub namespace: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub category: PluginCategory,
    pub requires_network: bool,
    pub config_schema: Vec<ConfigField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCategory {
    RemoteApi,
    RemoteLlm,
    LocalLlm,
}

impl PluginCategory {
    pub fn display_name(&self) -> &str {
        match self {
            Self::RemoteApi => "Remote Translation API",
            Self::RemoteLlm => "Remote LLM",
            Self::LocalLlm => "Local LLM",
        }
    }

    pub fn key(&self) -> &str {
        match self {
            Self::RemoteApi => "remote_api",
            Self::RemoteLlm => "remote_llm",
            Self::LocalLlm => "local_llm",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub metadata: PluginMetadata,
    pub is_available: bool,
    pub health_status: Option<HealthStatus>,
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
pub struct PluginConfig {
    pub namespace: String,
    pub fields: HashMap<String, String>,
}

impl PluginConfig {
    pub fn new(namespace: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            fields: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> &str {
        self.fields.get(key).map(|s| s.as_str()).unwrap_or("")
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.fields.insert(key.to_string(), value.to_string());
    }

    pub fn from_schema(namespace: &str, schema: &[ConfigField]) -> Self {
        let mut fields = HashMap::new();
        for field in schema {
            fields.insert(field.key.clone(), field.default.clone());
        }
        Self {
            namespace: namespace.to_string(),
            fields,
        }
    }
}

#[async_trait]
pub trait TranslationPlugin: Send + Sync {
    fn metadata(&self) -> &PluginMetadata;

    async fn translate(
        &self,
        request: &TranslateRequest,
        config: &PluginConfig,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError>;

    async fn health_check(&self, config: &PluginConfig) -> HealthStatus;

    fn create_default_config(&self) -> PluginConfig {
        PluginConfig::from_schema(&self.metadata().namespace, &self.metadata().config_schema)
    }
}

type PluginFactory = Box<dyn Fn() -> Box<dyn TranslationPlugin> + Send + Sync>;

pub struct PluginRegistry {
    factories: HashMap<String, PluginFactory>,
    plugins: HashMap<String, Box<dyn TranslationPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
            plugins: HashMap::new(),
        }
    }

    pub fn register<P: TranslationPlugin + 'static>(&mut self, plugin: P) {
        let ns = plugin.metadata().namespace.clone();
        self.plugins.insert(ns, Box::new(plugin));
    }

    pub fn register_factory<F>(&mut self, namespace: &str, factory: F)
    where
        F: Fn() -> Box<dyn TranslationPlugin> + Send + Sync + 'static,
    {
        self.factories.insert(namespace.to_string(), Box::new(factory));
    }

    pub fn get(&self, namespace: &str) -> Option<&dyn TranslationPlugin> {
        self.plugins.get(namespace).map(|p| p.as_ref())
    }

    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins
            .values()
            .map(|p| {
                let metadata = p.metadata().clone();
                PluginInfo {
                    is_available: true,
                    health_status: Some(HealthStatus::Unknown),
                    metadata,
                }
            })
            .collect()
    }

    pub fn list_by_category(&self, category: &PluginCategory) -> Vec<PluginInfo> {
        self.plugins
            .values()
            .filter(|p| match (category, &p.metadata().category) {
                (PluginCategory::RemoteApi, PluginCategory::RemoteApi) => true,
                (PluginCategory::RemoteLlm, PluginCategory::RemoteLlm) => true,
                (PluginCategory::LocalLlm, PluginCategory::LocalLlm) => true,
                _ => false,
            })
            .map(|p| {
                let metadata = p.metadata().clone();
                PluginInfo {
                    is_available: true,
                    health_status: Some(HealthStatus::Unknown),
                    metadata,
                }
            })
            .collect()
    }

    pub fn namespaces(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn create_registry() -> Arc<RwLock<PluginRegistry>> {
    let mut registry = PluginRegistry::new();

    registry.register(crate::translate::google::GoogleTranslatePlugin::new());
    registry.register(crate::translate::deepl::DeepLTranslatePlugin::new());
    registry.register(crate::translate::libre::LibreTranslatePlugin::new());
    registry.register(crate::translate::llm_api::LlmTranslatePlugin::new());
    registry.register(crate::translate::ollama::OllamaTranslatePlugin::new());

    Arc::new(RwLock::new(registry))
}
