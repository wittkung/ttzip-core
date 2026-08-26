// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use super::*;

    const SAMPLE_ASS: &str = r##"
[Script Info]
Title: Test ASS Script
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080
WrapStyle: 0
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,24,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
Style: TopHeader,Helvetica,32,&H0000FFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,1,1,8,20,20,20,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:04.00,Default,,0,0,0,,{\pos(960,540)\an5\b1}Hello {\c&H0000FF&}World\NSecond line{\r}Plain
Dialogue: 1,0:00:02.50,0:00:05.50,TopHeader,Alice,20,20,20,,{\i1}Overlapping dialogue{\i0}
"##;

    const SAMPLE_SRT: &str = r##"
1
00:00:01,000 --> 00:00:04,500
Hello <b>Bold</b> and <font color="#FF0000">Red</font>!
Second line in cue

2
00:00:05,000 --> 00:00:08,200
<i>Italic</i> and <u>Underlined</u> text
"##;

    const SAMPLE_VTT: &str = r##"WEBVTT - Sample WebVTT File

1
00:00:01.000 --> 00:00:04.000 position:10%
<v John>Hello <b>world</b> from WebVTT!</v>

00:05.500 --> 00:08.500
Short timecode format cue
"##;

    #[test]
    fn test_ass_parser_full_document() {
        let script = parse_subtitle_script(SAMPLE_ASS, None);
        assert_eq!(script.format, SubtitleFormat::Ass);
        assert_eq!(script.title.as_deref(), Some("Test ASS Script"));
        assert_eq!(script.play_res_x, Some(1920));
        assert_eq!(script.play_res_y, Some(1080));
        assert_eq!(script.scaled_border_and_shadow, Some(true));

        assert_eq!(script.styles.len(), 2);
        let default_style = script.styles.get("Default").expect("Default style missing");
        assert_eq!(default_style.font_name, "Arial");
        assert_eq!(default_style.font_size, 24.0);
        assert!(default_style.bold);
        assert_eq!(default_style.alignment, SubtitleAlignment::BottomCenter);

        let top_style = script.styles.get("TopHeader").expect("TopHeader style missing");
        assert_eq!(top_style.alignment, SubtitleAlignment::TopCenter);

        assert_eq!(script.dialogues.len(), 2);
        let d0 = &script.dialogues[0];
        assert_eq!(d0.start_ms, 1000);
        assert_eq!(d0.end_ms, 4000);
        assert_eq!(d0.plain_text, "Hello World\nSecond linePlain");

        let pos_span = &d0.spans[0];
        assert_eq!(pos_span.position, Some((960.0, 540.0)));
        assert_eq!(pos_span.alignment, Some(SubtitleAlignment::MiddleCenter));
        assert_eq!(pos_span.bold, Some(true));
    }

    #[test]
    fn test_ass_color_and_inline_tags() {
        let text = r#"{\b1\c&H0000FF&\pos(100,200)}Red Text\N{\i1\1c&H00FF00&}Green Italic"#;
        let (spans, plain) = parse_ass_spans(text);
        assert_eq!(plain, "Red Text\nGreen Italic");
        assert!(!spans.is_empty());

        let red_span = &spans[0];
        assert_eq!(red_span.text, "Red Text");
        assert_eq!(red_span.bold, Some(true));
        assert_eq!(red_span.primary_color, Some(SubtitleColor::from_rgb(255, 0, 0)));
        assert_eq!(red_span.position, Some((100.0, 200.0)));

        let green_span = spans.iter().find(|s| s.text == "Green Italic").expect("Green span");
        assert_eq!(green_span.italic, Some(true));
        assert_eq!(green_span.primary_color, Some(SubtitleColor::from_rgb(0, 255, 0)));
    }

    #[test]
    fn test_srt_parser_and_html_tags() {
        let script = parse_subtitle_script(SAMPLE_SRT, None);
        assert_eq!(script.format, SubtitleFormat::Srt);
        assert_eq!(script.dialogues.len(), 2);

        let d0 = &script.dialogues[0];
        assert_eq!(d0.start_ms, 1000);
        assert_eq!(d0.end_ms, 4500);
        assert_eq!(d0.plain_text, "Hello Bold and Red!\nSecond line in cue");

        let bold_span = d0.spans.iter().find(|s| s.text == "Bold").expect("Bold span");
        assert_eq!(bold_span.bold, Some(true));

        let red_span = d0.spans.iter().find(|s| s.text == "Red").expect("Red span");
        assert_eq!(red_span.primary_color, Some(SubtitleColor::from_rgb(255, 0, 0)));

        let d1 = &script.dialogues[1];
        assert_eq!(d1.start_ms, 5000);
        assert_eq!(d1.end_ms, 8200);
        let italic_span = d1.spans.iter().find(|s| s.text == "Italic").expect("Italic span");
        assert_eq!(italic_span.italic, Some(true));
    }

    #[test]
    fn test_vtt_parser_and_voice_tags() {
        let script = parse_subtitle_script(SAMPLE_VTT, None);
        assert_eq!(script.format, SubtitleFormat::Vtt);
        assert_eq!(script.dialogues.len(), 2);

        let d0 = &script.dialogues[0];
        assert_eq!(d0.start_ms, 1000);
        assert_eq!(d0.end_ms, 4000);
        assert_eq!(d0.actor, "John");
        assert_eq!(d0.plain_text, "Hello world from WebVTT!");

        let d1 = &script.dialogues[1];
        assert_eq!(d1.start_ms, 5500);
        assert_eq!(d1.end_ms, 8500);
        assert_eq!(d1.plain_text, "Short timecode format cue");
    }

    #[test]
    fn test_timeline_binary_search_overlapping() {
        let d1 = SubtitleDialogue {
            layer: 0, start_ms: 1000, end_ms: 5000, style: "Default".to_string(),
            actor: "D1".to_string(), margin_l: 0, margin_r: 0, margin_v: 0,
            effect: "".to_string(), raw_text: "Line 1".to_string(),
            plain_text: "Line 1".to_string(), spans: vec![SubtitleSpan::plain("Line 1")],
        };
        let mut d2 = d1.clone();
        d2.start_ms = 2000; d2.end_ms = 4000; d2.actor = "D2".to_string();
        let mut d3 = d1.clone();
        d3.start_ms = 3000; d3.end_ms = 6000; d3.actor = "D3".to_string();

        let timeline = SubtitleTimeline::new(vec![d3, d1, d2]);
        assert_eq!(timeline.len(), 3);
        assert_eq!(timeline.total_duration_ms(), 6000);

        // Point checks
        assert_eq!(timeline.find_active_dialogues(500).len(), 0);
        assert_eq!(timeline.find_active_dialogues(1000).len(), 1);
        assert_eq!(timeline.find_active_dialogues(2500).len(), 2);
        assert_eq!(timeline.find_active_dialogues(3500).len(), 3);
        assert_eq!(timeline.find_active_dialogues(4500).len(), 2);
        assert_eq!(timeline.find_active_dialogues(5500).len(), 1);
        assert_eq!(timeline.find_active_dialogues(6000).len(), 0);

        // Microsecond checks
        assert_eq!(timeline.find_active_dialogues_micros(2_500_000).len(), 2);
        assert_eq!(timeline.find_active_dialogues_micros(3_500_000).len(), 3);

        // Range checks
        let in_range = timeline.find_dialogues_in_range(2000, 3500);
        assert_eq!(in_range.len(), 3);
    }

    #[test]
    fn test_edge_cases_and_resilience() {
        // Empty content
        let empty_script = parse_subtitle_script("", None);
        assert!(empty_script.dialogues.is_empty());
        let empty_timeline = SubtitleTimeline::new(vec![]);
        assert!(empty_timeline.find_active_dialogues(1000).is_empty());
        assert_eq!(empty_timeline.total_duration_ms(), 0);

        // Negative timestamps and boundary conditions
        assert!(empty_timeline.find_active_dialogues(-500).is_empty());
        assert!(empty_timeline.find_active_dialogues_micros(-500_000).is_empty());

        // Malformed timestamp fallback
        assert_eq!(parse_ass_timestamp("invalid:time:format"), None);
        assert_eq!(parse_srt_vtt_timestamp("bad_timecode"), None);

        // Color hex edge cases
        assert_eq!(SubtitleColor::from_ass_hex(""), None);
        assert_eq!(SubtitleColor::from_html_hex("xyz"), None);
        assert_eq!(SubtitleColor::from_html_hex("#123"), Some(SubtitleColor::from_rgb(17, 34, 51)));
    }
