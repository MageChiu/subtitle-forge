use crate::error::AsrError;
use fast_vad::vad::detector::{VadConfig as FastVadConfig, VAD};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SpeechRegion {
    pub start_sample: usize,
    pub end_sample: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedChunk {
    pub main_start_sample: usize,
    pub main_end_sample: usize,
    pub window_start_sample: usize,
    pub window_end_sample: usize,
}

#[derive(Debug, Clone)]
pub struct VadPlannerConfig {
    pub sample_rate: u32,
    pub min_speech_ms: u32,
    pub min_silence_ms: u32,
    pub hangover_ms: u32,
    pub max_merge_gap_ms: u32,
    pub min_chunk_ms: u32,
    pub max_chunk_ms: u32,
    pub overlap_ms: u32,
    pub threshold_probability: f32,
    pub energy_frame_ms: u32,
    pub energy_threshold_floor: f32,
    pub energy_threshold_ratio: f32,
}

impl Default for VadPlannerConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            min_speech_ms: 180,
            min_silence_ms: 450,
            hangover_ms: 80,
            max_merge_gap_ms: 250,
            min_chunk_ms: 15_000,
            max_chunk_ms: 120_000,
            overlap_ms: 5_000,
            threshold_probability: 0.82,
            energy_frame_ms: 20,
            energy_threshold_floor: 0.008,
            energy_threshold_ratio: 0.18,
        }
    }
}

pub struct VadPlanner;

impl VadPlanner {
    pub fn detect_regions(
        samples: &[f32],
        config: &VadPlannerConfig,
    ) -> Result<Vec<SpeechRegion>, AsrError> {
        let vad = VAD::with_config(
            config.sample_rate as usize,
            FastVadConfig {
                threshold_probability: config.threshold_probability,
                min_speech_ms: config.min_speech_ms as usize,
                min_silence_ms: config.min_silence_ms as usize,
                hangover_ms: config.hangover_ms as usize,
            },
        )
        .map_err(|e| AsrError::Transcription(format!("Failed to initialize VAD: {}", e)))?;

        let raw_segments = vad.detect_segments(samples);
        Ok(raw_segments
            .into_iter()
            .filter_map(|segment| {
                let start_sample = segment[0] as usize;
                let end_sample = segment[1] as usize;
                (end_sample > start_sample).then_some(SpeechRegion {
                    start_sample,
                    end_sample,
                })
            })
            .collect())
    }

    pub fn merge_regions(
        regions: &[SpeechRegion],
        config: &VadPlannerConfig,
    ) -> Vec<SpeechRegion> {
        if regions.is_empty() {
            return Vec::new();
        }

        let max_gap_samples =
            ((config.max_merge_gap_ms as u64 * config.sample_rate as u64) / 1000) as usize;
        let min_chunk_samples =
            ((config.min_chunk_ms as u64 * config.sample_rate as u64) / 1000) as usize;

        let mut merged = Vec::new();
        let mut current = regions[0].clone();

        for region in &regions[1..] {
            let gap = region.start_sample.saturating_sub(current.end_sample);
            let current_len = current.end_sample.saturating_sub(current.start_sample);
            if gap <= max_gap_samples || current_len < min_chunk_samples {
                current.end_sample = current.end_sample.max(region.end_sample);
            } else {
                merged.push(current);
                current = region.clone();
            }
        }
        merged.push(current);
        merged
    }

