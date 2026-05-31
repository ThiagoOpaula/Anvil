use super::*;

// ── wrap_lines ────────────────────────────────────────────────────

#[test]
fn wrap_short_text_passes_through() {
    let result = wrap_lines("hello world", 20);
    assert_eq!(result, vec!["hello world"]);
}

#[test]
fn wrap_splits_at_width() {
    let result = wrap_lines("a b c d e f g h i j k l m n o p", 10);
    // Should produce multiple wrapped lines, none exceeding width.
    for line in &result {
        assert!(line.len() <= 10, "line '{}' too long", line);
    }
    // Re-join without wrapping should restore words.
    let rejoined: String = result
        .iter()
        .filter(|l| !l.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(rejoined.contains("a b c"));
}

#[test]
fn wrap_preserves_paragraphs() {
    let result = wrap_lines("hello\n\nworld", 80);
    assert_eq!(result, vec!["hello", "", "world"]);
}

#[test]
fn wrap_long_word_hard_breaks() {
    let result = wrap_lines("abcdefghijklmnopqrstuvwxyz", 5);
    for line in &result {
        assert!(line.len() <= 5, "line '{}' too long", line);
    }
}

// ── format_outcome_table ──────────────────────────────────────────

#[test]
fn outcome_rows_match_header_count() {
    let outcomes = vec![
        ModOutcome::UpToDate {
            slug: "sodium".into(),
            filename: "sodium.jar".into(),
            version: "0.5.11".into(),
        },
        ModOutcome::Unknown {
            filename: "mystery.jar".into(),
        },
    ];
    let rows = format_outcome_table(&outcomes);
    assert_eq!(rows.len(), 2);
    // Each row must have exactly 4 columns when used with
    // headers ["Status", "Mod", "Old Version", "New Version"].
    for row in &rows {
        assert_eq!(row.len(), 4);
    }
}

#[test]
fn updated_row_contains_arrow() {
    let outcomes = vec![ModOutcome::Updated {
        slug: "iris".into(),
        old_filename: "iris-1.7.jar".into(),
        new_filename: "iris-1.8.jar".into(),
        old_version: "1.7".into(),
        new_version: "1.8".into(),
    }];
    let rows = format_outcome_table(&outcomes);
    assert_eq!(rows[0][0], "\u{2191}"); // ↑
}

// ── format_summary ────────────────────────────────────────────────

#[test]
fn summary_contains_keys() {
    let summary = RunSummary {
        total_jars: 42,
        identified: 30,
        unknown: 12,
        updates_available: 5,
        updates_applied: 3,
        up_to_date: 20,
        unavailable: 5,
        skipped: 2,
        failed: 0,
    };
    let out = format_summary(&summary);
    assert!(out.contains("42"));
    assert!(out.contains("RUN SUMMARY"));
}
