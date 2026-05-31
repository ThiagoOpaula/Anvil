//! Lockfile read/write, state serialisation, and diffing between runs.
//!
//! The lockfile (`anvil.lock`) is a JSON document stored alongside the
//! mods that records the resolved state after each successful run. It enables
//! `--list` to show what changed between two runs.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::types::{IdentifiedMod, LockFile, LockedMod, ModOutcome};

/// Construct the lockfile path for a given mods directory.
pub fn lockfile_path(mods_dir: &Path) -> PathBuf {
    mods_dir.join("anvil.lock")
}

/// Read the existing lockfile, returning `Ok(None)` when no lockfile exists
/// yet (first run, or the file was manually deleted).
pub fn read_lockfile(mods_dir: &Path) -> Result<Option<LockFile>> {
    let path = lockfile_path(mods_dir);

    match fs::read_to_string(&path) {
        Ok(content) => {
            let lockfile: LockFile = serde_json::from_str(&content).map_err(|e| {
                Error::Other(format!("failed to parse lockfile '{}': {}", path.display(), e))
            })?;
            Ok(Some(lockfile))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::Io(e)),
    }
}

/// Write (or overwrite) a lockfile with the current state.
///
/// `current_mods` should reflect the mods now on disk after the run —
/// typically built by [`build_locked_mods`] from the outcomes and
/// identified-mod list.
pub fn write_lockfile(
    mods_dir: &Path,
    target_game_version: Option<&str>,
    target_loader: Option<&str>,
    current_mods: &[LockedMod],
) -> Result<()> {
    let path = lockfile_path(mods_dir);

    let lockfile = LockFile {
        version: 1,
        updated_at: chrono::Local::now().to_rfc3339(),
        target_game_version: target_game_version.map(String::from),
        target_loader: target_loader.map(String::from),
        mods: current_mods.to_vec(),
    };

    let json =
        serde_json::to_string_pretty(&lockfile).map_err(|e| Error::Other(e.to_string()))?;

    fs::write(&path, json)?;
    tracing::debug!("lockfile written: {}", path.display());
    Ok(())
}

/// Build [`LockedMod`] entries from the run outcomes and the original
/// identified-mod list.
///
/// Only "successful" outcomes are included: [`ModOutcome::Updated`],
/// [`ModOutcome::UpToDate`], and [`ModOutcome::Unavailable`].
/// Unknown, filtered-out, and failed mods are skipped.
///
/// Matching between outcomes and identified mods is done by filename.
/// For updated mods the **new** filename and version are recorded.
pub fn build_locked_mods(
    outcomes: &[ModOutcome],
    identified: &[IdentifiedMod],
) -> Vec<LockedMod> {
    let mut result = Vec::new();

    for outcome in outcomes {
        match outcome {
            ModOutcome::Updated {
                slug,
                old_filename,
                new_filename,
                old_version: _,
                new_version,
            } => {
                if let Some(id) = identified.iter().find(|m| m.filename == *old_filename) {
                    result.push(LockedMod {
                        filename: new_filename.clone(),
                        sha1: id.sha1.clone(),
                        project_id: id.current_version.project_id.clone(),
                        slug: slug.clone(),
                        version_id: id.current_version.id.clone(),
                        version_number: new_version.clone(),
                        loaders: id.current_version.loaders.clone(),
                        game_versions: id.current_version.game_versions.clone(),
                    });
                }
            }

            ModOutcome::UpToDate {
                slug,
                filename,
                version,
            } => {
                if let Some(id) = identified.iter().find(|m| m.filename == *filename) {
                    result.push(LockedMod {
                        filename: filename.clone(),
                        sha1: id.sha1.clone(),
                        project_id: id.current_version.project_id.clone(),
                        slug: slug.clone(),
                        version_id: id.current_version.id.clone(),
                        version_number: version.clone(),
                        loaders: id.current_version.loaders.clone(),
                        game_versions: id.current_version.game_versions.clone(),
                    });
                }
            }

            ModOutcome::Unavailable {
                slug,
                filename,
                current_version,
                ..
            } => {
                if let Some(id) = identified.iter().find(|m| m.filename == *filename) {
                    result.push(LockedMod {
                        filename: filename.clone(),
                        sha1: id.sha1.clone(),
                        project_id: id.current_version.project_id.clone(),
                        slug: slug.clone(),
                        version_id: id.current_version.id.clone(),
                        version_number: current_version.clone(),
                        loaders: id.current_version.loaders.clone(),
                        game_versions: id.current_version.game_versions.clone(),
                    });
                }
            }

            // Unknown, FilteredOut, Failed — deliberately excluded.
            _ => {}
        }
    }

    result
}

/// Compare two lockfiles and return a human-readable diff.
///
/// Each line describes one change:
/// - `Added: {slug} v{version}`
/// - `Removed: {slug} v{version}`
/// - `Updated: {slug} {old_version} -> {new_version}`
pub fn diff_lockfile(old: &LockFile, new: &LockFile) -> Vec<String> {
    let mut lines = Vec::new();

    let old_by_slug: HashMap<&str, &LockedMod> =
        old.mods.iter().map(|m| (m.slug.as_str(), m)).collect();
    let new_by_slug: HashMap<&str, &LockedMod> =
        new.mods.iter().map(|m| (m.slug.as_str(), m)).collect();

    // Added: present in new but not in old.
    for (slug, m) in &new_by_slug {
        if !old_by_slug.contains_key(slug) {
            lines.push(format!("Added: {} v{}", slug, m.version_number));
        }
    }

    // Removed: present in old but not in new.
    for (slug, m) in &old_by_slug {
        if !new_by_slug.contains_key(slug) {
            lines.push(format!("Removed: {} v{}", slug, m.version_number));
        }
    }

    // Updated: present in both but version changed.
    for (slug, new_m) in &new_by_slug {
        if let Some(old_m) = old_by_slug.get(slug) {
            if old_m.version_number != new_m.version_number {
                lines.push(format!(
                    "Updated: {} {} -> {}",
                    slug, old_m.version_number, new_m.version_number
                ));
            }
        }
    }

    lines
}
