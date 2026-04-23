// ============================================================
// subtitle/types.rs — Core subtitle data structures
// ============================================================

use serde::{Deserialize, Serialize};

/// Timecode representation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Timecode {
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub milliseconds: u32,
}

impl Timecode {
    /// Create from total milliseconds
    pub fn from_ms(total_ms: u64) -> Self {
        Self {
            hours: (total_ms / 3_600_000) as u32,
            minutes: ((total_ms % 3_600_000) / 60_000) as u32,
            seconds: ((total_ms % 60_000) / 1_000) as u32,
            milliseconds: (total_ms % 1_000) as u32,
        }
    }

    /// Convert to total milliseconds
    pub fn to_ms(&self) -> u64 {
        self.hours as u64 * 3_600_000
            + self.minutes as u64 * 60_000
            + self.seconds as u64 * 1_000
            + self.milliseconds as u64
    }

    /// Format as SRT timecode: "00:01:23,456"
    pub fn to_srt_string(&self) -> String {
        format!(
            "{:02}:{:02}:{:02},{:03}",
            self.hours, self.minutes, self.seconds, self.milliseconds
        )
    }

    /// Format as VTT timecode: "00:01:23.456"
    pub fn to_vtt_string(&self) -> String {
        format!(
            "{:02}:{:02}:{:02}.{:03}",
            self.hours, self.minutes, self.seconds, self.milliseconds
        )
    }

    /// Format as ASS timecode: "0:01:23.45" (centiseconds)
    pub fn to_ass_string(&self) -> String {
        format!(
            "{}:{:02}:{:02}.{:02}",
            self.hours,
            self.minutes,
            self.seconds,
            self.milliseconds / 10
        )
    }

    /// Parse SRT timecode: "00:01:23,456"
    pub fn parse_srt(s: &str) -> Option<Self> {
        let s = s.trim();
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return None;
        }
        let hours: u32 = parts[0].parse().ok()?;
        let minutes: u32 = parts[1].parse().ok()?;
        let sec_parts: Vec<&str> = parts[2].split(',').collect();
        if sec_parts.len() != 2 {
            return None;
        }
        let seconds: u32 = sec_parts[0].parse().ok()?;
        let milliseconds: u32 = sec_parts[1].parse().ok()?;
        Some(Self {
            hours,
            minutes,
            seconds,
            milliseconds,
        })
    }
}

impl std::fmt::Display for Timecode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_srt_string())
    }
}

/// A single subtitle entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleEntry {
    /// 1-based index
    pub index: usize,
    /// Start timecode
    pub start: Timecode,
    /// End timecode
    pub end: Timecode,
    /// Primary language text
    pub primary_text: String,
    /// Secondary language text (for bilingual subtitles)
    pub secondary_text: Option<String>,
}

/// Complete subtitle file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleFile {
    pub entries: Vec<SubtitleEntry>,
    pub source_language: String,
    pub target_language: Option<String>,
    pub format: SubtitleFormat,
}

/// Supported subtitle formats
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SubtitleFormat {
    Srt,
    Ass,
    Vtt,
}

impl SubtitleFormat {
    pub fn extension(&self) -> &str {
        match self {
            Self::Srt => "srt",
            Self::Ass => "ass",
            Self::Vtt => "vtt",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Srt => "SubRip (SRT)",
            Self::Ass => "Advanced SubStation Alpha (ASS)",
            Self::Vtt => "WebVTT (VTT)",
        }
    }
}
