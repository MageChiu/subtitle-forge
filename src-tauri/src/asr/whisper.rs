use super::engine::*;
use crate::error::AsrError;
use async_trait::async_trait;
use std::io::Read as IoRead;
use std::path::Path;
use tokio::sync::mpsc;

struct StderrGuard {
    saved_fd: Option<i32>,
}

impl StderrGuard {
    fn suppress() -> Self {
        unsafe {
            let saved_fd = libc::dup(libc::STDERR_FILENO);
            if saved_fd >= 0 {
                let devnull = libc::open(b"/dev/null\0".as_ptr() as *const i8, libc::O_WRONLY);
                if devnull >= 0 {
                    libc::dup2(devnull, libc::STDERR_FILENO);
                    libc::close(devnull);
                    Self { saved_fd: Some(saved_fd) }
                } else {
                    libc::close(saved_fd);
                    Self { saved_fd: None }
                }
            } else {
                Self { saved_fd: None }
            }
        }
    }
}

impl Drop for StderrGuard {
    fn drop(&mut self) {
        if let Some(fd) = self.saved_fd {
            unsafe {
                libc::dup2(fd, libc::STDERR_FILENO);
                libc::close(fd);
            }
        }
    }
}

const CHUNK_DURATION_SECS: usize = 300;
const SAMPLE_RATE: u32 = 16000;
const CHUNK_SAMPLES: usize = CHUNK_DURATION_SECS * (SAMPLE_RATE as usize);

pub struct WhisperEngine {
    model_path: std::path::PathBuf,
    use_gpu: bool,
}

impl WhisperEngine {
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
            LanguageInfo { code: "it".into(), name: "Italiano".into() },
            LanguageInfo { code: "ar".into(), name: "Arabic".into() },
            LanguageInfo { code: "hi".into(), name: "Hindi".into() },
            LanguageInfo { code: "th".into(), name: "Thai".into() },
            LanguageInfo { code: "vi".into(), name: "Vietnamese".into() },
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

        if !model_path.exists() {
            return Err(AsrError::ModelLoad(format!(
                "Whisper model not found: {:?}. Please download it in Settings > Models",
                model_path
            )));
        }

        if !audio_path.exists() {
            return Err(AsrError::Transcription(format!(
                "Audio file not found: {:?}",
                audio_path
            )));
        }

        tracing::info!("Loading Whisper model...");
        let mut ctx_params = whisper_rs::WhisperContextParameters::new();
        ctx_params.use_gpu(use_gpu);

        let _stderr_guard = StderrGuard::suppress();
        let ctx = whisper_rs::WhisperContext::new_with_params(
            model_path.to_str().ok_or_else(|| AsrError::ModelLoad("Invalid model path".into()))?,
            ctx_params,
        )
        .map_err(|e| AsrError::ModelLoad(format!("Failed to load model: {}", e)))?;
        drop(_stderr_guard);

        tracing::info!("Model loaded, reading audio samples...");
        let samples = Self::read_wav_samples(audio_path)?;
        let total_duration_ms = samples.len() as u64 * 1000 / SAMPLE_RATE as u64;
        tracing::info!(
            "Read {} samples ({}ms / {:.1}min at {}Hz)",
            samples.len(),
            total_duration_ms,
            total_duration_ms as f64 / 60000.0,
            SAMPLE_RATE
        );

        let num_chunks = (samples.len() + CHUNK_SAMPLES - 1) / CHUNK_SAMPLES;
        tracing::info!(
            "Will process in {} chunk(s) of {}s each (total {:.1}min)",
            num_chunks,
            CHUNK_DURATION_SECS,
            total_duration_ms as f64 / 60000.0
        );

        let _ = progress_tx.try_send(AsrProgress {
            percent: 0.0,
            current_segment: None,
        });

        let mut all_segments: Vec<Segment> = Vec::new();
        let mut segment_index: usize = 0;
        let mut detected_lang: Option<String> = None;

