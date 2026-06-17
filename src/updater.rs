//! Core update orchestration — the main pipeline that ties together all
//! other modules.
//!
//! The pipeline has these phases:
//!
//! 1. Discover JARs          (`scanner::find_jars`)
//! 2. Hash every JAR         (`scanner::compute_sha1` in a loop)
//! 3. Identify on Modrinth   (cache-aware, 4-way parallel)
//! 4. Apply filters          (`filters::apply`)
//! 5. Check for updates      (cache-aware, 4-way parallel)
//! 6. Fetch project metadata (cache-aware, 4-way parallel)
//! 7. Print summary table
//! 8. Dry-run short-circuit
//! 9. Confirmation prompt
//! 10. Download updates      (sequential, with backup)
//! 11. Write lockfile
//! 12. Print final summary

use std::path::PathBuf;

use futures::stream::{self, StreamExt};

use crate::backup;
use crate::cache::ApiCache;
use crate::config::ResolvedConfig;
use crate::error::{Error, Result};
use crate::filters;
use crate::locking;
use crate::output;
use crate::scanner;
use crate::types::{
    ApiClient, DependencyType, FilterOpts, IdentifiedMod, ModOutcome, ProgressRenderer, RunSummary,
    UpdateCandidate,
};

/// Run the full update pipeline. Called from `main()` after config resolution.
pub async fn run(
    config: &ResolvedConfig,
    api: &dyn ApiClient,
    cache: &ApiCache,
    progress: &dyn ProgressRenderer,
) -> Result<RunSummary> {
    // ── 1. Find JARs ─────────────────────────────────────────────────────
    let jars = scanner::find_jars(&config.mods_dir)?;
    if jars.is_empty() {
        tracing::info!(
            "no JAR files found in {} — nothing to do",
            config.mods_dir.display()
        );
        return Ok(RunSummary::default());
    }
    let total_jars = jars.len();
    tracing::info!("found {} JAR(s) in {}", total_jars, config.mods_dir.display());

    // ── 2. Hash all JARs ─────────────────────────────────────────────────
    progress.start_phase("Hashing", total_jars as u64);
    let mut hashes: Vec<(PathBuf, String)> = Vec::with_capacity(total_jars);
    for jar in &jars {
        match scanner::compute_sha1(jar) {
            Ok(hash) => hashes.push((jar.clone(), hash)),
            Err(e) => {
                tracing::warn!(
                    path = %jar.display(),
                    error = %e,
                    "skipping JAR — unable to compute SHA1"
                );
            }
        }
        progress.increment(1);
    }
    progress.finish_phase();

    let hashed_count = hashes.len();

    // ── 3. Identify on Modrinth (cache-aware, 4-way concurrent) ─────────
    //
    // Each task emits `(path, sha1, Option<IdentifiedMod>)` so we can
    // reconstruct unknown filenames after the stream completes.
    if progress.is_cancelled() {
        return Err(Error::Cancelled);
    }
    progress.start_phase("Identifying", hashed_count as u64);

    let identify_results: Vec<(PathBuf, String, Option<IdentifiedMod>)> =
        stream::iter(hashes)
            .map(|(path, sha1)| {
                async {
                    // ── check cache first ──────────────────────────────
                    if let Ok(Some(version)) = cache.get_version(&sha1) {
                        tracing::debug!(sha1 = %sha1, "version cache hit");
                        let filename = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let entry = IdentifiedMod {
                            path: path.clone(),
                            sha1: sha1.clone(),
                            filename,
                            current_version: version,
                        };
                        return (path, sha1, Some(entry));
                    }

                    // ── cache miss — hit the API ───────────────────────
                    match api.get_version_from_hash(&sha1).await {
                        Ok(Some(version)) => {
                            let _ = cache.set_version(&sha1, &version);
                            let filename = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            let entry = IdentifiedMod {
                                path: path.clone(),
                                sha1: sha1.clone(),
                                filename,
                                current_version: version,
                            };
                            (path, sha1, Some(entry))
                        }
                        Ok(None) => {
                            // 404 — mod not on Modrinth.
                            (path, sha1, None)
                        }
                        Err(e) => {
                            tracing::debug!(
                                path = %path.display(),
                                error = %e,
                                "API lookup failed — treating as unknown"
                            );
                            (path, sha1, None)
                        }
                    }
                }
            })
            .buffer_unordered(4)
            .inspect(|_| progress.increment(1))
            .collect()
            .await;

    progress.finish_phase();

    // Split results into identified and unknown.
    let mut identified: Vec<IdentifiedMod> = Vec::new();
    let mut unknown_outcomes: Vec<ModOutcome> = Vec::new();

    for (path, _sha1, result) in identify_results {
        match result {
            Some(mod_info) => identified.push(mod_info),
            None => {
                let filename = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                tracing::info!(
                    filename = %filename,
                    "mod not found on Modrinth"
                );
                unknown_outcomes.push(ModOutcome::Unknown { filename });
            }
        }
    }

    let identified_count = identified.len();
    let unknown_count = unknown_outcomes.len();

    // ── 4. Apply filters ─────────────────────────────────────────────────
    let filter_opts = FilterOpts {
        include: config.include.clone(),
        exclude: config.exclude.clone(),
        loader: config.loader.clone(),
        game_version: None, // Target sent to the API in Phase 5, not used as a pre-filter.
    };

    let before_filter = identified.len();
    let filtered = filters::apply(&identified, &filter_opts);
    let filtered_out_count = before_filter - filtered.len();

    if filtered_out_count > 0 {
        tracing::info!("filtered out {} mod(s)", filtered_out_count);
    }

    // Record filtered-out outcomes.
    let filtered_filenames: std::collections::HashSet<String> = filtered
        .iter()
        .map(|m| m.filename.clone())
        .collect();

    let filtered_outcomes: Vec<ModOutcome> = identified
        .iter()
        .filter(|m| !filtered_filenames.contains(&m.filename))
        .map(|m| ModOutcome::FilteredOut {
            filename: m.filename.clone(),
            reason: "did not match include/exclude/loader/game-version filters".into(),
        })
        .collect();

    // ── 5. Check for updates (parallel, 4 concurrent) ────────────────────
    if progress.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let check_results: Vec<(ModOutcome, Option<UpdateCandidate>)> = stream::iter(&filtered)
        .map(|m| {
            async {
                // Build loader list: use config override, else auto-detect.
                let loaders: Vec<String> = config
                    .loader
                    .as_ref()
                    .map(|l| vec![l.clone()])
                    .unwrap_or_else(|| m.current_version.loaders.clone());

                // Build game-version list: use config override, else auto-detect.
                let game_versions: Vec<String> = config
                    .game_version
                    .as_ref()
                    .map(|gv| vec![gv.clone()])
                    .unwrap_or_else(|| m.current_version.game_versions.clone());

                // Check cache first.
                let latest = if let Ok(Some(v)) =
                    cache.get_update(&m.sha1, &loaders, &game_versions)
                {
                    tracing::debug!(
                        sha1 = %m.sha1,
                        "update cache hit"
                    );
                    Some(v)
                } else {
                    match api
                        .get_latest_version(&m.sha1, &loaders, &game_versions)
                        .await
                    {
                        Ok(Some(v)) => {
                            let _ = cache.set_update(&m.sha1, &loaders, &game_versions, &v);
                            Some(v)
                        }
                        Ok(None) => None,
                        Err(e) => {
                            tracing::warn!(
                                filename = %m.filename,
                                error = %e,
                                "failed to check for updates — assuming up-to-date"
                            );
                            None
                        }
                    }
                };

                match latest {
                    Some(latest_version) if latest_version.id != m.current_version.id => {
                        // Update available.
                        (
                            ModOutcome::UpToDate {
                                slug: String::new(),
                                filename: String::new(),
                                version: String::new(),
                            }, // placeholder — we use the candidate, not this outcome
                            Some(UpdateCandidate {
                                identified: m.clone(),
                                latest_version,
                                project: None,
                            }),
                        )
                    }
                    Some(_) => {
                        // Up to date.
                        (
                            ModOutcome::UpToDate {
                                slug: m.current_version.name.clone(),
                                filename: m.filename.clone(),
                                version: m.current_version.version_number.clone(),
                            },
                            None,
                        )
                    }
                    None if config.game_version.is_some() => {
                        // Explicit game version requested but no matching version exists.
                        (
                            ModOutcome::Unavailable {
                                slug: m.current_version.name.clone(),
                                filename: m.filename.clone(),
                                current_version: m.current_version.version_number.clone(),
                                game_version: config
                                    .game_version
                                    .clone()
                                    .unwrap_or_default(),
                            },
                            None,
                        )
                    }
                    None => {
                        // No game version specified — API returned nothing;
                        // treat as up-to-date (the current version is the best we have).
                        (
                            ModOutcome::UpToDate {
                                slug: m.current_version.name.clone(),
                                filename: m.filename.clone(),
                                version: m.current_version.version_number.clone(),
                            },
                            None,
                        )
                    }
                }
            }
        })
        .buffer_unordered(4)
        .collect()
        .await;

    // Split results.
    let mut up_to_date_outcomes: Vec<ModOutcome> = Vec::new();
    let mut unavailable_outcomes: Vec<ModOutcome> = Vec::new();
    let mut candidates: Vec<UpdateCandidate> = Vec::new();

    for (outcome, candidate) in check_results {
        match candidate {
            Some(c) => candidates.push(c),
            None => match outcome {
                ModOutcome::UpToDate { .. } => up_to_date_outcomes.push(outcome),
                ModOutcome::Unavailable { .. } => unavailable_outcomes.push(outcome),
                _ => {} // shouldn't happen
            },
        }
    }

    let updates_available = candidates.len();

    // ── 6. Fetch project metadata (parallel, 4 concurrent) ───────────────
    if progress.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let candidates: Vec<UpdateCandidate> = stream::iter(candidates)
        .map(|mut candidate| {
            async {
                let project_id = candidate.latest_version.project_id.clone();

                // Check cache first.
                let project = if let Ok(Some(p)) = cache.get_project(&project_id) {
                    tracing::debug!(project_id = %project_id, "project cache hit");
                    Some(p)
                } else {
                    match api.get_project(&project_id).await {
                        Ok(p) => {
                            let _ = cache.set_project(&project_id, &p);
                            Some(p)
                        }
                        Err(e) => {
                            tracing::warn!(
                                project_id = %project_id,
                                error = %e,
                                "failed to fetch project metadata"
                            );
                            None
                        }
                    }
                };

                // Warn if the project has a problematic status.
                if let Some(ref p) = project
                    && p.status.is_problematic() {
                        tracing::warn!(
                            slug = %p.slug,
                            status = ?p.status,
                            "mod has problematic status — consider removing"
                        );
                    }

                // Check dependencies for incompatibilities with installed mods.
                for dep in &candidate.latest_version.dependencies {
                    if dep.dependency_type == DependencyType::Incompatible
                        && let Some(ref dep_project_id) = dep.project_id {
                            let is_installed = filtered
                                .iter()
                                .any(|m| m.current_version.project_id == *dep_project_id);
                            if is_installed {
                                tracing::warn!(
                                    mod_name = %candidate.identified.current_version.name,
                                    incompatible_project = %dep_project_id,
                                    "update introduces an incompatibility with an installed mod"
                                );
                            }
                        }
                }

                candidate.project = project;
                candidate
            }
        })
        .buffer_unordered(4)
        .collect()
        .await;

    // ── 7. Print summary table ───────────────────────────────────────────
    let temp_updated_outcomes: Vec<ModOutcome> = candidates
        .iter()
        .map(|c| {
            let slug = c
                .project
                .as_ref()
                .map(|p| p.slug.clone())
                .unwrap_or_else(|| c.identified.current_version.name.clone());
            let new_filename = c
                .latest_version
                .files
                .first()
                .map(|f| f.filename.clone())
                .unwrap_or_default();
            ModOutcome::Updated {
                slug,
                old_filename: c.identified.filename.clone(),
                new_filename,
                old_version: c.identified.current_version.version_number.clone(),
                new_version: c.latest_version.version_number.clone(),
            }
        })
        .collect();

    let mut table_outcomes: Vec<ModOutcome> = Vec::new();
    table_outcomes.extend(temp_updated_outcomes);
    table_outcomes.extend(up_to_date_outcomes.clone());
    table_outcomes.extend(unavailable_outcomes.clone());
    table_outcomes.extend(unknown_outcomes.clone());
    table_outcomes.extend(filtered_outcomes.clone());

    progress.report_outcomes(&table_outcomes);

    let headers: &[&str] = &["Status", "Mod", "Current", "Latest"];
    let rows = output::format_outcome_table(&table_outcomes);
    progress.print_table(headers, &rows);

    // ── 8. Dry-run short-circuit ─────────────────────────────────────────
    if config.dry_run {
        tracing::info!("dry run — no files changed.");
        let summary = RunSummary {
            total_jars,
            identified: identified_count,
            unknown: unknown_count,
            updates_available,
            updates_applied: 0,
            up_to_date: up_to_date_outcomes.len(),
            unavailable: unavailable_outcomes.len(),
            skipped: filtered_out_count,
            failed: 0,
        };
        println!("{}", output::format_summary(&summary));
        return Ok(summary);
    }

    // ── 9. Confirmation prompt ───────────────────────────────────────────
    if candidates.is_empty() {
        tracing::info!("no updates available.");
        let summary = RunSummary {
            total_jars,
            identified: identified_count,
            unknown: unknown_count,
            updates_available: 0,
            updates_applied: 0,
            up_to_date: up_to_date_outcomes.len(),
            unavailable: unavailable_outcomes.len(),
            skipped: filtered_out_count,
            failed: 0,
        };
        println!("{}", output::format_summary(&summary));
        return Ok(summary);
    }

    if config.confirm {
        let question = format!("Download {} update(s)?", candidates.len());
        if !progress.confirm(&question) {
            tracing::info!("cancelled by user.");
            return Err(Error::Cancelled);
        }
    }

    // ── 10. Download updates (sequential) ────────────────────────────────
    if progress.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let max_updates = config.max_updates.unwrap_or(usize::MAX);
    let to_download: Vec<&UpdateCandidate> =
        candidates.iter().take(max_updates).collect();

    let mut backup_dir: Option<PathBuf> = None;
    let mut updated_outcomes: Vec<ModOutcome> = Vec::new();
    let mut failed_outcomes: Vec<ModOutcome> = Vec::new();

    for candidate in &to_download {
        // Pick the primary file, or fall back to the first file.
        let file = candidate
            .latest_version
            .files
            .iter()
            .find(|f| f.primary)
            .or_else(|| candidate.latest_version.files.first())
            .ok_or_else(|| {
                Error::Other(format!(
                    "no files attached to version {} of {}",
                    candidate.latest_version.version_number,
                    candidate.identified.filename
                ))
            })?;

        let dest_path = config.mods_dir.join(&file.filename);

        // Create backup directory on first download.
        if config.backup && backup_dir.is_none() {
            let gv = config.game_version.as_deref().unwrap_or("");
            backup_dir = Some(backup::create_backup_dir(&config.mods_dir, gv)?);
        }

        // Move (or delete) the old JAR.
        if let Some(ref bd) = backup_dir {
            backup::move_to_backup(&candidate.identified.path, bd)?;
        } else {
            // No backup — just remove the old file so we don't leave
            // stale JARs around when filenames change.
            if candidate.identified.path.exists()
                && let Err(e) = std::fs::remove_file(&candidate.identified.path) {
                    tracing::warn!(
                        path = %candidate.identified.path.display(),
                        error = %e,
                        "failed to remove old JAR (backup disabled)"
                    );
                }
        }

        // Download the new file.
        let progress_cb = |downloaded: u64, total: Option<u64>| {
            if let Some(t) = total {
                tracing::debug!("download: {} / {} bytes", downloaded, t);
            } else {
                tracing::debug!("download: {} bytes", downloaded);
            }
        };

        match api
            .download_file(&file.url, &dest_path, &progress_cb)
            .await
        {
            Ok(actual_sha1) if actual_sha1 == file.hashes.sha1 => {
                // Success — SHA1 matches.
                let slug = candidate
                    .project
                    .as_ref()
                    .map(|p| p.slug.clone())
                    .unwrap_or_else(|| {
                        candidate.identified.current_version.name.clone()
                    });

                tracing::info!(
                    slug = %slug,
                    old = %candidate.identified.current_version.version_number,
                    new = %candidate.latest_version.version_number,
                    "updated"
                );

                updated_outcomes.push(ModOutcome::Updated {
                    slug: slug.clone(),
                    old_filename: candidate.identified.filename.clone(),
                    new_filename: file.filename.clone(),
                    old_version: candidate
                        .identified
                        .current_version
                        .version_number
                        .clone(),
                    new_version: candidate.latest_version.version_number.clone(),
                });

                // Print changelog if requested.
                if config.changelog
                    && let Some(ref changelog) = candidate.latest_version.changelog
                        && !changelog.is_empty() {
                            progress.print_changelog(
                                &slug,
                                &candidate.latest_version.version_number,
                                changelog,
                            );
                        }
            }
            Ok(actual_sha1) => {
                // SHA1 mismatch — corrupted download.
                let _ = std::fs::remove_file(&dest_path);

                // Restore old file from backup if we have one.
                if let Some(ref bd) = backup_dir {
                    let backup_path = bd.join(&candidate.identified.filename);
                    if backup_path.exists() {
                        let _ = std::fs::rename(&backup_path, &candidate.identified.path);
                    }
                }

                let error_msg = format!(
                    "SHA1 mismatch: expected {}, got {}",
                    file.hashes.sha1, actual_sha1
                );
                tracing::error!(
                    filename = %candidate.identified.filename,
                    "{}",
                    error_msg
                );

                failed_outcomes.push(ModOutcome::Failed {
                    filename: candidate.identified.filename.clone(),
                    error: error_msg,
                });
            }
            Err(e) => {
                // Download failed — clean up partial file.
                let _ = std::fs::remove_file(&dest_path);

                // Restore old file from backup.
                if let Some(ref bd) = backup_dir {
                    let backup_path = bd.join(&candidate.identified.filename);
                    if backup_path.exists() {
                        let _ = std::fs::rename(&backup_path, &candidate.identified.path);
                    }
                }

                tracing::error!(
                    filename = %candidate.identified.filename,
                    error = %e,
                    "download failed"
                );

                failed_outcomes.push(ModOutcome::Failed {
                    filename: candidate.identified.filename.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    let updates_applied = updated_outcomes.len();
    let failed = failed_outcomes.len();

    // ── 11. Write lockfile ───────────────────────────────────────────────
    let all_successful: Vec<ModOutcome> = updated_outcomes
        .iter()
        .cloned()
        .chain(up_to_date_outcomes.iter().cloned())
        .chain(unavailable_outcomes.iter().cloned())
        .collect();

    let locked_mods = locking::build_locked_mods(&all_successful, &filtered);
    locking::write_lockfile(
        config.game_version.as_deref(),
        config.loader.as_deref(),
        &locked_mods,
    )?;

    // ── 12. Print final summary ──────────────────────────────────────────
    let summary = RunSummary {
        total_jars,
        identified: identified_count,
        unknown: unknown_count,
        updates_available,
        updates_applied,
        up_to_date: up_to_date_outcomes.len(),
        unavailable: unavailable_outcomes.len(),
        skipped: filtered_out_count,
        failed,
    };

    println!("{}", output::format_summary(&summary));
    Ok(summary)
}
