//! Pure helper functions extracted from `app.rs` for testability.
//!
//! These functions have no dependencies on `AnvilApp` state or egui
//! and can be tested with plain `#[test]` functions.

use crate::types::{ImportedMod, ModOutcome};

// ── Instance / profile helpers ────────────────────────────────────────────

/// The instance/profile folder that owns a mods dir, if it is informative.
///
/// `…/instances/spark/minecraft/mods` → `spark`; `~/.minecraft/mods` → `None`
/// (the official launcher has no instance folder there).
pub fn instance_label(mods_dir: &std::path::Path) -> Option<String> {
    let parent = mods_dir.parent()?;
    let parent_name = parent.file_name()?.to_string_lossy().to_lowercase();

    let candidate = match parent_name.as_str() {
        "minecraft" | ".minecraft" => parent
            .parent()?
            .file_name()?
            .to_string_lossy()
            .to_lowercase(),
        _ => parent_name,
    };

    // `~/.minecraft/mods` would otherwise be named after the user's home dir.
    let is_home_dir = dirs::home_dir()
        .and_then(|home| home.file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .is_some_and(|home| home == candidate);

    if candidate == "mods" || candidate.starts_with('.') || is_home_dir {
        None
    } else {
        Some(candidate)
    }
}

// ── String helpers ────────────────────────────────────────────────────────

/// Lower-case, TOML-friendly form of a folder name.
pub fn slugify(text: &str) -> String {
    let slug: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    slug.trim_matches('-').to_string()
}

/// Comma-separated patterns → `Some(list)`, or `None` when the field is blank
/// (so clearing the field removes the key instead of writing an empty list).
pub fn split_patterns(text: &str) -> Option<Vec<String>> {
    if text.trim().is_empty() {
        return None;
    }
    Some(
        text.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    )
}

// ── Outcome display helpers ───────────────────────────────────────────────

/// The display label for an outcome (mod name, slug, or filename).
pub fn outcome_label(outcome: &ModOutcome) -> &str {
    match outcome {
        ModOutcome::Updated { slug, .. }
        | ModOutcome::UpToDate { slug, .. }
        | ModOutcome::Unavailable { slug, .. } => slug,
        ModOutcome::Unknown { filename } => filename,
        ModOutcome::FilteredOut { filename, .. } => filename,
        ModOutcome::Failed { filename, .. } => filename,
    }
}

/// The current version string for an outcome (or em-dash if N/A).
pub fn outcome_current(outcome: &ModOutcome) -> &str {
    match outcome {
        ModOutcome::Updated { old_version, .. } => old_version,
        ModOutcome::UpToDate { version, .. } => version,
        ModOutcome::Unavailable {
            current_version, ..
        } => current_version,
        _ => "\u{2014}",
    }
}

/// The latest version string for an outcome (or em-dash if N/A).
pub fn outcome_latest(outcome: &ModOutcome) -> &str {
    match outcome {
        ModOutcome::Updated { new_version, .. } => new_version,
        ModOutcome::UpToDate { version, .. } => version,
        _ => "\u{2014}",
    }
}

// ── Import helpers ────────────────────────────────────────────────────────

/// Parse an imported CSV file with header row and project_id column.
///
/// Expected format (matching what export generates):
/// `Filename,Name,Version,Project ID,Loader,Game Versions`
/// Each row is parsed, the Project ID column is required.
pub fn parse_imported_csv(content: &str) -> Vec<ImportedMod> {
    let mut mods = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue; // Skip header and blank lines
        }
        // Simple comma split — handles basic CSV without quoting (the export
        // format quotes everything, but for import we accept both).
        let fields: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
        if fields.len() >= 4 {
            let project_id = fields[3].to_string();
            if !project_id.is_empty() {
                mods.push(ImportedMod {
                    project_id,
                    slug: None,
                    name: if fields.len() > 1 && !fields[1].is_empty() {
                        Some(fields[1].to_string())
                    } else {
                        None
                    },
                    version_number: if fields.len() > 2 && !fields[2].is_empty() {
                        Some(fields[2].to_string())
                    } else {
                        None
                    },
                    loaders: if fields.len() > 4 && !fields[4].is_empty() {
                        fields[4].split(", ").map(|s| s.to_string()).collect()
                    } else {
                        vec![]
                    },
                    game_versions: if fields.len() > 5 && !fields[5].is_empty() {
                        fields[5].split(", ").map(|s| s.to_string()).collect()
                    } else {
                        vec![]
                    },
                });
            }
        }
    }
    mods
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "tests/test_gui_util.rs"]
mod tests;