        for chunk_idx in 0..num_chunks {
            let start = chunk_idx * CHUNK_SAMPLES;
            let end = std::cmp::min(start + CHUNK_SAMPLES, samples.len());
            let chunk_samples = &samples[start..end];
            let chunk_offset_ms = (start as u64 * 1000) / SAMPLE_RATE as u64;

            tracing::info!(
                "Processing chunk {}/{} (offset {:.1}s, {} samples)...",
                chunk_idx + 1,
                num_chunks,
                chunk_offset_ms as f64 / 1000.0,
                chunk_samples.len()
            );

            let _stderr_guard = StderrGuard::suppress();
            let mut state = ctx.create_state()
                .map_err(|e| AsrError::Transcription(format!("Failed to create state: {}", e)))?;

            let mut params = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });

            if let Some(ref lang) = config.language {
                if lang != "auto" {
                    params.set_language(Some(lang));
                }
            }
            params.set_translate(config.translate_to_english);
            params.set_n_threads(config.n_threads as i32);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_print_special(false);
            params.set_token_timestamps(true);
            params.set_no_timestamps(false);
            params.set_suppress_blank(true);
            params.set_suppress_non_speech_tokens(true);

            state.full(params, chunk_samples)
                .map_err(|e| AsrError::Transcription(format!(
                    "Transcription failed at chunk {}: {}", chunk_idx + 1, e
                )))?;
            drop(_stderr_guard);

            let num_segments = state.full_n_segments()
                .map_err(|e| AsrError::Transcription(format!("Failed to get segment count: {}", e)))?;

            if detected_lang.is_none() && num_segments > 0 {
                detected_lang = state.full_lang_id_from_state()
                    .ok()
                    .and_then(|id| whisper_rs::get_lang_str(id).map(String::from));
            }

            for i in 0..num_segments {
                let seg_start_ms = (state.full_get_segment_t0(i)
                    .map_err(|e| AsrError::Transcription(format!("Failed to get segment start: {}", e)))?
                    * 10) as u64;
                let seg_end_ms = (state.full_get_segment_t1(i)
                    .map_err(|e| AsrError::Transcription(format!("Failed to get segment end: {}", e)))?
                    * 10) as u64;
                let text = state.full_get_segment_text_lossy(i)
                    .map_err(|e| AsrError::Transcription(format!("Failed to get segment text: {}", e)))?;

                let trimmed = text.trim().to_string();
                if trimmed.is_empty() || trimmed == "[Pause]" {
                    continue;
                }

                all_segments.push(Segment {
                    index: segment_index,
                    start_ms: chunk_offset_ms + seg_start_ms,
                    end_ms: chunk_offset_ms + seg_end_ms,
                    text: trimmed,
                    language: detected_lang.clone().unwrap_or_else(|| "unknown".to_string()),
                    confidence: 1.0,
                });
                segment_index += 1;
            }

            let percent = ((chunk_idx + 1) as f32 / num_chunks as f32) * 100.0;
            tracing::info!(
                "Chunk {}/{} done: {} segments so far ({:.1}%)",
                chunk_idx + 1,
                num_chunks,
                all_segments.len(),
                percent
            );
            let _ = progress_tx.try_send(AsrProgress {
                percent,
                current_segment: all_segments.last().cloned(),
            });
        }

        let lang = detected_lang.unwrap_or_else(|| "unknown".to_string());
        tracing::info!(
            "Transcription complete: {} segments, language: {}, duration: {:.1}min",
            all_segments.len(),
            lang,
            total_duration_ms as f64 / 60000.0
        );

        let _ = progress_tx.try_send(AsrProgress {
            percent: 100.0,
            current_segment: None,
        });

        Ok(all_segments)
    }

    fn read_wav_samples(path: &Path) -> Result<Vec<f32>, AsrError> {
        let mut file = std::fs::File::open(path)
            .map_err(|e| AsrError::Transcription(format!("Failed to open WAV file: {}", e)))?;

        let mut header = [0u8; 44];
        file.read_exact(&mut header)
            .map_err(|e| AsrError::Transcription(format!("Failed to read WAV header: {}", e)))?;

        if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
            return Err(AsrError::Transcription("Not a valid WAV file".into()));
        }

        let audio_format = u16::from_le_bytes([header[20], header[21]]);
        let num_channels = u16::from_le_bytes([header[22], header[23]]) as usize;
        let sample_rate = u32::from_le_bytes([header[24], header[25], header[26], header[27]]);
        let bits_per_sample = u16::from_le_bytes([header[34], header[35]]) as usize;

        tracing::info!(
            "WAV: format={}, channels={}, rate={}, bits={}",
            audio_format, num_channels, sample_rate, bits_per_sample
        );

        if audio_format != 1 {
            return Err(AsrError::Transcription(
                format!("Unsupported WAV format: {} (only PCM supported)", audio_format)
            ));
        }

        let data_size = u32::from_le_bytes([header[40], header[41], header[42], header[43]]) as usize;
        let _bytes_per_sample = bits_per_sample / 8;

        let mut raw_data = vec![0u8; data_size];
        file.read_exact(&mut raw_data)
            .map_err(|e| AsrError::Transcription(format!("Failed to read WAV data: {}", e)))?;

        let samples: Vec<f32> = match bits_per_sample {
            16 => raw_data
                .chunks_exact(2)
                .map(|chunk| {
                    let val = i16::from_le_bytes([chunk[0], chunk[1]]);
                    val as f32 / 32768.0
                })
                .collect(),
            32 if sample_rate != 3 => raw_data
                .chunks_exact(4)
                .map(|chunk| {
                    let val = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    val
                })
                .collect(),
            _ => {
                return Err(AsrError::Transcription(
                    format!("Unsupported bits per sample: {}", bits_per_sample)
                ));
            }
        };

        let mono_samples = if num_channels > 1 {
            samples
                .chunks_exact(num_channels)
                .map(|ch| ch.iter().sum::<f32>() / num_channels as f32)
                .collect()
        } else {
            samples
        };

        let final_samples = if sample_rate != 16000 {
            tracing::info!("Resampling from {}Hz to 16000Hz...", sample_rate);
            simple_resample(&mono_samples, sample_rate, 16000)
        } else {
            mono_samples
        };

        Ok(final_samples)
    }
}

fn simple_resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);
    for i in 0..output_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos as usize;
        let frac = src_pos - idx as f64;
        if idx + 1 < samples.len() {
            output.push((samples[idx] as f64 * (1.0 - frac) + samples[idx + 1] as f64 * frac) as f32);
        } else if idx < samples.len() {
            output.push(samples[idx]);
        }
    }
    output
}
