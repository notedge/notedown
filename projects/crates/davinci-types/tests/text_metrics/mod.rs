use davinci_types::{measure_line, wrap_label};

#[test]
fn latin_widths_come_from_the_hmtx_table() {
    // Trebuchet MS regular: `A` has advance 1208 in 2048 units-per-em.
    assert_eq!(measure_line("A", 2048.0), 1208.0);
    assert_eq!(measure_line("AA", 2048.0), 2416.0);
    // The ellipsis glyph exists in the table.
    assert_eq!(measure_line("…", 2048.0), 1504.0);
    // An unmapped control codepoint falls back to the default advance.
    assert_eq!(measure_line("\u{1}", 2048.0), 617.0);
}

#[test]
fn cjk_measures_full_width() {
    assert_eq!(measure_line("采", 2048.0), 2048.0);
    assert_eq!(measure_line("采集文档", 2048.0), 8192.0);
    // Mixed Latin and CJK accumulate per-character advances.
    assert_eq!(measure_line("A采", 2048.0), 1208.0 + 2048.0);
}

#[test]
fn wrapping_keeps_words_and_respects_the_width_budget() {
    let lines = wrap_label("alpha beta gamma delta epsilon", 12.0, 60.0, 10);
    assert!(lines.len() > 1, "the label must wrap: {lines:?}");
    for line in &lines {
        assert!(measure_line(line, 12.0) <= 60.0, "line exceeds the budget: {line}");
    }
    assert_eq!(lines.join(" ").replace('…', ""), "alpha beta gamma delta epsilon");
}

#[test]
fn overlong_words_split_by_character() {
    let lines = wrap_label("supercalifragilisticexpialidocious", 12.0, 50.0, 10);
    assert!(lines.len() >= 2, "{lines:?}");
    assert_eq!(lines.concat(), "supercalifragilisticexpialidocious");
    for line in &lines {
        assert!(measure_line(line, 12.0) <= 50.0, "{line}");
    }
}

#[test]
fn overflow_truncates_the_last_line_with_an_ellipsis() {
    let long = (0..40).map(|index| format!("word{index}")).collect::<Vec<_>>().join(" ");
    let lines = wrap_label(&long, 12.0, 200.0, 3);
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines[2].ends_with('…'), "{lines:?}");
    assert!(measure_line(&lines[2], 12.0) <= 200.0, "{lines:?}");
    for line in &lines {
        assert!(measure_line(line, 12.0) <= 200.0, "{line}");
    }
}

#[test]
fn measurement_is_deterministic() {
    assert_eq!(measure_line("采集文档 A --> B", 13.0), measure_line("采集文档 A --> B", 13.0));
    assert_eq!(wrap_label("alpha beta gamma", 12.0, 60.0, 3), wrap_label("alpha beta gamma", 12.0, 60.0, 3));
}
