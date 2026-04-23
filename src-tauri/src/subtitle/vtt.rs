// ============================================================
// subtitle/vtt.rs — WebVTT format writer
// ============================================================

use super::types::*;

pub struct VttWriter;

impl VttWriter {
    /// Generate WebVTT subtitle content
    pub fn write(subtitle: &SubtitleFile) -> String {
        let mut output = String::new();

        // VTT header
        output.push_str("WEBVTT\n");
        output.push_str("Kind: captions\n");
        if let Some(ref target) = subtitle.target_language {
            output.push_str(&format!(
                "Language: {}/{}\n",
                subtitle.source_language, target
            ));
        } else {
            output.push_str(&format!("Language: {}\n", subtitle.source_language));
        }
        output.push('\n');

        for entry in &subtitle.entries {
            // Optional cue identifier
            output.push_str(&format!("cue-{}\n", entry.index));

            // Timecodes
            output.push_str(&format!(
                "{} --> {}\n",
                entry.start.to_vtt_string(),
                entry.end.to_vtt_string()
            ));

            // Text
            output.push_str(&entry.primary_text);
            output.push('\n');
            if let Some(ref secondary) = entry.secondary_text {
                output.push_str(secondary);
                output.push('\n');
            }

            output.push('\n');
        }

        output
    }
}
