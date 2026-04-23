// ============================================================
// subtitle/ass.rs — ASS format writer with bilingual styling
// ============================================================

use super::types::*;
use serde::{Deserialize, Serialize};

/// ASS style configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssStyle {
    pub primary_font: String,
    pub primary_size: u32,
    pub primary_color: String,   // ASS color format: &HAABBGGRR
    pub secondary_font: String,
    pub secondary_size: u32,
    pub secondary_color: String,
    pub outline_size: u32,
    pub shadow_size: u32,
    pub play_res_x: u32,
    pub play_res_y: u32,
}

impl Default for AssStyle {
    fn default() -> Self {
        Self {
            primary_font: "Arial".to_string(),
            primary_size: 48,
            primary_color: "&H00FFFFFF".to_string(),    // White
            secondary_font: "Arial".to_string(),
            secondary_size: 36,
            secondary_color: "&H0000FFFF".to_string(),  // Yellow
            outline_size: 2,
            shadow_size: 1,
            play_res_x: 1920,
            play_res_y: 1080,
        }
    }
}

pub struct AssWriter;

impl AssWriter {
    /// Generate ASS subtitle content with bilingual styles
    pub fn write(subtitle: &SubtitleFile, style: &AssStyle) -> String {
        let mut output = String::new();

        // [Script Info]
        output.push_str("[Script Info]\n");
        output.push_str("Title: SubtitleForge Generated Bilingual Subtitles\n");
        output.push_str("ScriptType: v4.00+\n");
        output.push_str(&format!("PlayResX: {}\n", style.play_res_x));
        output.push_str(&format!("PlayResY: {}\n", style.play_res_y));
        output.push_str("WrapStyle: 0\n");
        output.push_str("ScaledBorderAndShadow: yes\n");
        output.push_str("YCbCr Matrix: None\n");
        output.push('\n');

        // [V4+ Styles]
        output.push_str("[V4+ Styles]\n");
        output.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");

        // Primary style (bottom, alignment=2)
        output.push_str(&format!(
            "Style: Primary,{},{},{},&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,{},{},2,10,10,30,1\n",
            style.primary_font, style.primary_size, style.primary_color,
            style.outline_size, style.shadow_size
        ));

        // Secondary style (top, alignment=8)
        output.push_str(&format!(
            "Style: Secondary,{},{},{},&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,{},{},8,10,10,30,1\n",
            style.secondary_font, style.secondary_size, style.secondary_color,
            style.outline_size, style.shadow_size
        ));

        output.push('\n');

        // [Events]
        output.push_str("[Events]\n");
        output.push_str("Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n");

        for entry in &subtitle.entries {
            let start = entry.start.to_ass_string();
            let end = entry.end.to_ass_string();

            // Primary language (bottom)
            output.push_str(&format!(
                "Dialogue: 0,{},{},Primary,,0,0,0,,{}\n",
                start, end,
                Self::escape_ass_text(&entry.primary_text)
            ));

            // Secondary language (top)
            if let Some(ref secondary) = entry.secondary_text {
                output.push_str(&format!(
                    "Dialogue: 0,{},{},Secondary,,0,0,0,,{}\n",
                    start, end,
                    Self::escape_ass_text(secondary)
                ));
            }
        }

        output
    }

    /// Escape special characters in ASS text
    fn escape_ass_text(text: &str) -> String {
        text.replace('\\', "\\\\")
            .replace('{', "\\{")
            .replace('}', "\\}")
    }
}
