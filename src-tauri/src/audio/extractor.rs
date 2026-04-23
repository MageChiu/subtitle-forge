use crate::error::AudioError;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct ExtractConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub format: AudioFormat,
}

impl Default for ExtractConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            format: AudioFormat::Wav,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AudioFormat {
    Wav,
    RawPcmF32,
}

#[derive(Debug, Clone)]
pub struct ExtractProgress {
    pub percent: f32,
    pub duration_ms: u64,
    pub processed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaInfo {
    pub duration_ms: u64,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub audio_sample_rate: u32,
    pub audio_channels: u16,
    pub file_size: u64,
    pub format_name: String,
}

pub struct AudioExtractor;

impl AudioExtractor {
    pub fn probe(input: &Path) -> Result<MediaInfo, AudioError> {
        tracing::info!("Probing media file: {:?}", input);

        if !input.exists() {
            return Err(AudioError::Ffmpeg(format!(
                "File not found: {}",
                input.display()
            )));
        }

        let ictx = ffmpeg_next::format::input(input)
            .map_err(|e| AudioError::Ffmpeg(format!("Failed to open input: {}", e)))?;

        let duration_ms = if ictx.duration() > 0 {
            ictx.duration() as u64 / 1000
        } else {
            0
        };
        let format_name = ictx.format().name().to_string();

        let audio_stream = ictx.streams().best(ffmpeg_next::media::Type::Audio);
        let video_stream = ictx.streams().best(ffmpeg_next::media::Type::Video);

        let audio_codec = audio_stream.as_ref().map(|s| {
            let id = s.parameters().id();
            format!("{:?}", id)
        });

        let video_codec = video_stream.as_ref().map(|s| {
            let id = s.parameters().id();
            format!("{:?}", id)
        });

        let (audio_sample_rate, audio_channels) = audio_stream
            .as_ref()
            .map(|s| {
                let params = s.parameters();
                let codec_ctx = ffmpeg_next::codec::context::Context::from_parameters(params).ok();
                if let Some(ctx) = codec_ctx {
                    if let Ok(audio) = ctx.decoder().audio() {
                        return (audio.rate() as u32, audio.channels() as u16);
                    }
                }
                (0u32, 0u16)
            })
            .unwrap_or((0, 0));

        let file_size = std::fs::metadata(input)
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(MediaInfo {
            duration_ms,
            video_codec,
            audio_codec,
            audio_sample_rate,
            audio_channels,
            file_size,
            format_name,
        })
    }

    pub async fn extract(
        input: &Path,
        output_dir: &Path,
        config: &ExtractConfig,
        progress_tx: mpsc::Sender<ExtractProgress>,
    ) -> Result<PathBuf, AudioError> {
        let input = input.to_path_buf();
        let output_dir = output_dir.to_path_buf();
        let config = config.clone();

        tokio::task::spawn_blocking(move || {
            Self::extract_sync(&input, &output_dir, &config, progress_tx)
        })
        .await
        .map_err(|e| AudioError::Ffmpeg(format!("Task join error: {}", e)))?
    }

