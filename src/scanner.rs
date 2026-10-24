//! Content discovery, SHA1 hashing, and batch identification against the
//! Modrinth API.
//!
//! Modrinth identifies files by hash regardless of what they are, so mods,
//! resource packs and shader packs all flow through the same pipeline. The
//! only differences are the folder each lives in and its extension.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use futures::stream::{self, StreamExt};
use sha1::{Digest, Sha1};
use tracing;

use crate::error::Error;
use crate::types::{ApiClient, IdentifiedMod};

// ── Content kinds ─────────────────────────────────────────────────────────

/// A kind of Minecraft content Anvil can identify and update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackKind {
    /// A mod: `.jar` in `mods/`.
    Mod,
    /// A resource pack: `.zip` in `resourcepacks/`.
    ResourcePack,
    /// A shader pack: `.zip` in `shaderpacks/`.
    ShaderPack,
}

impl PackKind {
    /// File extension without the leading dot.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mod => "jar",
            Self::ResourcePack | Self::ShaderPack => "zip",
        }
    }

    /// Folder name, relative to the Minecraft instance root.
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Mod => "mods",
            Self::ResourcePack => "resourcepacks",
            Self::ShaderPack => "shaderpacks",
        }
    }

    /// Human-readable label for tables and logs.
    pub fn label(self) -> &'static str {
        match self {
            Self::Mod => "Mod",
            Self::ResourcePack => "Resource pack",
            Self::ShaderPack => "Shader pack",
        }
    }

    /// Classify a file by the folder it sits in.
    pub fn of_path(path: &Path) -> Self {
        let in_dir = |kind: Self| {
            path.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(kind.dir_name()))
        };

        if in_dir(Self::ShaderPack) {
            Self::ShaderPack
        } else if in_dir(Self::ResourcePack) {
            Self::ResourcePack
        } else {
            Self::Mod
        }
    }
}

/// A folder to scan together with the kind of content it holds.
#[derive(Debug, Clone)]
pub struct ScanTarget {
    pub kind: PackKind,
    pub dir: PathBuf,
}

/// Folders to scan for a given `mods_dir`.
///
/// Always contains `mods_dir` itself. Launchers keep `resourcepacks/` and
/// `shaderpacks/` as siblings of `mods/` under one instance root, so those
/// are added when they exist.
pub fn scan_targets(mods_dir: &Path) -> Vec<ScanTarget> {
    let mut targets = vec![ScanTarget {
        kind: PackKind::Mod,
        dir: mods_dir.to_path_buf(),
    }];

    if let Some(instance_root) = mods_dir.parent() {
        for kind in [PackKind::ResourcePack, PackKind::ShaderPack] {
            let dir = instance_root.join(kind.dir_name());
            if dir.is_dir() {
                targets.push(ScanTarget { kind, dir });
            }
        }
    }

    targets
}

// ── File Discovery ────────────────────────────────────────────────────────

/// Find all files of `kind` in a directory, sorted alphabetically by
/// filename (case-insensitive).
///
/// Returns an empty `Vec` if the directory does not exist — this is not an
/// error, just an empty result.
pub fn find_files(dir: &Path, kind: PackKind) -> crate::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = Vec::new();

    let entries = match fs::read_dir(dir) {
        Ok(iter) => iter,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(files),
        Err(e) => return Err(Error::Io(e)),
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        // Only regular files with this kind's extension (case-insensitive).
        if !path.is_file() {
            continue;
        }

        let matches_extension = path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case(kind.extension()))
            .unwrap_or(false);

        if matches_extension {
            files.push(path);
        }
    }

    files.sort_by(|a, b| {
        let a_name = a
            .file_name()
            .map(|n| n.to_ascii_lowercase())
            .unwrap_or_default();
        let b_name = b
            .file_name()
            .map(|n| n.to_ascii_lowercase())
            .unwrap_or_default();
        a_name.cmp(&b_name)
    });

    Ok(files)
}

/// Every file Anvil can identify under `mods_dir` and its sibling pack
/// folders, mods first.
pub fn find_all_files(mods_dir: &Path) -> crate::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for target in scan_targets(mods_dir) {
        files.extend(find_files(&target.dir, target.kind)?);
    }
    Ok(files)
}

// ── Hashing ───────────────────────────────────────────────────────────────

/// Compute the SHA1 hex digest of a file, reading in 64 KiB chunks.
pub fn compute_sha1(path: &Path) -> crate::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 64 * 1024]; // 64 KiB

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    let digest = hasher.finalize();
    Ok(hex::encode(digest))
}

/// Hash all discovered JARs sequentially (CPU-bound, not worth
/// parallelising). Skips files that cannot be read, logging a warning.
pub fn hash_all(jars: &[PathBuf]) -> Vec<(PathBuf, String)> {
    jars.iter()
        .filter_map(|path| match compute_sha1(path) {
            Ok(hash) => Some((path.clone(), hash)),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "skipping JAR — unable to compute SHA1"
                );
                None
            }
        })
        .collect()
}

// ── Batch Identification ──────────────────────────────────────────────────

/// Batch-identify JARs against the Modrinth API.
///
/// Takes (path, sha1) pairs and an `ApiClient` trait object. Runs up to 4
/// concurrent lookups via `buffer_unordered`. Returns a `Vec` in the same
/// order as the input, with `None` for mods that could not be identified
/// (not on Modrinth, or an API error).
pub async fn batch_identify(
    hashes: &[(PathBuf, String)],
    api: &dyn ApiClient,
) -> Vec<Option<IdentifiedMod>> {
    stream::iter(hashes)
        .map(|(path, sha1)| async move {
            match api.get_version_from_hash(sha1).await {
                Ok(Some(version)) => {
                    let filename = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();

                    Some(IdentifiedMod {
                        path: path.clone(),
                        sha1: sha1.clone(),
                        filename,
                        current_version: version,
                    })
                }
                Ok(None) => {
                    tracing::debug!(
                        path = %path.display(),
                        sha1 = %sha1,
                        "mod not found on Modrinth (404)"
                    );
                    None
                }
                Err(e) => {
                    tracing::debug!(
                        path = %path.display(),
                        sha1 = %sha1,
                        error = %e,
                        "skipping mod — API lookup failed"
                    );
                    None
                }
            }
        })
        .buffer_unordered(4)
        .collect()
        .await
}
