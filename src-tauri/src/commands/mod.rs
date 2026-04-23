// ============================================================
// commands/mod.rs — Tauri IPC command handlers
// ============================================================

use crate::asr::engine::LanguageInfo;
use crate::asr::models::{ModelInfo, ModelManager};
use crate::audio::extractor::MediaInfo;
use crate::config::settings::AppConfig;
use crate::pipeline::orchestrator::{PipelineConfig, PipelineOrchestrator, PipelineStage};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tokio::sync::{mpsc, watch, Mutex};

/// Global application state managed by Tauri
pub struct AppState {
    pub models_dir: PathBuf,
    pub tmp_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub cancel_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
    pub is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl AppState {
    pub fn new(models_dir: PathBuf, tmp_dir: PathBuf, cache_dir: PathBuf) -> Self {
        Self {
            models_dir,
            tmp_dir,
            cache_dir,
            cancel_tx: Arc::new(Mutex::new(None)),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

/// Start the processing pipeline
#[tauri::command]
pub async fn start_pipeline(
    app: AppHandle,
    state: State<'_, AppState>,
    config: PipelineConfig,
) -> Result<String, String> {
    // Prevent multiple concurrent runs
    if state
        .is_running
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err("A pipeline is already running".into());
    }
    state
        .is_running
        .store(true, std::sync::atomic::Ordering::Relaxed);

    // Create cancellation channel
    let (cancel_tx, cancel_rx) = watch::channel(false);
    {
        let mut tx = state.cancel_tx.lock().await;
        *tx = Some(cancel_tx);
    }

    // Create progress event channel
    let (stage_tx, mut stage_rx) = mpsc::channel::<PipelineStage>(64);

    // Forward pipeline stages as Tauri events
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(stage) = stage_rx.recv().await {
            let _ = app_clone.emit("pipeline-progress", &stage);
        }
    });

    // TODO: Initialize engines based on config
    // For now, return a descriptive error about what would happen
    //
    // let asr_engine = Box::new(WhisperEngine::new(
    //     PathBuf::from(&config.asr_model),
    //     config.use_gpu,
    // ));
    //
    // let translate_engine = match config.translate_engine.as_str() {
    //     "llm" => Box::new(LlmTranslateEngine::new(llm_config)) as Box<dyn TranslateEngine>,
    //     "deepl" => Box::new(DeepLTranslateEngine::new(api_key)) as Box<dyn TranslateEngine>,
    //     _ => return Err("Unknown translate engine".into()),
    // };
    //
    // let orchestrator = PipelineOrchestrator::new(
    //     asr_engine,
    //     translate_engine,
    //     state.tmp_dir.clone(),
    // );
    //
    // let result = orchestrator.run(config, stage_tx, cancel_rx).await;

    let is_running = state.is_running.clone();

    // Placeholder — in real implementation, this spawns the orchestrator
    tokio::spawn(async move {
        // orchestrator.run(config, stage_tx, cancel_rx).await
        is_running.store(false, std::sync::atomic::Ordering::Relaxed);
    });

    Ok("Pipeline started".into())
}

/// Cancel the running pipeline
#[tauri::command]
pub async fn cancel_pipeline(state: State<'_, AppState>) -> Result<(), String> {
    let tx = state.cancel_tx.lock().await;
    if let Some(ref cancel_tx) = *tx {
        cancel_tx.send(true).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// List available ASR models
#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let manager = ModelManager::new(state.models_dir.clone());
    Ok(manager.list_models())
}

/// Probe a video file for metadata
#[tauri::command]
pub async fn probe_video(path: String) -> Result<MediaInfo, String> {
    let path = std::path::Path::new(&path);
    crate::audio::extractor::AudioExtractor::probe(path).map_err(|e| e.to_string())
}

/// Get supported language list
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

/// Get application configuration
#[tauri::command]
pub async fn get_app_config() -> Result<AppConfig, String> {
    // Load from settings file or return defaults
    Ok(AppConfig::default())
}

/// Save application configuration
#[tauri::command]
pub async fn save_app_config(config: AppConfig) -> Result<(), String> {
    // Save to settings file
    tracing::info!("Saving config: {:?}", config);
    Ok(())
}
