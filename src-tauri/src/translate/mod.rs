pub mod deepl;
pub mod engine;
pub mod google;
pub mod libre;
pub mod llm_api;
pub mod middleware;
pub mod ollama;
pub mod plugin;

pub use engine::{TranslateEngine, TranslateProgress, TranslateRequest, TranslateResult};
pub use middleware::MiddlewarePlugin;
pub use plugin::{
    create_registry, ConfigField, ConfigFieldType, HealthStatus,
    PluginCategory, PluginConfig, PluginInfo, PluginMetadata, PluginRegistry,
    TranslationPlugin,
};

use crate::error::TranslateError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type SharedRegistry = Arc<RwLock<PluginRegistry>>;

pub async fn translate_with_plugin(
    registry: &SharedRegistry,
    namespace: &str,
    request: &TranslateRequest,
    config: &PluginConfig,
    progress_tx: tokio::sync::mpsc::Sender<TranslateProgress>,
) -> Result<TranslateResult, TranslateError> {
    let rg = registry.read().await;
    let plugin = rg.get(namespace).ok_or_else(|| {
        TranslateError::Network(format!("Translation plugin '{}' not found", namespace))
    })?;

    plugin.translate(request, config, progress_tx).await
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AllPluginConfigs {
    pub active_plugin: String,
    pub configs: HashMap<String, PluginConfig>,
}

impl Default for AllPluginConfigs {
    fn default() -> Self {
        let mut configs = HashMap::new();
        configs.insert("google/v1".to_string(), PluginConfig::new("google/v1"));
        configs.insert("deepl/v1".to_string(), PluginConfig {
            namespace: "deepl/v1".to_string(),
            fields: HashMap::from([("api_key".to_string(), String::new())]),
        });
        configs.insert("libre/v1".to_string(), PluginConfig {
            namespace: "libre/v1".to_string(),
            fields: HashMap::from([
                ("base_url".to_string(), "https://libretranslate.de".to_string()),
                ("api_key".to_string(), String::new()),
            ]),
        });
        configs.insert("llm/v1".to_string(), PluginConfig {
            namespace: "llm/v1".to_string(),
            fields: HashMap::from([
                ("api_key".to_string(), String::new()),
                ("base_url".to_string(), "https://api.openai.com/v1".to_string()),
                ("model".to_string(), "gpt-4o-mini".to_string()),
                ("batch_size".to_string(), "20".to_string()),
            ]),
        });
        configs.insert("ollama/v1".to_string(), PluginConfig {
            namespace: "ollama/v1".to_string(),
            fields: HashMap::from([
                ("base_url".to_string(), "http://localhost:11434".to_string()),
                ("model".to_string(), "qwen2.5:7b".to_string()),
                ("batch_size".to_string(), "20".to_string()),
            ]),
        });
        Self {
            active_plugin: "google/v1".to_string(),
            configs,
        }
    }
}