    pub fn detect_energy_regions(
        samples: &[f32],
        config: &VadPlannerConfig,
    ) -> Vec<SpeechRegion> {
        let frame_samples = (((config.energy_frame_ms as u64) * config.sample_rate as u64) / 1000)
            as usize;
        if frame_samples == 0 || samples.is_empty() {
            return Vec::new();
        }

        let mut frame_rms = Vec::new();
        let mut pos = 0usize;
        while pos + frame_samples <= samples.len() {
            frame_rms.push(Self::frame_rms(&samples[pos..pos + frame_samples]));
            pos += frame_samples;
        }
        if frame_rms.is_empty() {
            return Vec::new();
        }

        let mut sorted = frame_rms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p20 = sorted[((sorted.len() as f32 * 0.20).floor() as usize).min(sorted.len() - 1)];
        let p95 = sorted[((sorted.len() as f32 * 0.95).floor() as usize).min(sorted.len() - 1)];
        let threshold =
            config.energy_threshold_floor.max(p20 + (p95 - p20) * config.energy_threshold_ratio);

        let mut active: Vec<bool> = frame_rms.iter().map(|&rms| rms >= threshold).collect();
        let min_speech_frames =
            ((config.min_speech_ms as usize) / config.energy_frame_ms as usize).max(1);
        let min_silence_frames =
            ((config.min_silence_ms as usize) / config.energy_frame_ms as usize).max(1);

        Self::remove_short_runs(&mut active, true, min_speech_frames);
        Self::fill_short_runs(&mut active, false, min_silence_frames);
        Self::regions_from_flags(&active, frame_samples)
    }

    pub fn intersect_regions(
        primary: &[SpeechRegion],
        secondary: &[SpeechRegion],
        min_overlap_samples: usize,
    ) -> Vec<SpeechRegion> {
        let mut result = Vec::new();
        let mut i = 0usize;
        let mut j = 0usize;

        while i < primary.len() && j < secondary.len() {
            let start = primary[i].start_sample.max(secondary[j].start_sample);
            let end = primary[i].end_sample.min(secondary[j].end_sample);
            if end > start && end - start >= min_overlap_samples {
                result.push(SpeechRegion {
                    start_sample: start,
                    end_sample: end,
                });
            }

            if primary[i].end_sample < secondary[j].end_sample {
                i += 1;
            } else {
                j += 1;
            }
        }

        result
    }

    pub fn plan_chunks(
        regions: &[SpeechRegion],
        samples: &[f32],
        total_samples: usize,
        config: &VadPlannerConfig,
    ) -> Vec<PlannedChunk> {
        if regions.is_empty() {
            return Vec::new();
        }

        let regions = Self::split_long_regions_by_energy(regions, samples, config);
        Self::plan_chunks_from_split_regions(&regions, total_samples, config)
    }

    pub fn plan_chunks_from_split_regions(
        regions: &[SpeechRegion],
        total_samples: usize,
        config: &VadPlannerConfig,
    ) -> Vec<PlannedChunk> {
        if regions.is_empty() {
            return Vec::new();
        }

        let max_chunk_samples =
            ((config.max_chunk_ms as u64 * config.sample_rate as u64) / 1000) as usize;
        let min_chunk_samples =
            ((config.min_chunk_ms as u64 * config.sample_rate as u64) / 1000) as usize;
        let overlap_samples =
            ((config.overlap_ms as u64 * config.sample_rate as u64) / 1000) as usize;

        let mut planned = Vec::new();
        let mut idx = 0usize;

        while idx < regions.len() {
            let chunk_start = regions[idx].start_sample;
            let mut chunk_end = regions[idx].end_sample;
            let mut next_idx = idx + 1;

            while next_idx < regions.len() {
                let candidate_end = regions[next_idx].end_sample;
                if candidate_end.saturating_sub(chunk_start) > max_chunk_samples {
                    break;
                }
                chunk_end = candidate_end;
                next_idx += 1;
            }

            if chunk_end.saturating_sub(chunk_start) > max_chunk_samples {
                let mut sub_start = chunk_start;
                while sub_start < chunk_end {
                    let sub_end = (sub_start + max_chunk_samples).min(chunk_end);
                    planned.push(Self::build_chunk(
                        sub_start,
                        sub_end,
                        total_samples,
                        overlap_samples,
                    ));
                    sub_start = sub_end;
                }
            } else {
                let main_end = if chunk_end.saturating_sub(chunk_start) < min_chunk_samples {
                    (chunk_start + min_chunk_samples).min(total_samples)
                } else {
                    chunk_end
                };
                planned.push(Self::build_chunk(
                    chunk_start,
                    main_end,
                    total_samples,
                    overlap_samples,
                ));
            }

            idx = next_idx;
        }

        planned
    }

