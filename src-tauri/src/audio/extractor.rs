// ============================================================
// audio/extractor.rs — Audio extraction via FFmpeg
// ============================================================

use crate::error::AudioError;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Audio extraction configuration
#[derive(Debug, Clone)]
pub struct ExtractConfig {
    /// Target sample rate (default: 16000 Hz for Whisper)
    pub sample_rate: u32,
    /// Number of channels (default: 1 = mono)
    pub channels: u16,
    /// Output format
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

/// Progress update during extraction
#[derive(Debug, Clone)]
pub struct ExtractProgress {
    pub percent: f32,
    pub duration_ms: u64,
    pub processed_ms: u64,
}

/// Media file information
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

/// Audio extractor using FFmpeg
pub struct AudioExtractor;

impl AudioExtractor {
    /// Probe media file for metadata
    pub fn probe(input: &Path) -> Result<MediaInfo, AudioError> {
        // Implementation using ffmpeg-next:
        //
        // let ictx = ffmpeg_next::format::input(input)
        //     .map_err(|e| AudioError::Ffmpeg(e.to_string()))?;
        //
        // let duration_ms = ictx.duration() as u64 / 1000; // AV_TIME_BASE
        // let format_name = ictx.format().name().to_string();
        //
        // let audio_stream = ictx.streams().best(ffmpeg_next::media::Type::Audio);
        // let video_stream = ictx.streams().best(ffmpeg_next::media::Type::Video);
        //
        // ... extract codec info, sample rate, channels ...

        todo!("Implement with ffmpeg-next")
    }

    /// Extract audio from video file
    ///
    /// Converts to 16kHz mono WAV suitable for Whisper ASR.
    pub async fn extract(
        input: &Path,
        output_dir: &Path,
        config: &ExtractConfig,
        progress_tx: mpsc::Sender<ExtractProgress>,
    ) -> Result<PathBuf, AudioError> {
        let input = input.to_path_buf();
        let output_dir = output_dir.to_path_buf();
        let config = config.clone();

        // Run FFmpeg processing in blocking thread pool
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
        // Generate output filename
        let stem = input
            .file_stem()
            .ok_or_else(|| AudioError::Ffmpeg("Invalid input filename".into()))?
            .to_string_lossy();
        let output_path = output_dir.join(format!("{}_audio.wav", stem));

        // Implementation outline using ffmpeg-next:
        //
        // 1. Open input context
        //    let mut ictx = ffmpeg_next::format::input(input)?;
        //
        // 2. Find best audio stream
        //    let audio_idx = ictx.streams().best(Type::Audio)
        //        .ok_or(AudioError::NoAudioStream)?.index();
        //
        // 3. Get decoder
        //    let codec_ctx = ffmpeg_next::codec::context::Context::from_parameters(
        //        ictx.stream(audio_idx).unwrap().parameters())?;
        //    let mut decoder = codec_ctx.decoder().audio()?;
        //
        // 4. Set up resampler (to 16kHz mono f32)
        //    let mut resampler = ffmpeg_next::software::resampling::Context::get(
        //        decoder.format(), decoder.channel_layout(), decoder.rate(),
        //        Sample::F32(Type::Planar), ChannelLayout::MONO, config.sample_rate)?;
        //
        // 5. Set up WAV output
        //    let mut octx = ffmpeg_next::format::output(&output_path)?;
        //    ... configure output stream ...
        //
        // 6. Decode loop: read packets → decode → resample → write
        //    for (stream, packet) in ictx.packets() {
        //        if stream.index() == audio_idx {
        //            decoder.send_packet(&packet)?;
        //            while decoder.receive_frame(&mut frame).is_ok() {
        //                resampler.run(&frame, &mut resampled)?;
        //                // write resampled to output
        //                // send progress update
        //            }
        //        }
        //    }

        tracing::info!("Audio extraction: {:?} -> {:?}", input, output_path);

        todo!("Implement with ffmpeg-next")
    }
}
