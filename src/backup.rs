//! Backup and rollback operations.
//!
//! Backups are timestamped directories created inside the mods folder.
//! Each backed-up JAR is moved atomically via `std::fs::rename` (same
//! filesystem = atomic on all major platforms).
//!
//! Backup directory naming: `backup_DD-MM-YYYY_mc{VERSION}`
//! Example: `backup_17-06-2026_mc1.21.1`

use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Create a timestamped backup directory inside `mods_dir`.
///
/// The directory name follows the pattern `backup_DD-MM-YYYY_mc{VERSION}`
/// so the user can see at a glance when the backup was made and for which
/// Minecraft version. When `game_version` is empty, `"auto"` is used.
///
/// Returns the path to the created directory.
pub fn create_backup_dir(mods_dir: &Path, game_version: &str) -> Result<PathBuf> {
    let version_slug = if game_version.is_empty() {
        "auto"
    } else {
        game_version
    };
    let name = format!(
        "backup_{}_mc{}",
        Local::now().format("%d-%m-%Y"),
        version_slug
    );
    let path = mods_dir.join(&name);
    fs::create_dir_all(&path)?;
    tracing::info!("created backup directory: {}", path.display());
    Ok(path)
}

/// Move a JAR file into the backup directory.
///
/// Uses `std::fs::rename` — atomic when source and destination are on
/// the same filesystem, which is always the case here since the backup
/// directory lives inside the same mods folder.
pub fn move_to_backup(jar: &Path, backup_dir: &Path) -> Result<()> {
    let filename = jar
        .file_name()
        .ok_or_else(|| Error::Other("jar path has no filename".into()))?;

    let dest = backup_dir.join(filename);
    fs::rename(jar, &dest)?;
    tracing::debug!("backed up: {} -> {}", jar.display(), dest.display());
    Ok(())
}

/// Find the most recent backup directory in `mods_dir`.
///
/// Scans for directories whose name starts with `backup_`. Because the
/// timestamp format (`DD-MM-YYYY`) sorts lexically by day first, newer
/// backups may not always sort last. We sort by directory modification
/// time to reliably find the newest.
///
/// Returns `Error::NoBackups` if no backup directory exists.
pub fn find_latest_backup(mods_dir: &Path) -> Result<PathBuf> {
    let mut backups: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

    let entries = fs::read_dir(mods_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
            && name.starts_with("backup_")
        {
            if let Ok(meta) = path.metadata() {
                if let Ok(mod_time) = meta.modified() {
                    backups.push((path, mod_time));
                }
            }
        }
    }

    // Sort by modification time descending — newest first.
    backups.sort_by(|a, b| b.1.cmp(&a.1));

    backups
        .into_iter()
        .next()
        .map(|(p, _)| p)
        .ok_or(Error::NoBackups)
}

/// Restore all JARs from the latest backup into `mods_dir`.
///
/// **Before restoring**, the current JAR files in the mods directory are
/// moved into a safety backup (`backup_before_rollback_DD-MM-YYYY_HHMMSS`)
/// so nothing is permanently lost.
///
/// Returns the number of files successfully restored.
pub fn rollback(mods_dir: &Path) -> Result<usize> {
    let backup_dir = find_latest_backup(mods_dir)?;

    // ── Safety backup: save current mods before overwriting ──────────
    let safety_name = format!(
        "backup_before_rollback_{}",
        Local::now().format("%d-%m-%Y_%H%M%S")
    );
    let safety_dir = mods_dir.join(&safety_name);
    fs::create_dir_all(&safety_dir)?;

    let mut safety_count = 0usize;
    for entry in fs::read_dir(mods_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .map(|e| e == "jar")
                .unwrap_or(false)
        {
            if let Some(name) = path.file_name() {
                fs::rename(&path, safety_dir.join(name))?;
                safety_count += 1;
            }
        }
    }
    if safety_count > 0 {
        tracing::info!(
            "safety backup: moved {safety_count} JAR(s) to {}",
            safety_dir.display()
        );
    }

    // ── Restore from the selected backup ─────────────────────────────
    let mut count = 0usize;
    for entry in fs::read_dir(&backup_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let filename = path
                .file_name()
                .ok_or_else(|| Error::Other("file in backup has no filename".into()))?;

            let dest = mods_dir.join(filename);
            fs::rename(&path, &dest)?;
            tracing::info!("restored: {}", dest.display());
            count += 1;
        }
    }

    Ok(count)
}
