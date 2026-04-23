use crate::asr::engine::LanguageInfo;
use crate::asr::models::{DownloadProgress, ModelInfo, ModelManager};
use crate::asr::whisper::WhisperEngine;
use crate::audio::extractor::MediaInfo;
use crate::config::settings::AppConfig;
use crate::pipeline::orchestrator::{PipelineConfig, PipelineOrchestrator, PipelineStage};
use crate::translate::{
    create_registry, AllPluginConfigs, PluginConfig, PluginInfo, SharedRegistry,
    TranslateEngine, TranslateProgress, TranslateRequest, TranslateResult,
};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{mpsc, watch, Mutex};

pub struct AppState {
    pub models_dir: PathBuf,
    pub tmp_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub cancel_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
    pub is_running: Arc<std::sync::atomic::AtomicBool>,
    pub plugin_configs: Arc<Mutex<AllPluginConfigs>>,
    pub plugin_registry: SharedRegistry,
}

impl AppState {
    pub fn new(models_dir: PathBuf, tmp_dir: PathBuf, cache_dir: PathBuf) -> Self {
        Self {
            models_dir,
            tmp_dir,
            cache_dir,
            cancel_tx: Arc::new(Mutex::new(None)),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            plugin_configs: Arc::new(Mutex::new(AllPluginConfigs::default())),
            plugin_registry: create_registry(),
        }
    }
}

struct PluginTranslateAdapter {
    namespace: String,
    config: PluginConfig,
}

#[async_trait::async_trait]
impl TranslateEngine for PluginTranslateAdapter {
    async fn translate(
        &self,
        request: &TranslateRequest,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, crate::error::TranslateError> {
        let registry = crate::translate::create_registry();
        crate::translate::translate_with_plugin(
            &registry,
            &self.namespace,
            request,
            &self.config,
            progress_tx,
        )
        .await
    }

    fn name(&self) -> &str {
        &self.namespace
    }

    fn requires_network(&self) -> bool {
        true
    }

    fn supported_pairs(&self) -> Vec<(String, String)> {
        vec![]
    }
}

#[tauri::command]
pub async fn start_pipeline(
    app: AppHandle,
    state: State<'_, AppState>,
    config: PipelineConfig,
) -> Result<String, String> {
    if state
        .is_running
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err("A pipeline is already running".into());
    }

    let manager = ModelManager::new(state.models_dir.clone());
    manager.check_model(&config.asr_model)?;

    state
        .is_running
        .store(true, std::sync::atomic::Ordering::Relaxed);

    let (cancel_tx, cancel_rx) = watch::channel(false);
    {
        let mut tx = state.cancel_tx.lock().await;
        *tx = Some(cancel_tx);
    }

    let (stage_tx, mut stage_rx) = mpsc::channel::<PipelineStage>(64);

    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(stage) = stage_rx.recv().await {
            let _ = app_clone.emit("pipeline-progress", &stage);
        }
    });

    let models_dir = state.models_dir.clone();
    let tmp_dir = state.tmp_dir.clone();
    let is_running = state.is_running.clone();
    let plugin_namespace = config.translate_engine.clone();
    let plugin_config = {
        let all_configs = state.plugin_configs.lock().await;
        all_configs
            .configs
            .get(&plugin_namespace)
            .cloned()
            .unwrap_or_else(|| PluginConfig::new(&plugin_namespace))
    };

    tokio::spawn(async move {
        tracing::info!("Pipeline started for: {}", config.input_path);
        tracing::info!(
            "Source language: {:?}, Target language: {}",
            config.source_language,
            config.target_language
        );
        tracing::info!("Translation plugin: {}", plugin_namespace);

        let model_path = models_dir.join("whisper").join(format!(
            "ggml-{}.bin",
            config.asr_model
        ));

        let asr_engine = WhisperEngine::new(model_path, config.use_gpu);

        let translate_engine = Box::new(PluginTranslateAdapter {
            namespace: plugin_namespace.clone(),
            config: plugin_config,
        });

        let orchestrator = PipelineOrchestrator::new(
            Box::new(asr_engine),
            translate_engine,
            tmp_dir,
        );

        let result = orchestrator.run(config, stage_tx, cancel_rx).await;

        if let Err(e) = result {
            tracing::error!("Pipeline failed: {}", e);
        }

        is_running.store(false, std::sync::atomic::Ordering::Relaxed);
    });

    Ok("Pipeline started".into())
}

