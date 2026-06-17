//! JAR file discovery, SHA1 hashing, and batch identification against the
//! Modrinth API.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use futures::stream::{self, StreamExt};
use sha1::{Digest, Sha1};
use tracing;

use crate::error::Error;
use crate::types::{ApiClient, IdentifiedMod};

// ── File Discovery ────────────────────────────────────────────────────────

/// Find all `.jar` files in a directory, sorted alphabetically by filename
/// (case-insensitive).
///
/// Returns an empty `Vec` if the directory does not exist — this is not an
/// error, just an empty result.
pub fn find_jars(dir: &Path) -> crate::Result<Vec<PathBuf>> {
    let mut jars: Vec<PathBuf> = Vec::new();

    let entries = match fs::read_dir(dir) {
        Ok(iter) => iter,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(jars),
        Err(e) => return Err(Error::Io(e)),
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        // Only regular files with a .jar extension (case-insensitive).
        if !path.is_file() {
            continue;
        }

        let is_jar = path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("jar"))
            .unwrap_or(false);

        if is_jar {
            jars.push(path);
        }
    }

    // Sort by filename, case-insensitive, for display consistency.
    jars.sort_by(|a, b| {
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

    Ok(jars)
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
