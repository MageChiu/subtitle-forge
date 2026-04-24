// ============================================================
// pipeline/orchestrator.rs — Full pipeline orchestration
// ============================================================

use crate::audio::extractor::{AudioExtractor, ExtractConfig};
use crate::asr::engine::{AsrConfig, AsrEngine};
use crate::config::settings::recommended_asr_threads;
use crate::error::AppError;
use crate::subtitle::ass::{AssStyle, AssWriter};
use crate::subtitle::merger::SubtitleMerger;
use crate::subtitle::srt::SrtWriter;
use crate::subtitle::types::*;
use crate::subtitle::vtt::VttWriter;
use crate::translate::engine::{TranslateEngine, TranslateRequest};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::mpsc;

/// Pipeline task configuration (from frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub input_path: String,
    pub output_dir: String,
    pub source_language: Option<String>,
    pub target_language: String,
    pub output_format: SubtitleFormat,
    pub asr_model: String,
    pub translate_engine: String,
    pub use_gpu: bool,
    pub n_threads: Option<u32>,
    pub skip_translation: bool,
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
        source_output_path: String,
        bilingual_output_path: Option<String>,
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
    cache_dir: PathBuf,
}

impl PipelineOrchestrator {
    pub fn new(
        asr_engine: Box<dyn AsrEngine>,
        translate_engine: Box<dyn TranslateEngine>,
        tmp_dir: PathBuf,
        cache_dir: PathBuf,
    ) -> Self {
        Self {
            asr_engine,
            translate_engine,
            tmp_dir,
            cache_dir,
        }
    }

