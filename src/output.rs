//! Progress bars, formatted tables, changelog display, and user prompts.
//!
//! Backed by `indicatif` (spinner/bars) and `console` (terminal width).

use std::io::{self, BufRead, Write};
use std::sync::Mutex;

use console::Term;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::types::{ModOutcome, ProgressRenderer, RunSummary};

// ── ConsoleProgress ───────────────────────────────────────────────────────

/// Progress-renderer backed by `indicatif` multi-bars and `console::Term`.
pub struct ConsoleProgress {
    multi: MultiProgress,
    current_bar: Mutex<Option<ProgressBar>>,
    term: Term,
}

impl Default for ConsoleProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleProgress {
    /// Create a new progress renderer for the current terminal.
    pub fn new() -> Self {
        Self {
            multi: MultiProgress::new(),
            current_bar: Mutex::new(None),
            term: Term::stdout(),
        }
    }
}

impl ProgressRenderer for ConsoleProgress {
    fn start_phase(&self, label: &str, total: u64) {
        let style = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}",
        )
        .expect("valid template")
        .progress_chars("=>-");

        let pb = ProgressBar::new(total);
        pb.set_style(style);
        pb.set_message(label.to_string());

        let pb = self.multi.add(pb);

        let mut guard = self.current_bar.lock().expect("lock");
        *guard = Some(pb);
    }

    fn increment(&self, n: u64) {
        let guard = self.current_bar.lock().expect("lock");
        if let Some(ref pb) = *guard {
            pb.inc(n);
        }
    }

    fn finish_phase(&self) {
        let mut guard = self.current_bar.lock().expect("lock");
        if let Some(pb) = guard.take() {
            pb.finish_and_clear();
        }
    }

    fn print_table(&self, headers: &[&str], rows: &[Vec<String>]) {
        if headers.is_empty() {
            return;
        }

        let (term_w, _) = self.term.size();
        let term_width = term_w as usize;
        let num_cols = headers.len();

        // Compute the widest content in each column.
        let mut col_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                if i < num_cols {
                    col_widths[i] = col_widths[i].max(cell.len());
                }
            }
        }

        // Cap each column: share the available width, accounting for
        // 2-space padding between column pairs.
        let total_padding = 2 * num_cols.saturating_sub(1);
        let usable = term_width.saturating_sub(total_padding);
        let max_col = if num_cols > 0 && usable >= num_cols {
            usable / num_cols
        } else {
            10 // fallback when the terminal is impossibly narrow
        };

        for w in &mut col_widths {
            *w = (*w).min(max_col).max(4);
        }

        // Helper: render one row of cells into a line.
        let format_row = |cells: &[&str]| -> String {
            cells
                .iter()
                .enumerate()
                .map(|(i, cell)| {
                    let truncated: String = cell.chars().take(col_widths[i]).collect();
                    // Pad with trailing spaces so columns align.
                    format!("{:width$}", truncated, width = col_widths[i])
                })
                .collect::<Vec<_>>()
                .join("  ")
        };

        // Header
        let header_strs: Vec<&str> = headers.to_vec();
        println!("{}", format_row(&header_strs));

        // Separator line
        let sep: String = col_widths
            .iter()
            .map(|&w| "-".repeat(w))
            .collect::<Vec<_>>()
            .join("  ");
        println!("{}", sep);

        // Data rows
        for row in rows {
            let row_strs: Vec<&str> = row.iter().map(|s| s.as_str()).collect();
            println!("{}", format_row(&row_strs));
        }
    }

    fn print_changelog(&self, slug: &str, version: &str, changelog: &str) {
        println!("── {} v{} ──", slug, version);

        let display = if changelog.len() > 500 {
            let head: String = changelog.chars().take(500).collect();
            format!("{} ... (truncated)", head)
        } else {
            changelog.to_string()
        };

        for line in wrap_lines(&display, 80) {
            println!("{}", line);
        }
    }

    fn confirm(&self, question: &str) -> bool {
        print!("{} [y/N]: ", question);
        let _ = io::stdout().flush();

        let mut line = String::new();
        let stdin = io::stdin();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => false, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                trimmed.starts_with('y') || trimmed.starts_with('Y')
            }
            Err(_) => false,
        }
    }
}