    fn extract_sync(
        input: &Path,
        output_dir: &Path,
        config: &ExtractConfig,
        progress_tx: mpsc::Sender<ExtractProgress>,
    ) -> Result<PathBuf, AudioError> {
        let stem = input
            .file_stem()
            .ok_or_else(|| AudioError::Ffmpeg("Invalid input filename".into()))?
            .to_string_lossy();
        let output_path = output_dir.join(format!("{}_audio.wav", stem));

        tracing::info!("Audio extraction: {:?} -> {:?}", input, output_path);

        if !input.exists() {
            return Err(AudioError::Ffmpeg(format!(
                "Input file not found: {}",
                input.display()
            )));
        }

        let mut ictx = ffmpeg_next::format::input(input)
            .map_err(|e| AudioError::Ffmpeg(format!("Failed to open input: {}", e)))?;

        let audio_stream = ictx.streams().best(ffmpeg_next::media::Type::Audio);
        let audio_stream_ref = audio_stream
            .as_ref()
            .ok_or(AudioError::NoAudioStream)?;

        let audio_idx = audio_stream_ref.index();
        let audio_time_base = audio_stream_ref.time_base();
        let duration_ms = if ictx.duration() > 0 {
            ictx.duration() as u64 / 1000
        } else {
            0
        };

        let codec_ctx = ffmpeg_next::codec::context::Context::from_parameters(
            audio_stream_ref.parameters(),
        )
        .map_err(|e| AudioError::Ffmpeg(format!("Failed to create codec context: {}", e)))?;

        drop(audio_stream);

        let mut decoder = codec_ctx
            .decoder()
            .audio()
            .map_err(|e| AudioError::Ffmpeg(format!("Failed to open audio decoder: {}", e)))?;

        let mut resampler = ffmpeg_next::software::resampling::Context::get(
            decoder.format(),
            decoder.channel_layout(),
            decoder.rate(),
            ffmpeg_next::format::Sample::I16(
                ffmpeg_next::format::sample::Type::Packed,
            ),
            ffmpeg_next::channel_layout::ChannelLayout::MONO,
            config.sample_rate,
        )
        .map_err(|e| AudioError::Ffmpeg(format!("Failed to create resampler: {}", e)))?;

        let mut pcm_data: Vec<i16> = Vec::new();
        let mut decoded_frame = ffmpeg_next::util::frame::Audio::empty();
        let mut resampled_frame = ffmpeg_next::util::frame::Audio::empty();

        for (stream, packet) in ictx.packets() {
            if stream.index() == audio_idx {
                decoder.send_packet(&packet)
                    .map_err(|e| AudioError::Ffmpeg(format!("Decode error: {}", e)))?;

                while decoder.receive_frame(&mut decoded_frame).is_ok() {
                    resampler.run(&decoded_frame, &mut resampled_frame)
                        .map_err(|e| AudioError::Ffmpeg(format!("Resample error: {}", e)))?;

                    let data = resampled_frame.data(0);
                    let samples = data.chunks_exact(2)
                        .filter_map(|chunk| {
                            let bytes = [chunk[0], chunk[1]];
                            Some(i16::from_le_bytes(bytes))
                        });
                    pcm_data.extend(samples);

                    if duration_ms > 0 {
                        let ts_ms = decoded_frame.pts()
                            .map(|pts| {
                                let tb = audio_time_base;
                                (pts as f64 * tb.numerator() as f64 / tb.denominator() as f64 * 1000.0) as u64
                            })
                            .unwrap_or(0);
                        let percent = (ts_ms as f32 / duration_ms as f32 * 100.0).min(100.0);
                        let _ = progress_tx.try_send(ExtractProgress {
                            percent,
                            duration_ms,
                            processed_ms: ts_ms,
                        });
                    }
                }
            }
        }

        decoder.send_eof()
            .map_err(|e| AudioError::Ffmpeg(format!("Flush decoder error: {}", e)))?;
        while decoder.receive_frame(&mut decoded_frame).is_ok() {
            resampler.run(&decoded_frame, &mut resampled_frame)
                .map_err(|e| AudioError::Ffmpeg(format!("Resample flush error: {}", e)))?;
            let data = resampled_frame.data(0);
            let samples = data.chunks_exact(2)
                .filter_map(|chunk| {
                    let bytes = [chunk[0], chunk[1]];
                    Some(i16::from_le_bytes(bytes))
                });
            pcm_data.extend(samples);
        }

        tracing::info!("Decoded {} PCM samples ({}ms at {}Hz)",
            pcm_data.len(),
            pcm_data.len() as u64 * 1000 / config.sample_rate as u64,
            config.sample_rate
        );

        Self::write_wav_file(&output_path, &pcm_data, config.sample_rate, 1)?;

        let _ = progress_tx.try_send(ExtractProgress {
            percent: 100.0,
            duration_ms,
            processed_ms: duration_ms,
        });

        tracing::info!("Audio extraction complete: {:?}", output_path);
        Ok(output_path)
    }

    fn write_wav_file(
        path: &Path,
        samples: &[i16],
        sample_rate: u32,
        channels: u16,
    ) -> Result<(), AudioError> {
        let data_size = (samples.len() * 2) as u32;
        let file_size = 36 + data_size;
        let byte_rate = sample_rate * channels as u32 * 2;
        let block_align = channels * 2;
        let bits_per_sample = 16u16;

        let mut file = std::fs::File::create(path)
            .map_err(|e| AudioError::Ffmpeg(format!("Failed to create WAV file: {}", e)))?;

        file.write_all(b"RIFF")
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;
        file.write_all(&file_size.to_le_bytes())
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;
        file.write_all(b"WAVE")
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;

        file.write_all(b"fmt ")
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;
        file.write_all(&16u32.to_le_bytes())
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;
        file.write_all(&1u16.to_le_bytes())
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;
        file.write_all(&channels.to_le_bytes())
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;
        file.write_all(&sample_rate.to_le_bytes())
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;
        file.write_all(&byte_rate.to_le_bytes())
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;
        file.write_all(&block_align.to_le_bytes())
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;
        file.write_all(&bits_per_sample.to_le_bytes())
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;

        file.write_all(b"data")
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;
        file.write_all(&data_size.to_le_bytes())
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;

        let bytes: Vec<u8> = samples.iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        file.write_all(&bytes)
            .map_err(|e| AudioError::Ffmpeg(format!("WAV write error: {}", e)))?;

        file.flush()
            .map_err(|e| AudioError::Ffmpeg(format!("WAV flush error: {}", e)))?;

        tracing::info!("WAV file written: {} bytes, {}Hz, {}ch, {} samples",
            data_size + 44, sample_rate, channels, samples.len());

        Ok(())
    }
}
