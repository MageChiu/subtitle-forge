// ============================================================
// asr/whisper.rs — whisper.cpp implementation via whisper-rs
// ============================================================

use super::engine::*;
use crate::error::AsrError;
use async_trait::async_trait;
use std::path::Path;
use tokio::sync::mpsc;

/// Whisper ASR engine backed by whisper.cpp
pub struct WhisperEngine {
    model_path: std::path::PathBuf,
    use_gpu: bool,
}

impl WhisperEngine {
    /// Create a new WhisperEngine
    pub fn new(model_path: std::path::PathBuf, use_gpu: bool) -> Self {
        Self { model_path, use_gpu }
    }
}

#[async_trait]
impl AsrEngine for WhisperEngine {
    async fn transcribe(
        &self,
        audio_path: &Path,
        config: &AsrConfig,
        progress_tx: mpsc::Sender<AsrProgress>,
    ) -> Result<Vec<Segment>, AsrError> {
        let audio_path = audio_path.to_path_buf();
        let model_path = self.model_path.clone();
        let use_gpu = self.use_gpu;
        let config = config.clone();

        tokio::task::spawn_blocking(move || {
            Self::transcribe_sync(&audio_path, &model_path, use_gpu, &config, progress_tx)
        })
        .await
        .map_err(|e| AsrError::Transcription(format!("Task join error: {}", e)))?
    }

    fn supported_languages(&self) -> Vec<LanguageInfo> {
        vec![
            LanguageInfo { code: "auto".into(), name: "Auto Detect".into() },
            LanguageInfo { code: "en".into(), name: "English".into() },
            LanguageInfo { code: "zh".into(), name: "Chinese".into() },
            LanguageInfo { code: "ja".into(), name: "Japanese".into() },
            LanguageInfo { code: "ko".into(), name: "Korean".into() },
            LanguageInfo { code: "fr".into(), name: "French".into() },
            LanguageInfo { code: "de".into(), name: "German".into() },
            LanguageInfo { code: "es".into(), name: "Spanish".into() },
            LanguageInfo { code: "ru".into(), name: "Russian".into() },
            LanguageInfo { code: "pt".into(), name: "Portuguese".into() },
            LanguageInfo { code: "it".into(), name: "Italian".into() },
            LanguageInfo { code: "ar".into(), name: "Arabic".into() },
            LanguageInfo { code: "hi".into(), name: "Hindi".into() },
            LanguageInfo { code: "th".into(), name: "Thai".into() },
            LanguageInfo { code: "vi".into(), name: "Vietnamese".into() },
            // Whisper supports 99 languages total
        ]
    }

    fn name(&self) -> &str {
        "whisper.cpp"
    }
}

impl WhisperEngine {
    fn transcribe_sync(
        audio_path: &Path,
        model_path: &Path,
        use_gpu: bool,
        config: &AsrConfig,
        progress_tx: mpsc::Sender<AsrProgress>,
    ) -> Result<Vec<Segment>, AsrError> {
        tracing::info!(
            "Starting Whisper transcription: audio={:?}, model={:?}, gpu={}",
            audio_path, model_path, use_gpu
        );

        // Implementation using whisper-rs:
        //
        // // 1. Load model
        // let mut ctx_params = WhisperContextParameters::default();
        // ctx_params.use_gpu(use_gpu);
        // let ctx = WhisperContext::new_with_params(
        //     model_path.to_str().unwrap(), ctx_params
        // ).map_err(|e| AsrError::ModelLoad(e.to_string()))?;
        //
        // // 2. Read audio samples (16kHz f32 mono)
        // let samples = Self::read_wav_samples(audio_path)?;
        //
        // // 3. Create inference state
        // let mut state = ctx.create_state()
        //     .map_err(|e| AsrError::Transcription(e.to_string()))?;
        //
        // // 4. Configure parameters
        // let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        // if let Some(ref lang) = config.language {
        //     params.set_language(Some(lang));
        // }
        // params.set_translate(config.translate_to_english);
        // params.set_n_threads(config.n_threads as i32);
        // params.set_print_progress(false);
        // params.set_print_realtime(false);
        // params.set_token_timestamps(true);
        //
        // // Set progress callback
        // params.set_progress_callback_safe(|progress| {
        //     let _ = progress_tx.blocking_send(AsrProgress {
        //         percent: progress as f32,
        //         current_segment: None,
        //     });
        // });
        //
        // // 5. Run inference
        // state.full(params, &samples)
        //     .map_err(|e| AsrError::Transcription(e.to_string()))?;
        //
        // // 6. Collect segments
        // let num_segments = state.full_n_segments()
        //     .map_err(|e| AsrError::Transcription(e.to_string()))?;
        //
        // let mut segments = Vec::with_capacity(num_segments as usize);
        // for i in 0..num_segments {
        //     let start_ms = (state.full_get_segment_t0(i)? * 10) as u64;
        //     let end_ms = (state.full_get_segment_t1(i)? * 10) as u64;
        //     let text = state.full_get_segment_text(i)?;
        //     let lang = state.full_lang_id()
        //         .map(|id| whisper_rs::get_lang_str(id).to_string())
        //         .unwrap_or_else(|_| "unknown".to_string());
        //
        //     segments.push(Segment {
        //         index: i as usize,
        //         start_ms,
        //         end_ms,
        //         text: text.trim().to_string(),
        //         language: lang,
        //         confidence: 1.0, // whisper.cpp doesn't expose per-segment confidence easily
        //     });
        //
        //     let _ = progress_tx.blocking_send(AsrProgress {
        //         percent: ((i + 1) as f32 / num_segments as f32) * 100.0,
        //         current_segment: Some(segments.last().unwrap().clone()),
        //     });
        // }
        //
        // Ok(segments)

        todo!("Implement with whisper-rs")
    }

    /// Read WAV file and return f32 samples at 16kHz mono
    #[allow(dead_code)]
    fn read_wav_samples(path: &Path) -> Result<Vec<f32>, AsrError> {
        // Use hound or manual WAV parsing:
        //
        // let reader = hound::WavReader::open(path)
        //     .map_err(|e| AsrError::Transcription(format!("WAV read error: {}", e)))?;
        // let spec = reader.spec();
        // let samples: Vec<f32> = match spec.sample_format {
        //     hound::SampleFormat::Float => reader.into_samples::<f32>()
        //         .filter_map(Result::ok).collect(),
        //     hound::SampleFormat::Int => reader.into_samples::<i16>()
        //         .filter_map(Result::ok)
        //         .map(|s| s as f32 / 32768.0)
        //         .collect(),
        // };
        // Ok(samples)

        todo!("Implement WAV reader")
    }
}