#[tauri::command]
pub async fn cancel_pipeline(state: State<'_, AppState>) -> Result<(), String> {
    let tx = state.cancel_tx.lock().await;
    if let Some(ref cancel_tx) = *tx {
        cancel_tx.send(true).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let manager = ModelManager::new(state.models_dir.clone());
    Ok(manager.list_models())
}

#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model_key: String,
) -> Result<String, String> {
    let manager = ModelManager::new(state.models_dir.clone());

    let (progress_tx, mut progress_rx) = mpsc::channel::<DownloadProgress>(32);

    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let _ = app_clone.emit("model-download-progress", &progress);
        }
    });

    manager.download_model(&model_key, progress_tx).await
}

#[tauri::command]
pub async fn open_model_directory(state: State<'_, AppState>) -> Result<(), String> {
    let models_dir = state.models_dir.clone();
    std::fs::create_dir_all(&models_dir).map_err(|e| format!("Failed to create dir: {}", e))?;

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&models_dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&models_dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&models_dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn check_model_exists(
    state: State<'_, AppState>,
    model_key: String,
) -> Result<bool, String> {
    let manager = ModelManager::new(state.models_dir.clone());
    Ok(manager.check_model(&model_key).is_ok())
}

#[tauri::command]
pub async fn list_translate_plugins(state: State<'_, AppState>) -> Result<Vec<PluginInfo>, String> {
    let registry = state.plugin_registry.read().await;
    Ok(registry.list_plugins())
}

#[tauri::command]
pub async fn get_plugin_configs(state: State<'_, AppState>) -> Result<AllPluginConfigs, String> {
    Ok(state.plugin_configs.lock().await.clone())
}

#[tauri::command]
pub async fn save_plugin_configs(
    state: State<'_, AppState>,
    configs: AllPluginConfigs,
) -> Result<(), String> {
    let mut guard = state.plugin_configs.lock().await;
    *guard = configs;
    tracing::info!("Plugin configs saved, active: {}", guard.active_plugin);
    Ok(())
}

#[tauri::command]
pub async fn health_check_plugin(
    state: State<'_, AppState>,
    namespace: String,
) -> Result<crate::translate::HealthStatus, String> {
    let config = {
        let all_configs = state.plugin_configs.lock().await;
        all_configs.configs.get(&namespace)
            .cloned()
            .unwrap_or_else(|| PluginConfig::new(&namespace))
    };
    let registry = state.plugin_registry.read().await;
    let plugin = registry.get(&namespace).ok_or_else(|| format!("Plugin '{}' not found", namespace))?;
    Ok(plugin.health_check(&config).await)
}

#[tauri::command]
pub async fn probe_video(path: String) -> Result<MediaInfo, String> {
    let path = std::path::Path::new(&path);
    crate::audio::extractor::AudioExtractor::probe(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_supported_languages() -> Vec<LanguageInfo> {
    vec![
        LanguageInfo { code: "auto".into(), name: "Auto Detect".into() },
        LanguageInfo { code: "en".into(), name: "English".into() },
        LanguageInfo { code: "zh".into(), name: "中文".into() },
        LanguageInfo { code: "ja".into(), name: "日本語".into() },
        LanguageInfo { code: "ko".into(), name: "한국어".into() },
        LanguageInfo { code: "fr".into(), name: "Français".into() },
        LanguageInfo { code: "de".into(), name: "Deutsch".into() },
        LanguageInfo { code: "es".into(), name: "Español".into() },
        LanguageInfo { code: "ru".into(), name: "Русский".into() },
        LanguageInfo { code: "pt".into(), name: "Português".into() },
        LanguageInfo { code: "it".into(), name: "Italiano".into() },
        LanguageInfo { code: "ar".into(), name: "العربية".into() },
        LanguageInfo { code: "hi".into(), name: "हिन्दी".into() },
        LanguageInfo { code: "th".into(), name: "ไทย".into() },
        LanguageInfo { code: "vi".into(), name: "Tiếng Việt".into() },
    ]
}

#[tauri::command]
pub async fn get_app_config() -> Result<AppConfig, String> {
    Ok(AppConfig::default())
}

#[tauri::command]
pub async fn save_app_config(config: AppConfig) -> Result<(), String> {
    tracing::info!("Saving config: {:?}", config);
    Ok(())
}
