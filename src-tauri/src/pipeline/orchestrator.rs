// ============================================================
// pipeline/orchestrator.rs — Full pipeline orchestration
// ============================================================

use crate::audio::extractor::{AudioExtractor, ExtractConfig};
use crate::asr::engine::{AsrConfig, AsrEngine, Segment};
use crate::error::AppError;
use crate::subtitle::ass::{AssStyle, AssWriter};
use crate::subtitle::merger::SubtitleMerger;
use crate::subtitle::srt::SrtWriter;
use crate::subtitle::types::*;
use crate::subtitle::vtt::VttWriter;
use crate::translate::engine::{TranslateEngine, TranslateRequest};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Pipeline task configuration (from frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Input video file path
    pub input_path: String,
    /// Output directory
    pub output_dir: String,
    /// Source language (None = auto-detect)
    pub source_language: Option<String>,
    /// Target language for translation
    pub target_language: String,
    /// Output subtitle format
    pub output_format: SubtitleFormat,
    /// Whisper model path
    pub asr_model: String,
    /// Translation engine identifier
    pub translate_engine: String,
    /// Enable GPU acceleration
    pub use_gpu: bool,
}

/// Pipeline stage for progress tracking
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "stage")]
pub enum PipelineStage {
    #[serde(rename = "idle")]
    Idle,

    #[serde(rename = "extracting_audio")]
    ExtractingAudio { percent: f32 },

    #[serde(rename = "transcribing")]
    Transcribing {
        percent: f32,
        current_text: Option<String>,
    },

    #[serde(rename = "translating")]
    Translating {
        percent: f32,
        translated_count: usize,
        total_count: usize,
    },

    #[serde(rename = "generating_subtitle")]
    GeneratingSubtitle,

    #[serde(rename = "completed")]
    Completed {
        output_path: String,
        segment_count: usize,
        duration_ms: u64,
    },

    #[serde(rename = "failed")]
    Failed { error: String },

    #[serde(rename = "cancelled")]
    Cancelled,
}

/// Pipeline orchestrator — coordinates all processing stages
pub struct PipelineOrchestrator {
    asr_engine: Box<dyn AsrEngine>,
    translate_engine: Box<dyn TranslateEngine>,
    tmp_dir: PathBuf,
}

impl PipelineOrchestrator {
    pub fn new(
        asr_engine: Box<dyn AsrEngine>,
        translate_engine: Box<dyn TranslateEngine>,
        tmp_dir: PathBuf,
    ) -> Self {
        Self {
            asr_engine,
            translate_engine,
            tmp_dir,
        }
    }

