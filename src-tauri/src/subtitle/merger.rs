// ============================================================
// subtitle/merger.rs — Merge ASR segments + translations
// ============================================================

use super::types::*;
use crate::asr::engine::Segment;
use crate::error::SubtitleError;
use crate::translate::engine::TranslateResult;

pub struct SubtitleMerger;

impl SubtitleMerger {
    /// Merge ASR segments and translation results into a bilingual SubtitleFile
    pub fn merge(
        segments: &[Segment],
        translation: &TranslateResult,
        source_lang: &str,
        target_lang: &str,
        format: SubtitleFormat,
    ) -> Result<SubtitleFile, SubtitleError> {
        if segments.len() != translation.texts.len() {
            return Err(SubtitleError::SegmentMismatch {
                asr: segments.len(),
                translation: translation.texts.len(),
            });
        }

        let entries: Vec<SubtitleEntry> = segments
            .iter()
            .zip(translation.texts.iter())
            .enumerate()
            .map(|(i, (seg, translated))| SubtitleEntry {
                index: i + 1,
                start: Timecode::from_ms(seg.start_ms),
                end: Timecode::from_ms(seg.end_ms),
                primary_text: seg.text.trim().to_string(),
                secondary_text: Some(translated.trim().to_string()),
            })
            .collect();

        Ok(SubtitleFile {
            entries,
            source_language: source_lang.to_string(),
            target_language: Some(target_lang.to_string()),
            format,
        })
    }

    /// Create monolingual subtitle from ASR segments (no translation)
    pub fn from_segments(
        segments: &[Segment],
        source_lang: &str,
        format: SubtitleFormat,
    ) -> SubtitleFile {
        let entries: Vec<SubtitleEntry> = segments
            .iter()
            .enumerate()
            .map(|(i, seg)| SubtitleEntry {
                index: i + 1,
                start: Timecode::from_ms(seg.start_ms),
                end: Timecode::from_ms(seg.end_ms),
                primary_text: seg.text.trim().to_string(),
                secondary_text: None,
            })
            .collect();

        SubtitleFile {
            entries,
            source_language: source_lang.to_string(),
            target_language: None,
            format,
        }
    }
}