    pub fn split_long_regions_by_energy(
        regions: &[SpeechRegion],
        samples: &[f32],
        config: &VadPlannerConfig,
    ) -> Vec<SpeechRegion> {
        let max_chunk_samples =
            ((config.max_chunk_ms as u64 * config.sample_rate as u64) / 1000) as usize;
        let min_chunk_samples =
            ((config.min_chunk_ms as u64 * config.sample_rate as u64) / 1000) as usize;
        let frame_samples = ((20_u64 * config.sample_rate as u64) / 1000) as usize;
        let search_radius_samples = ((10_000_u64 * config.sample_rate as u64) / 1000) as usize;

        let mut split_regions = Vec::new();

        for region in regions {
            let region_len = region.end_sample.saturating_sub(region.start_sample);
            if region_len <= max_chunk_samples {
                split_regions.push(region.clone());
                continue;
            }

            let mut sub_start = region.start_sample;
            while region.end_sample.saturating_sub(sub_start) > max_chunk_samples {
                let target = sub_start + max_chunk_samples;
                let search_start = target
                    .saturating_sub(search_radius_samples)
                    .max(sub_start + min_chunk_samples / 2);
                let search_end = (target + search_radius_samples)
                    .min(region.end_sample.saturating_sub(min_chunk_samples / 2));

                let cut = Self::find_low_energy_cut(
                    samples,
                    search_start,
                    search_end,
                    frame_samples.max(1),
                    target,
                );

                let cut = cut
                    .max(sub_start + min_chunk_samples.min(region.end_sample.saturating_sub(sub_start)))
                    .min(region.end_sample.saturating_sub(min_chunk_samples.min(region.end_sample.saturating_sub(sub_start))));

                if cut <= sub_start || cut >= region.end_sample {
                    break;
                }

                split_regions.push(SpeechRegion {
                    start_sample: sub_start,
                    end_sample: cut,
                });
                sub_start = cut;
            }

            if sub_start < region.end_sample {
                split_regions.push(SpeechRegion {
                    start_sample: sub_start,
                    end_sample: region.end_sample,
                });
            }
        }

        split_regions
    }

    fn find_low_energy_cut(
        samples: &[f32],
        search_start: usize,
        search_end: usize,
        frame_samples: usize,
        fallback: usize,
    ) -> usize {
        if search_end <= search_start || frame_samples == 0 {
            return fallback;
        }

        let mut best_pos = fallback;
        let mut best_energy = f32::INFINITY;
        let mut pos = search_start;

        while pos + frame_samples <= search_end && pos + frame_samples <= samples.len() {
            let energy = Self::frame_rms(&samples[pos..pos + frame_samples]);
            if energy < best_energy {
                best_energy = energy;
                best_pos = pos + frame_samples / 2;
            }
            pos += frame_samples;
        }

        best_pos
    }

    fn regions_from_flags(flags: &[bool], frame_samples: usize) -> Vec<SpeechRegion> {
        let mut regions = Vec::new();
        let mut start: Option<usize> = None;

        for (idx, &is_active) in flags.iter().enumerate() {
            match (start, is_active) {
                (None, true) => start = Some(idx * frame_samples),
                (Some(region_start), false) => {
                    regions.push(SpeechRegion {
                        start_sample: region_start,
                        end_sample: idx * frame_samples,
                    });
                    start = None;
                }
                _ => {}
            }
        }

        if let Some(region_start) = start {
            regions.push(SpeechRegion {
                start_sample: region_start,
                end_sample: flags.len() * frame_samples,
            });
        }

        regions
    }

