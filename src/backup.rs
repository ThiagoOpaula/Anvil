//! Backup and rollback operations.
//!
//! Backups are timestamped directories created inside the mods folder.
//! Each backed-up JAR is moved atomically via `std::fs::rename` (same
//! filesystem = atomic on all major platforms).

use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Create a timestamped backup directory inside `mods_dir`.
///
/// The directory name follows the pattern `backup_YYYYMMDD_HHMMSS` so
/// lexical sorting by name is also chronological.
///
/// Returns the path to the created directory.
pub fn create_backup_dir(mods_dir: &Path) -> Result<PathBuf> {
    let name = format!("backup_{}", Local::now().format("%Y%m%d_%H%M%S"));
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
/// timestamp format (`YYYYMMDD_HHMMSS`) sorts lexically in chronological
/// order, a reverse sort yields the newest backup first.
///
/// Returns `Error::NoBackups` if no backup directory exists.
pub fn find_latest_backup(mods_dir: &Path) -> Result<PathBuf> {
    let mut backups: Vec<PathBuf> = Vec::new();

    let entries = fs::read_dir(mods_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("backup_") {
                    backups.push(path);
                }
            }
        }
    }

    // Reverse sort: newest (largest timestamp) first.
    backups.sort_by(|a, b| b.cmp(a));

    backups
        .into_iter()
        .next()
        .ok_or(Error::NoBackups)
}

/// Restore all JARs from the latest backup into `mods_dir`.
///
/// Moves each file from the backup directory back into the mods folder,
/// overwriting any existing file with the same name. Returns the number
/// of files successfully restored.
pub fn rollback(mods_dir: &Path) -> Result<usize> {
    let backup_dir = find_latest_backup(mods_dir)?;
    let mut count = 0;

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