// ── Free functions ────────────────────────────────────────────────────────

/// Format a `RunSummary` into a human-readable block suitable for printing
/// at the end of the run.
pub fn format_summary(summary: &RunSummary) -> String {
    let mut out = String::new();
    out.push_str("═══════════════════════════\n");
    out.push_str("         RUN SUMMARY       \n");
    out.push_str("═══════════════════════════\n");
    out.push_str(&format!(
        "  Total JARs found:  {}\n",
        summary.total_jars
    ));
    out.push_str(&format!(
        "  Identified:        {}  |  Unknown:         {}\n",
        summary.identified, summary.unknown
    ));
    out.push_str(&format!(
        "  Updates available: {}  |  Updates applied: {}\n",
        summary.updates_available, summary.updates_applied
    ));
    out.push_str(&format!(
        "  Up-to-date:        {}  |  Unavailable:     {}\n",
        summary.up_to_date, summary.unavailable
    ));
    out.push_str(&format!(
        "  Skipped:           {}  |  Failed:          {}\n",
        summary.skipped, summary.failed
    ));
    out.push_str("═══════════════════════════\n");
    out
}

/// Convert a slice of `ModOutcome` values into table rows ready for
/// `print_table`.  Each row is `[status_icon, slug_or_filename, old_version,
/// new_version]`.
pub fn format_outcome_table(outcomes: &[ModOutcome]) -> Vec<Vec<String>> {
    outcomes
        .iter()
        .map(|o| match o {
            ModOutcome::Updated {
                slug,
                old_version,
                new_version,
                ..
            } => vec![
                "\u{2191}".to_string(), // ↑
                slug.clone(),
                old_version.clone(),
                new_version.clone(),
            ],
            ModOutcome::UpToDate {
                slug, version, ..
            } => vec![
                "\u{2713}".to_string(), // ✓
                slug.clone(),
                version.clone(),
                String::new(),
            ],
            ModOutcome::Unavailable {
                slug,
                current_version,
                ..
            } => vec![
                "\u{2717}".to_string(), // ✗
                slug.clone(),
                current_version.clone(),
                String::new(),
            ],
            ModOutcome::Unknown { filename } => vec![
                "?".to_string(),
                filename.clone(),
                String::new(),
                String::new(),
            ],
            ModOutcome::FilteredOut { filename, .. } => vec![
                "\u{2298}".to_string(), // ⊘
                filename.clone(),
                String::new(),
                String::new(),
            ],
            ModOutcome::Failed { filename, .. } => vec![
                "!".to_string(),
                filename.clone(),
                String::new(),
                String::new(),
            ],
        })
        .collect()
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Word-wrap `text` at `width` columns. Paragraphs (blank-line separated)
/// are preserved; long words that exceed `width` are hard-broken at the
/// column boundary.
fn wrap_lines(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();

    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            out.push(String::new());
            continue;
        }

        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if line.is_empty() {
                line.push_str(word);
                // If the word itself exceeds width, hard-break it.
                while line.len() > width {
                    let head: String = line.chars().take(width).collect();
                    out.push(head);
                    line = line.chars().skip(width).collect::<String>();
                }
            } else if line.len() + 1 + word.len() <= width {
                line.push(' ');
                line.push_str(word);
            } else {
                // Flush current line.
                out.push(line);
                line = String::from(word);
                // If the word itself exceeds width, hard-break it.
                while line.len() > width {
                    let head: String = line.chars().take(width).collect();
                    out.push(head);
                    line = line.chars().skip(width).collect::<String>();
                }
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
    }

    out
}

#[cfg(test)]
#[path = "tests/test_output.rs"]
mod tests;