    fn remove_short_runs(flags: &mut [bool], target: bool, min_len: usize) {
        let mut idx = 0usize;
        while idx < flags.len() {
            if flags[idx] != target {
                idx += 1;
                continue;
            }
            let start = idx;
            while idx < flags.len() && flags[idx] == target {
                idx += 1;
            }
            if idx - start < min_len {
                for flag in &mut flags[start..idx] {
                    *flag = !target;
                }
            }
        }
    }

    fn fill_short_runs(flags: &mut [bool], target: bool, max_len: usize) {
        let mut idx = 0usize;
        while idx < flags.len() {
            if flags[idx] != target {
                idx += 1;
                continue;
            }
            let start = idx;
            while idx < flags.len() && flags[idx] == target {
                idx += 1;
            }
            if idx - start <= max_len {
                for flag in &mut flags[start..idx] {
                    *flag = !target;
                }
            }
        }
    }

    fn frame_rms(frame: &[f32]) -> f32 {
        if frame.is_empty() {
            return 0.0;
        }
        let sum = frame.iter().map(|v| v * v).sum::<f32>();
        (sum / frame.len() as f32).sqrt()
    }

    pub fn plan_fixed_chunks(total_samples: usize, config: &VadPlannerConfig) -> Vec<PlannedChunk> {
        let max_chunk_samples =
            ((config.max_chunk_ms as u64 * config.sample_rate as u64) / 1000) as usize;
        let overlap_samples =
            ((config.overlap_ms as u64 * config.sample_rate as u64) / 1000) as usize;

        let mut chunks = Vec::new();
        let mut start = 0usize;
        while start < total_samples {
            let end = (start + max_chunk_samples).min(total_samples);
            chunks.push(Self::build_chunk(start, end, total_samples, overlap_samples));
            start = end;
        }
        if chunks.is_empty() {
            chunks.push(Self::build_chunk(0, 0, total_samples, overlap_samples));
        }
        chunks
    }

    pub fn build_chunk(
        main_start_sample: usize,
        main_end_sample: usize,
        total_samples: usize,
        overlap_samples: usize,
    ) -> PlannedChunk {
        PlannedChunk {
            main_start_sample,
            main_end_sample,
            window_start_sample: main_start_sample.saturating_sub(overlap_samples),
            window_end_sample: (main_end_sample + overlap_samples).min(total_samples),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_close_regions() {
        let config = VadPlannerConfig::default();
        let regions = vec![
            SpeechRegion {
                start_sample: 0,
                end_sample: 16_000,
            },
            SpeechRegion {
                start_sample: 16_000 + 4_000,
                end_sample: 40_000,
            },
        ];

        let merged = VadPlanner::merge_regions(&regions, &config);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].start_sample, 0);
        assert_eq!(merged[0].end_sample, 40_000);
    }

    #[test]
    fn plans_chunks_with_overlap() {
        let config = VadPlannerConfig::default();
        let regions = vec![
            SpeechRegion {
                start_sample: 16_000,
                end_sample: 80_000,
            },
            SpeechRegion {
                start_sample: 100_000,
                end_sample: 180_000,
            },
        ];

        let samples = vec![0.0f32; 400_000];
        let chunks = VadPlanner::plan_chunks(&regions, &samples, 400_000, &config);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].window_start_sample < chunks[0].main_start_sample);
        assert!(chunks[0].window_end_sample > chunks[0].main_end_sample);
    }

    #[test]
    fn splits_long_regions() {
        let config = VadPlannerConfig::default();
        let samples = vec![0.0f32; 4_000_000];
        let regions = vec![SpeechRegion {
            start_sample: 0,
            end_sample: 3_500_000,
        }];

        let split = VadPlanner::split_long_regions_by_energy(&regions, &samples, &config);
        assert!(split.len() > 1);
    }

    #[test]
    fn detects_energy_regions() {
        let config = VadPlannerConfig::default();
        let mut samples = vec![0.0f32; 16_000];
        for sample in &mut samples[4_000..8_000] {
            *sample = 0.2;
        }
        let regions = VadPlanner::detect_energy_regions(&samples, &config);
        assert!(!regions.is_empty());
    }
}
