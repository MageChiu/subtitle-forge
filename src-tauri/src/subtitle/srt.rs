// ============================================================
// subtitle/srt.rs — SRT format reader/writer
// ============================================================

use super::types::*;
use crate::error::SubtitleError;

pub struct SrtWriter;
pub struct SrtParser;

impl SrtWriter {
    /// Generate SRT subtitle content
    pub fn write(subtitle: &SubtitleFile) -> String {
        let mut output = String::new();

        for entry in &subtitle.entries {
            // Index
            output.push_str(&entry.index.to_string());
            output.push('\n');

            // Timecodes
            output.push_str(&format!(
                "{} --> {}\n",
                entry.start.to_srt_string(),
                entry.end.to_srt_string()
            ));

            // Primary text
            output.push_str(&entry.primary_text);
            output.push('\n');

            // Secondary text (bilingual)
            if let Some(ref secondary) = entry.secondary_text {
                output.push_str(secondary);
                output.push('\n');
            }

            // Blank line separator
            output.push('\n');
        }

        output
    }
}

impl SrtParser {
    /// Parse SRT subtitle content
    pub fn parse(content: &str) -> Result<Vec<SubtitleEntry>, SubtitleError> {
        let mut entries = Vec::new();
        let blocks: Vec<&str> = content.split("\n\n").collect();

        for (block_idx, block) in blocks.iter().enumerate() {
            let block = block.trim();
            if block.is_empty() {
                continue;
            }

            let lines: Vec<&str> = block.lines().collect();
            if lines.len() < 3 {
                continue; // Skip malformed blocks
            }

            // Parse index
            let index: usize = lines[0].trim().parse().map_err(|_| SubtitleError::Parse {
                line: block_idx * 4 + 1,
                message: format!("Invalid index: '{}'", lines[0]),
            })?;

            // Parse timecodes
            let time_parts: Vec<&str> = lines[1].split("-->").collect();
            if time_parts.len() != 2 {
                return Err(SubtitleError::Parse {
                    line: block_idx * 4 + 2,
                    message: format!("Invalid timecode line: '{}'", lines[1]),
                });
            }

            let start = Timecode::parse_srt(time_parts[0]).ok_or_else(|| SubtitleError::Parse {
                line: block_idx * 4 + 2,
                message: format!("Invalid start timecode: '{}'", time_parts[0]),
            })?;

            let end = Timecode::parse_srt(time_parts[1]).ok_or_else(|| SubtitleError::Parse {
                line: block_idx * 4 + 2,
                message: format!("Invalid end timecode: '{}'", time_parts[1]),
            })?;

            // Parse text (remaining lines)
            let text_lines: Vec<&str> = lines[2..].to_vec();
            let primary_text = text_lines[0].trim().to_string();
            let secondary_text = if text_lines.len() > 1 {
                Some(text_lines[1..].join("\n").trim().to_string())
            } else {
                None
            };

            entries.push(SubtitleEntry {
                index,
                start,
                end,
                primary_text,
                secondary_text,
            });
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timecode_from_ms() {
        let tc = Timecode::from_ms(3_723_456);
        assert_eq!(tc.hours, 1);
        assert_eq!(tc.minutes, 2);
        assert_eq!(tc.seconds, 3);
        assert_eq!(tc.milliseconds, 456);
        assert_eq!(tc.to_srt_string(), "01:02:03,456");
    }

    #[test]
    fn test_timecode_roundtrip() {
        let original_ms: u64 = 7_384_123;
        let tc = Timecode::from_ms(original_ms);
        assert_eq!(tc.to_ms(), original_ms);
    }

    #[test]
    fn test_srt_write_monolingual() {
        let subtitle = SubtitleFile {
            entries: vec![SubtitleEntry {
                index: 1,
                start: Timecode::from_ms(1000),
                end: Timecode::from_ms(4000),
                primary_text: "Hello, world!".to_string(),
                secondary_text: None,
            }],
            source_language: "en".to_string(),
            target_language: None,
            format: SubtitleFormat::Srt,
        };

        let srt = SrtWriter::write(&subtitle);
        assert!(srt.contains("1\n"));
        assert!(srt.contains("00:00:01,000 --> 00:00:04,000"));
        assert!(srt.contains("Hello, world!"));
    }

    #[test]
    fn test_srt_write_bilingual() {
        let subtitle = SubtitleFile {
            entries: vec![SubtitleEntry {
                index: 1,
                start: Timecode::from_ms(1000),
                end: Timecode::from_ms(4000),
                primary_text: "Hello, world!".to_string(),
                secondary_text: Some("你好，世界！".to_string()),
            }],
            source_language: "en".to_string(),
            target_language: Some("zh".to_string()),
            format: SubtitleFormat::Srt,
        };

        let srt = SrtWriter::write(&subtitle);
        assert!(srt.contains("Hello, world!"));
        assert!(srt.contains("你好，世界！"));
    }

    #[test]
    fn test_srt_parse() {
        let content = r#"1
00:00:01,000 --> 00:00:04,000
Hello, world!
你好，世界！

2
00:00:05,000 --> 00:00:08,500
This is a test.
这是一个测试。
"#;

        let entries = SrtParser::parse(content).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].primary_text, "Hello, world!");
        assert_eq!(
            entries[0].secondary_text.as_deref(),
            Some("你好，世界！")
        );
        assert_eq!(entries[1].start.to_ms(), 5000);
    }
}