    /// Execute the full pipeline
    pub async fn run(
        &self,
        config: PipelineConfig,
        stage_tx: mpsc::Sender<PipelineStage>,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<String, AppError> {
        let input_path = Path::new(&config.input_path);
        let output_dir = Path::new(&config.output_dir);

        // =============== Stage 1: Audio Extraction ===============
        tracing::info!("Stage 1: Extracting audio from {:?}", input_path);

        let (extract_progress_tx, mut extract_progress_rx) = mpsc::channel(32);
        let stage_tx_1 = stage_tx.clone();
        tokio::spawn(async move {
            while let Some(p) = extract_progress_rx.recv().await {
                let _ = stage_tx_1
                    .send(PipelineStage::ExtractingAudio { percent: p.percent })
                    .await;
            }
        });

        let audio_path = AudioExtractor::extract(
            input_path,
            &self.tmp_dir,
            &ExtractConfig::default(),
            extract_progress_tx,
        )
        .await
        .map_err(AppError::Audio)?;

        // Check cancellation
        if *cancel_rx.borrow() {
            let _ = stage_tx.send(PipelineStage::Cancelled).await;
            return Err(AppError::Pipeline("Cancelled".into()));
        }

        // =============== Stage 2: Speech Recognition ===============
        tracing::info!("Stage 2: Transcribing audio {:?}", audio_path);

        let (asr_progress_tx, mut asr_progress_rx) = mpsc::channel(32);
        let stage_tx_2 = stage_tx.clone();
        tokio::spawn(async move {
            while let Some(p) = asr_progress_rx.recv().await {
                let _ = stage_tx_2
                    .send(PipelineStage::Transcribing {
                        percent: p.percent,
                        current_text: p.current_segment.map(|s| s.text),
                    })
                    .await;
            }
        });

        let asr_config = AsrConfig {
            model_path: PathBuf::from(&config.asr_model),
            language: config.source_language.clone(),
            translate_to_english: false,
            n_threads: num_cpus::get() as u32,
            use_gpu: config.use_gpu,
            ..Default::default()
        };

        let segments = self
            .asr_engine
            .transcribe(&audio_path, &asr_config, asr_progress_tx)
            .await
            .map_err(AppError::Asr)?;

        tracing::info!("ASR complete: {} segments", segments.len());

        if segments.is_empty() {
            return Err(AppError::Pipeline("No speech detected in audio".into()));
        }

        // Check cancellation
        if *cancel_rx.borrow() {
            let _ = stage_tx.send(PipelineStage::Cancelled).await;
            return Err(AppError::Pipeline("Cancelled".into()));
        }

        // Detect source language from first segment
        let source_lang = config
            .source_language
            .as_deref()
            .unwrap_or_else(|| &segments[0].language);

        // =============== Stage 3: Translation ===============
        tracing::info!(
            "Stage 3: Translating {} segments ({} -> {})",
            segments.len(),
            source_lang,
            config.target_language
        );

        let (trans_progress_tx, mut trans_progress_rx) = mpsc::channel(32);
        let stage_tx_3 = stage_tx.clone();
        tokio::spawn(async move {
            while let Some(p) = trans_progress_rx.recv().await {
                let _ = stage_tx_3
                    .send(PipelineStage::Translating {
                        percent: p.percent,
                        translated_count: p.translated_count,
                        total_count: p.total_count,
                    })
                    .await;
            }
        });

        let translate_request = TranslateRequest {
            texts: segments.iter().map(|s| s.text.clone()).collect(),
            source_lang: source_lang.to_string(),
            target_lang: config.target_language.clone(),
            context_hint: None,
        };

        let translation = self
            .translate_engine
            .translate(&translate_request, trans_progress_tx)
            .await
            .map_err(AppError::Translate)?;

        // Check cancellation
        if *cancel_rx.borrow() {
            let _ = stage_tx.send(PipelineStage::Cancelled).await;
            return Err(AppError::Pipeline("Cancelled".into()));
        }

        // =============== Stage 4: Generate Subtitle File ===============
        tracing::info!("Stage 4: Generating {:?} subtitle", config.output_format);
        let _ = stage_tx.send(PipelineStage::GeneratingSubtitle).await;

        let subtitle = SubtitleMerger::merge(
            &segments,
            &translation,
            source_lang,
            &config.target_language,
            config.output_format,
        )
        .map_err(AppError::Subtitle)?;

        // Generate output content
        let output_content = match config.output_format {
            SubtitleFormat::Srt => SrtWriter::write(&subtitle),
            SubtitleFormat::Ass => AssWriter::write(&subtitle, &AssStyle::default()),
            SubtitleFormat::Vtt => VttWriter::write(&subtitle),
        };

        // Build output filename
        let input_stem = input_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        let output_filename = format!(
            "{}.{}-{}.{}",
            input_stem,
            source_lang,
            config.target_language,
            config.output_format.extension()
        );
        let output_path = output_dir.join(&output_filename);

        // Ensure output directory exists
        tokio::fs::create_dir_all(output_dir).await?;

        // Write with BOM for SRT/ASS (better compatibility with Asian text)
        let content_with_bom = format!("\u{FEFF}{}", output_content);
        tokio::fs::write(&output_path, content_with_bom.as_bytes()).await?;

        let output_str = output_path.to_string_lossy().to_string();
        let duration_ms = segments.last().map(|s| s.end_ms).unwrap_or(0);

        tracing::info!("Pipeline complete! Output: {}", output_str);

        let _ = stage_tx
            .send(PipelineStage::Completed {
                output_path: output_str.clone(),
                segment_count: segments.len(),
                duration_ms,
            })
            .await;

        // Cleanup temp audio file
        if let Err(e) = tokio::fs::remove_file(&audio_path).await {
            tracing::warn!("Failed to cleanup temp audio: {}", e);
        }

        Ok(output_str)
    }
}
