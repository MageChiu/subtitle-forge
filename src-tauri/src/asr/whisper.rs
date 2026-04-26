use super::engine::*;
use super::capabilities::{
    coreml_enabled, cuda_enabled, gpu_backend_available, metal_enabled, openblas_enabled,
};
use super::vad::{PlannedChunk, VadPlanner, VadPlannerConfig};
use crate::error::AsrError;
use async_trait::async_trait;
use serde::Serialize;
use std::io::Read as IoRead;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use tokio::sync::mpsc;

struct StderrGuard {
    #[cfg(unix)]
    saved_fd: Option<i32>,
}

impl StderrGuard {
    fn suppress() -> Self {
        #[cfg(unix)]
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

        #[cfg(not(unix))]
        {
            Self {}
        }
    }
}

impl Drop for StderrGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(fd) = self.saved_fd {
            unsafe {
                libc::dup2(fd, libc::STDERR_FILENO);
                libc::close(fd);
            }
        }
    }
}

const SAMPLE_RATE: u32 = 16000;

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
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<Vec<Segment>, AsrError> {
        let audio_path = audio_path.to_path_buf();
        let model_path = self.model_path.clone();
        let use_gpu = self.use_gpu;
        let config = config.clone();
        let cancel_flag = cancel_flag.clone();

        tokio::task::spawn_blocking(move || {
            Self::transcribe_sync(
                &audio_path,
                &model_path,
                use_gpu,
                &config,
                progress_tx,
                cancel_flag,
            )
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
    unsafe extern "C" fn abort_callback(user_data: *mut std::ffi::c_void) -> bool {
        let flag = &*(user_data as *const AtomicBool);
        flag.load(Ordering::Relaxed)
    }
    fn transcribe_sync(
        audio_path: &Path,
        model_path: &Path,
        use_gpu: bool,
        config: &AsrConfig,
        progress_tx: mpsc::Sender<AsrProgress>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<Vec<Segment>, AsrError> {
        tracing::info!(
            "Starting Whisper transcription: audio={:?}, model={:?}, gpu={}",
            audio_path, model_path, use_gpu
        );
        tracing::info!(
            "Whisper backend features: metal={}, coreml={}, openblas={}, cuda={}",
            metal_enabled(),
            coreml_enabled(),
            openblas_enabled(),
            cuda_enabled()
        );
        if use_gpu && !gpu_backend_available() {
            tracing::warn!("GPU acceleration requested, but no GPU-capable Whisper backend is compiled in");
        }

        Self::ensure_not_cancelled(&cancel_flag)?;

        if !model_path.exists() {
            return Err(AsrError::ModelLoad(format!(
                "Whisper model not found: {:?}. Please download it in Settings > Models",
                model_path
            )));
        }

        Self::ensure_not_cancelled(&cancel_flag)?;

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
        Self::ensure_not_cancelled(&cancel_flag)?;
        let total_duration_ms = samples.len() as u64 * 1000 / SAMPLE_RATE as u64;
        tracing::info!(
            "Read {} samples ({}ms / {:.1}min at {}Hz)",
            samples.len(),
            total_duration_ms,
            total_duration_ms as f64 / 60000.0,
            SAMPLE_RATE
        );

        let vad_config = VadPlannerConfig {
            sample_rate: SAMPLE_RATE,
            min_speech_ms: config.vad_min_speech_ms,
            min_silence_ms: config.vad_min_silence_ms,
            hangover_ms: config.vad_hangover_ms,
            max_merge_gap_ms: config.vad_max_merge_gap_ms,
            min_chunk_ms: config.vad_min_chunk_ms,
            max_chunk_ms: config.vad_max_chunk_ms,
            overlap_ms: config.vad_overlap_ms,
            threshold_probability: config.vad_threshold_probability,
            energy_frame_ms: 20,
            energy_threshold_floor: 0.008,
            energy_threshold_ratio: 0.18,
        };

        let planned_chunks = if config.enable_vad {
            let raw_regions = VadPlanner::detect_regions(&samples, &vad_config)?;
            let merged_regions = VadPlanner::merge_regions(&raw_regions, &vad_config);
            let energy_regions = VadPlanner::detect_energy_regions(&samples, &vad_config);
            let effective_regions = VadPlanner::intersect_regions(
                &merged_regions,
                &energy_regions,
                (((vad_config.energy_frame_ms as u64) * SAMPLE_RATE as u64) / 1000) as usize,
            );
            let split_regions =
                VadPlanner::split_long_regions_by_energy(&effective_regions, &samples, &vad_config);
            let speech_samples: usize = merged_regions
                .iter()
                .map(|region| region.end_sample.saturating_sub(region.start_sample))
                .sum();
            let speech_ratio = if samples.is_empty() {
                0.0
            } else {
                speech_samples as f64 / samples.len() as f64
            };
            let longest_merged_secs = merged_regions
                .iter()
                .map(|region| region.end_sample.saturating_sub(region.start_sample))
                .max()
                .unwrap_or(0) as f64
                / SAMPLE_RATE as f64;
            let longest_effective_secs = effective_regions
                .iter()
                .map(|region| region.end_sample.saturating_sub(region.start_sample))
                .max()
                .unwrap_or(0) as f64
                / SAMPLE_RATE as f64;
            let longest_split_secs = split_regions
                .iter()
                .map(|region| region.end_sample.saturating_sub(region.start_sample))
                .max()
                .unwrap_or(0) as f64
                / SAMPLE_RATE as f64;
            let planned_chunks =
                VadPlanner::plan_chunks_from_split_regions(&split_regions, samples.len(), &vad_config);
            tracing::info!(
                "VAD planned {} raw -> {} merged -> {} energy -> {} effective -> {} split -> {} chunk(s), speech coverage {:.1}%, longest merged {:.1}s, longest effective {:.1}s, longest split {:.1}s",
                raw_regions.len(),
                merged_regions.len(),
                energy_regions.len(),
                effective_regions.len(),
                split_regions.len(),
                planned_chunks.len(),
                speech_ratio * 100.0,
                longest_merged_secs,
                longest_effective_secs,
                longest_split_secs,
            );
            Self::write_debug_vad_artifacts(
                audio_path,
                config.debug_output_dir.as_deref(),
                &raw_regions,
                &merged_regions,
                &energy_regions,
                &effective_regions,
                &split_regions,
                &planned_chunks,
                total_duration_ms,
            );
            if planned_chunks.is_empty() {
                tracing::warn!(
                    "VAD/energy gating produced no chunks; falling back to fixed chunk planning"
                );
                VadPlanner::plan_fixed_chunks(samples.len(), &vad_config)
            } else {
                planned_chunks
            }
        } else {
            let planned_chunks = VadPlanner::plan_fixed_chunks(samples.len(), &vad_config);
            tracing::info!(
                "VAD disabled, falling back to {} fixed chunk(s) of up to {:.1}s with {:.1}s overlap",
                planned_chunks.len(),
                vad_config.max_chunk_ms as f64 / 1000.0,
                vad_config.overlap_ms as f64 / 1000.0
            );
            planned_chunks
        };
        let num_chunks = planned_chunks.len();
        tracing::info!(
            "Will process {} planned chunk(s) (total {:.1}min)",
            num_chunks,
            total_duration_ms as f64 / 60000.0
        );

        if num_chunks == 0 {
            tracing::warn!("No chunks available for ASR after planning");
            let _ = progress_tx.try_send(AsrProgress {
                percent: 100.0,
                current_segment: None,
            });
            return Ok(Vec::new());
        }

        let _ = progress_tx.try_send(AsrProgress {
            percent: 0.0,
            current_segment: None,
        });

        let (mut all_segments, mut detected_lang) = Self::transcribe_chunks(
            &ctx,
            &samples,
            &planned_chunks,
            config,
            &progress_tx,
            &cancel_flag,
        )?;

        if all_segments.is_empty() && config.enable_vad {
            let fallback_chunks = VadPlanner::plan_fixed_chunks(samples.len(), &vad_config);
            if !fallback_chunks.is_empty() {
                tracing::warn!(
                    "VAD-based transcription produced no segments; retrying with fixed chunk planning"
                );
                let _ = progress_tx.try_send(AsrProgress {
                    percent: 0.0,
                    current_segment: None,
                });
                let (fallback_segments, fallback_lang) = Self::transcribe_chunks(
                    &ctx,
                    &samples,
                    &fallback_chunks,
                    config,
                    &progress_tx,
                    &cancel_flag,
                )?;
                all_segments = fallback_segments;
                detected_lang = fallback_lang;
            }
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

    fn format_duration(seconds: f64) -> String {
        let total = seconds.max(0.0).round() as u64;
        let hours = total / 3600;
        let minutes = (total % 3600) / 60;
        let secs = total % 60;
        if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, minutes, secs)
        } else {
            format!("{:02}:{:02}", minutes, secs)
        }
    }

    fn transcribe_chunks(
        ctx: &whisper_rs::WhisperContext,
        samples: &[f32],
        planned_chunks: &[PlannedChunk],
        config: &AsrConfig,
        progress_tx: &mpsc::Sender<AsrProgress>,
        cancel_flag: &Arc<AtomicBool>,
    ) -> Result<(Vec<Segment>, Option<String>), AsrError> {
        let num_chunks = planned_chunks.len();
        let mut all_segments: Vec<Segment> = Vec::new();
        let mut segment_index: usize = 0;
        let mut detected_lang: Option<String> = None;
        let transcription_start = Instant::now();
        let _stderr_guard = StderrGuard::suppress();
        let mut state = ctx.create_state()
            .map_err(|e| AsrError::Transcription(format!("Failed to create state: {}", e)))?;

        for (chunk_idx, chunk) in planned_chunks.iter().enumerate() {
            Self::ensure_not_cancelled(cancel_flag)?;
            let main_start = chunk.main_start_sample;
            let main_end = chunk.main_end_sample;
            let window_start = chunk.window_start_sample;
            let window_end = chunk.window_end_sample;
            let chunk_samples = &samples[window_start..window_end];
            let main_offset_ms = (main_start as u64 * 1000) / SAMPLE_RATE as u64;
            let main_end_ms = ((main_end as u64) * 1000) / SAMPLE_RATE as u64;
            let window_offset_ms = (window_start as u64 * 1000) / SAMPLE_RATE as u64;
            let window_end_ms = ((window_end as u64) * 1000) / SAMPLE_RATE as u64;
            let main_audio_secs = (main_end - main_start) as f64 / SAMPLE_RATE as f64;
            let window_audio_secs = chunk_samples.len() as f64 / SAMPLE_RATE as f64;
            let left_overlap_secs = (main_start - window_start) as f64 / SAMPLE_RATE as f64;
            let right_overlap_secs = (window_end - main_end) as f64 / SAMPLE_RATE as f64;
            let chunk_start = Instant::now();

            tracing::info!(
                "Processing chunk {}/{} (main {:.1}s->{:.1}s / {:.1}s, window {:.1}s->{:.1}s / {:.1}s, overlap L {:.1}s R {:.1}s, {} samples)...",
                chunk_idx + 1,
                num_chunks,
                main_offset_ms as f64 / 1000.0,
                main_end_ms as f64 / 1000.0,
                main_audio_secs,
                window_offset_ms as f64 / 1000.0,
                window_end_ms as f64 / 1000.0,
                window_audio_secs,
                left_overlap_secs,
                right_overlap_secs,
                chunk_samples.len()
            );

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
            params.set_no_context(chunk_idx == 0);
            params.set_token_timestamps(false);
            params.set_no_timestamps(false);
            params.set_suppress_blank(true);
            params.set_suppress_non_speech_tokens(true);
            unsafe {
                params.set_abort_callback(Some(Self::abort_callback));
                params.set_abort_callback_user_data(
                    Arc::as_ptr(cancel_flag) as *const AtomicBool as *mut std::ffi::c_void,
                );
            }

            state.full(params, chunk_samples)
                .map_err(|e| AsrError::Transcription(format!(
                    "Transcription failed at chunk {}: {}", chunk_idx + 1, e
                )))?;

            Self::ensure_not_cancelled(cancel_flag)?;

            let num_segments = state.full_n_segments()
                .map_err(|e| AsrError::Transcription(format!("Failed to get segment count: {}", e)))?;

            if detected_lang.is_none() && num_segments > 0 {
                detected_lang = state.full_lang_id_from_state()
                    .ok()
                    .and_then(|id| whisper_rs::get_lang_str(id).map(String::from));
            }

            let mut kept_segments = 0usize;
            let mut dropped_overlap_segments = 0usize;

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

                let absolute_start_ms = window_offset_ms + seg_start_ms;
                let absolute_end_ms = window_offset_ms + seg_end_ms;
                let segment_midpoint_ms = (absolute_start_ms + absolute_end_ms) / 2;
                let main_start_ms = main_offset_ms;

                // Keep only segments whose midpoint falls inside the chunk's main interval.
                if segment_midpoint_ms < main_start_ms || segment_midpoint_ms > main_end_ms {
                    dropped_overlap_segments += 1;
                    continue;
                }

                all_segments.push(Segment {
                    index: segment_index,
                    start_ms: absolute_start_ms,
                    end_ms: absolute_end_ms,
                    text: trimmed,
                    language: detected_lang.clone().unwrap_or_else(|| "unknown".to_string()),
                    confidence: 1.0,
                });
                segment_index += 1;
                kept_segments += 1;
            }

            let percent = ((chunk_idx + 1) as f32 / num_chunks as f32) * 100.0;
            let chunk_elapsed = chunk_start.elapsed();
            let chunk_elapsed_secs = chunk_elapsed.as_secs_f64();
            let total_elapsed_secs = transcription_start.elapsed().as_secs_f64();
            let avg_chunk_secs = total_elapsed_secs / (chunk_idx + 1) as f64;
            let remaining_chunks = num_chunks.saturating_sub(chunk_idx + 1);
            let eta_secs = avg_chunk_secs * remaining_chunks as f64;
            let realtime_factor = if main_audio_secs > 0.0 {
                chunk_elapsed_secs / main_audio_secs
            } else {
                0.0
            };
            let speed = if chunk_elapsed_secs > 0.0 {
                main_audio_secs / chunk_elapsed_secs
            } else {
                0.0
            };

            tracing::info!(
                "Chunk {}/{} done: total={} (+{} kept, {} dropped by overlap, {} raw), {:.1}%, elapsed={}, avg/chunk={}, speed={:.2}x realtime (RTF {:.2}), eta={}",
                chunk_idx + 1,
                num_chunks,
                all_segments.len(),
                kept_segments,
                dropped_overlap_segments,
                num_segments,
                percent,
                Self::format_duration(chunk_elapsed_secs),
                Self::format_duration(avg_chunk_secs),
                speed,
                realtime_factor,
                Self::format_duration(eta_secs),
            );
            let _ = progress_tx.try_send(AsrProgress {
                percent,
                current_segment: all_segments.last().cloned(),
            });
        }

        Ok((all_segments, detected_lang))
    }

    fn ensure_not_cancelled(cancel_flag: &Arc<AtomicBool>) -> Result<(), AsrError> {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(AsrError::Transcription("Cancelled".into()));
        }
        Ok(())
    }

    fn write_debug_vad_artifacts(
        audio_path: &Path,
        debug_output_dir: Option<&Path>,
        raw_regions: &[super::vad::SpeechRegion],
        merged_regions: &[super::vad::SpeechRegion],
        energy_regions: &[super::vad::SpeechRegion],
        effective_regions: &[super::vad::SpeechRegion],
        split_regions: &[super::vad::SpeechRegion],
        planned_chunks: &[super::vad::PlannedChunk],
        total_duration_ms: u64,
    ) {
        #[derive(Serialize)]
        struct VadDebugArtifacts<'a> {
            audio_path: String,
            total_duration_ms: u64,
            raw_regions: &'a [super::vad::SpeechRegion],
            merged_regions: &'a [super::vad::SpeechRegion],
            energy_regions: &'a [super::vad::SpeechRegion],
            effective_regions: &'a [super::vad::SpeechRegion],
            split_regions: &'a [super::vad::SpeechRegion],
            planned_chunks: &'a [super::vad::PlannedChunk],
        }

        let Some(debug_output_dir) = debug_output_dir else {
            return;
        };
        if std::fs::create_dir_all(debug_output_dir).is_err() {
            return;
        }

        let stem = audio_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let output_path = debug_output_dir.join(format!("{}_vad.json", stem));
        let payload = VadDebugArtifacts {
            audio_path: audio_path.to_string_lossy().to_string(),
            total_duration_ms,
            raw_regions,
            merged_regions,
            energy_regions,
            effective_regions,
            split_regions,
            planned_chunks,
        };

        if let Ok(json) = serde_json::to_vec_pretty(&payload) {
            let _ = std::fs::write(output_path, json);
        }
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