    /// Execute the full pipeline
    pub async fn run(
        &self,
        config: PipelineConfig,
        stage_tx: mpsc::Sender<PipelineStage>,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<String, AppError> {
        match self.run_inner(config, stage_tx.clone(), cancel_rx).await {
            Ok(output) => Ok(output),
            Err(e) => {
                if !matches!(&e, AppError::Pipeline(message) if message == "Cancelled") {
                    let _ = stage_tx
                        .send(PipelineStage::Failed {
                            error: e.to_string(),
                        })
                        .await;
                }
                Err(e)
            }
        }
    }

    async fn run_inner(
        &self,
        config: PipelineConfig,
        stage_tx: mpsc::Sender<PipelineStage>,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<String, AppError> {
        let input_path = Path::new(&config.input_path);
        let output_dir = Path::new(&config.output_dir);
        let input_stem = input_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // =============== Stage 1: Audio Extraction ===============
        tracing::info!("Stage 1: Extracting audio from {:?}", input_path);

        let (extract_progress_tx, mut extract_progress_rx) = mpsc::channel::<crate::audio::extractor::ExtractProgress>(32);
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

        let (asr_progress_tx, mut asr_progress_rx) = mpsc::channel::<crate::asr::engine::AsrProgress>(32);
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

        let asr_cancel_flag = Arc::new(AtomicBool::new(false));
        let asr_cancel_flag_task = asr_cancel_flag.clone();
        let mut asr_cancel_rx = cancel_rx.clone();
        tokio::spawn(async move {
            loop {
                if *asr_cancel_rx.borrow() {
                    asr_cancel_flag_task.store(true, Ordering::Relaxed);
                    break;
                }
                if asr_cancel_rx.changed().await.is_err() {
                    break;
                }
            }
        });

        let asr_config = AsrConfig {
            model_path: PathBuf::from(&config.asr_model),
            language: config.source_language.clone(),
            translate_to_english: false,
            n_threads: config
                .n_threads
                .filter(|threads| *threads > 0)
                .unwrap_or_else(recommended_asr_threads),
            use_gpu: config.use_gpu,
            debug_output_dir: if cfg!(debug_assertions) {
                Some(self.cache_dir.clone())
            } else {
                None
            },
            ..Default::default()
        };

        let segments = match self
            .asr_engine
            .transcribe(&audio_path, &asr_config, asr_progress_tx, asr_cancel_flag.clone())
            .await
        {
            Ok(segments) => segments,
            Err(_e) if asr_cancel_flag.load(Ordering::Relaxed) => {
                let _ = stage_tx.send(PipelineStage::Cancelled).await;
                return Err(AppError::Pipeline("Cancelled".into()));
            }
            Err(e) => return Err(AppError::Asr(e)),
        };

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

        // =============== Stage 3: Generate source-language subtitle first ===============
        tracing::info!(
            "Stage 3: Generating source-language subtitle first ({} segments, lang={})",
            segments.len(),
            source_lang
        );
        let _ = stage_tx.send(PipelineStage::GeneratingSubtitle).await;

        let source_subtitle =
            SubtitleMerger::from_segments(&segments, source_lang, config.output_format);
        let source_output_filename = format!(
            "{}.{}.{}",
            input_stem,
            source_lang,
            config.output_format.extension()
        );
        let source_output_path = output_dir.join(&source_output_filename);
        self.write_subtitle_file(output_dir, &source_output_path, &source_subtitle, config.output_format)
            .await?;
        tracing::info!(
            "Source-language subtitle generated: {}",
            source_output_path.to_string_lossy()
        );

        if *cancel_rx.borrow() {
            let _ = stage_tx.send(PipelineStage::Cancelled).await;
            return Err(AppError::Pipeline("Cancelled".into()));
        }

        // =============== Stage 4: Translation (optional) ===============
        let bilingual_output_path = if config.skip_translation {
            tracing::info!("Stage 4: Skipping translation, keeping source-language subtitle only");
            None
        } else {
            tracing::info!(
                "Stage 4: Translating {} segments ({} -> {})",
                segments.len(),
                source_lang,
                config.target_language
            );

            let (trans_progress_tx, mut trans_progress_rx) = mpsc::channel::<crate::translate::engine::TranslateProgress>(32);
            let stage_tx_3 = stage_tx.clone();
            tokio::spawn(async move {
                while let Some(p) = trans_progress_rx.recv().await {
                    tracing::info!(
                        "Translation progress: {}/{} ({:.1}%)",
                        p.translated_count,
                        p.total_count,
                        p.percent
                    );
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

            let bilingual_subtitle = SubtitleMerger::merge(
                &segments,
                &translation,
                source_lang,
                &config.target_language,
                config.output_format,
            )
            .map_err(AppError::Subtitle)?;

            if *cancel_rx.borrow() {
                let _ = stage_tx.send(PipelineStage::Cancelled).await;
                return Err(AppError::Pipeline("Cancelled".into()));
            }

            // =============== Stage 5: Generate bilingual subtitle ===============
            tracing::info!("Stage 5: Generating bilingual subtitle");
            let _ = stage_tx.send(PipelineStage::GeneratingSubtitle).await;

            let bilingual_output_filename = format!(
                "{}.{}-{}.{}",
                input_stem,
                source_lang,
                config.target_language,
                config.output_format.extension()
            );
            let bilingual_output_path = output_dir.join(&bilingual_output_filename);
            self.write_subtitle_file(
                output_dir,
                &bilingual_output_path,
                &bilingual_subtitle,
                config.output_format,
            )
            .await?;
            tracing::info!(
                "Bilingual subtitle generated: {}",
                bilingual_output_path.to_string_lossy()
            );
            Some(bilingual_output_path)
        };
        let final_output_path = bilingual_output_path
            .as_ref()
            .unwrap_or(&source_output_path);
        let output_str = final_output_path.to_string_lossy().to_string();
        let duration_ms = segments.last().map(|s| s.end_ms).unwrap_or(0);

        tracing::info!("Pipeline complete! Output: {}", output_str);

        let _ = stage_tx
            .send(PipelineStage::Completed {
                output_path: output_str.clone(),
                source_output_path: source_output_path.to_string_lossy().to_string(),
                bilingual_output_path: bilingual_output_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string()),
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

    async fn write_subtitle_file(
        &self,
        output_dir: &Path,
        output_path: &Path,
        subtitle: &SubtitleFile,
        format: SubtitleFormat,
    ) -> Result<(), AppError> {
        let output_content = match format {
            SubtitleFormat::Srt => SrtWriter::write(subtitle),
            SubtitleFormat::Ass => AssWriter::write(subtitle, &AssStyle::default()),
            SubtitleFormat::Vtt => VttWriter::write(subtitle),
        };

        tokio::fs::create_dir_all(output_dir).await?;
        let content_with_bom = format!("\u{FEFF}{}", output_content);
        tokio::fs::write(output_path, content_with_bom.as_bytes()).await?;
        Ok(())
    }
}
