//! Anvil — Minecraft Mod Updater.
//!
//! Scans a mods folder — plus the sibling `resourcepacks/` and
//! `shaderpacks/` folders when they exist — identifies each file via SHA1
//! hash against the Modrinth API, checks for newer versions matching the
//! same loader and game version, and downloads updates (backing up old
//! files by default).
//!
//! Named after the anvil — Minecraft's repair-and-upgrade block.

pub mod api;
pub mod backup;
pub mod cache;
pub mod cli;
pub mod config;
pub mod deps;
pub mod discover;
pub mod error;
pub mod filters;
pub mod interactive;
#[cfg(feature = "gui")]
pub mod gui;
pub mod locking;
pub mod output;
pub mod paths;
pub mod scanner;
pub mod types;
pub mod updater;

#[cfg(test)]
mod test_utils;

use std::path::PathBuf;

use futures::stream::{self, StreamExt};

use crate::cache::ApiCache;
use crate::config::ResolvedConfig;
use crate::error::Result;
use crate::types::{ApiClient, FilterOpts, IdentifiedMod, ProgressRenderer};

/// List the configured profiles, marking the active one with `*`.
///
/// With `detect`, also prints a second table of mods folders found across the
/// launchers installed on this machine.
///
/// Read-only by signature: there is no write path, so the CLI can never
/// rewrite `config.toml`.
pub fn run_profiles(
    config: &ResolvedConfig,
    detect: bool,
    progress: &dyn ProgressRenderer,
) -> Result<()> {
    if config.profiles.is_empty() {
        tracing::info!(
            "No profiles configured. Add [[profiles]] entries to {}.",
            config.config_path.display()
        );
    } else {
        let headers: &[&str] = &["Name", "Mods Dir", "Exists"];
        let rows: Vec<Vec<String>> = config
            .profiles
            .iter()
            .map(|profile| {
                let active = config.active_profile.as_deref() == Some(profile.name.as_str());
                let is_dir = profile.mods_dir.is_dir();
                let exists = if is_dir { "yes" } else { "no" };
                vec![
                    format!("{}{}", if active { "* " } else { "  " }, profile.name),
                    profile.mods_dir.display().to_string(),
                    exists.to_string(),
                ]
            })
            .collect();
        progress.print_table(headers, &rows);
    }

    tracing::info!("Mods folder in use: {}", config.mods_dir.display());

    if detect {
        let found = discover::discover();
        if found.is_empty() {
            tracing::info!("No launcher mods folders detected.");
        } else {
            let headers: &[&str] = &["Launcher", "Mods Dir", "JARs"];
            let rows: Vec<Vec<String>> = found
                .iter()
                .map(|dir| {
                    vec![
                        dir.launcher.label().to_string(),
                        dir.mods_dir.display().to_string(),
                        dir.jar_count.to_string(),
                    ]
                })
                .collect();
            progress.print_table(headers, &rows);
        }
    }

    Ok(())
}

/// Run the scanner pipeline but stop after identification — print a table
/// of identified mods and exit.  Used by the `list` subcommand.
///
/// When `show_deps` is true, also prints a dependency tree and any
/// missing-required / incompatibility issues. Missing project titles are
/// resolved from Modrinth so the tree shows names, not raw ids.
///
/// When `install_deps` is true (and deps are shown), missing required
/// dependencies are downloaded into the mods folder after an optional
/// confirmation prompt (skipped when `assume_yes`).
pub async fn run_list(
    config: &ResolvedConfig,
    api: &dyn ApiClient,
    cache: &ApiCache,
    progress: &dyn ProgressRenderer,
    show_deps: bool,
    install_deps: bool,
    assume_yes: bool,
) -> Result<()> {
    // 1. Find files (mods, resource packs, shader packs)
    let files = scanner::find_all_files(&config.mods_dir)?;
    if files.is_empty() {
        tracing::info!("No mods, resource packs or shader packs found in {}.", config.mods_dir.display());
        return Ok(());
    }

    // 2. Hash all
    progress.start_phase("Hashing", files.len() as u64);
    let mut hashes: Vec<(PathBuf, String)> = Vec::with_capacity(files.len());
    for file in &files {
        match scanner::compute_sha1(file) {
            Ok(hash) => hashes.push((file.clone(), hash)),
            Err(e) => {
                tracing::warn!(
                    path = %file.display(),
                    error = %e,
                    "skipping file — unable to compute SHA1"
                );
            }
        }
        progress.increment(1);
    }
    progress.finish_phase();

    // 3. Identify (cache-aware, 4-way concurrent)
    progress.start_phase("Identifying", hashes.len() as u64);

    let identify_results: Vec<(PathBuf, String, Option<IdentifiedMod>)> =
        stream::iter(hashes)
            .map(|(path, sha1)| {
                async {
                    if let Ok(Some(version)) = cache.get_version(&sha1) {
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
                        Ok(None) => (path, sha1, None),
                        Err(e) => {
                            tracing::debug!(
                                path = %path.display(),
                                error = %e,
                                "API lookup failed"
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

    let mut identified: Vec<IdentifiedMod> = Vec::new();
    let mut unknown_count = 0usize;

    for (_path, _sha1, result) in identify_results {
        match result {
            Some(mod_info) => identified.push(mod_info),
            None => {
                unknown_count += 1;
            }
        }
    }

    // 4. Apply filters
    let filter_opts = FilterOpts {
        include: config.include.clone(),
        exclude: config.exclude.clone(),
        loader: config.loader.clone(),
        game_version: config.game_version.clone(),
    };

    let before_filter = identified.len();
    let filtered = filters::apply(&identified, &filter_opts);
    let filtered_out = before_filter - filtered.len();

    // 5. Print table: one row per file, mods and packs alike.
    let headers: &[&str] = &["Filename", "Kind", "Name", "Version", "Loader", "Game Versions"];
    let mut rows: Vec<Vec<String>> = Vec::new();

    for m in &filtered {
        let loaders = if m.current_version.loaders.is_empty() {
            String::from("—")
        } else {
            m.current_version.loaders.join(", ")
        };

        let game_versions = if m.current_version.game_versions.is_empty() {
            String::from("—")
        } else {
            m.current_version.game_versions.join(", ")
        };

        rows.push(vec![
            m.filename.clone(),
            scanner::PackKind::of_path(&m.path).label().to_string(),
            m.current_version.name.clone(),
            m.current_version.version_number.clone(),
            loaders,
            game_versions,
        ]);
    }

    if rows.is_empty() {
        tracing::info!("No mods matched the current filters.");
    } else {
        progress.print_table(headers, &rows);
    }

    // 6. Print summary counts
    tracing::info!(
        "Total files: {}  |  Identified: {}  |  Unknown: {}  |  Filtered out: {}  |  Shown: {}",
        files.len(),
        before_filter,
        unknown_count,
        filtered_out,
        filtered.len(),
    );

    // 7. Optional dependency tree over the full identified set (so
    //    missing-required checks see every installed mod, not just the
    //    filter-surviving rows).
    if show_deps {
        let mut graph = deps::build(&identified);

        // Resolve display names for missing targets (e.g. "Placeholder API"
        // instead of the raw Modrinth project id).
        deps::resolve_display_names(&mut graph, api, cache).await;

        println!();
        println!("Dependency tree");
        println!();
        for line in deps::format_tree(&identified, &graph) {
            println!("{}", line);
        }

        println!();
        match deps::format_issues(&graph) {
            Some(issues) => {
                for line in issues {
                    println!("{}", line);
                }
            }
            None => println!("No dependency issues found."),
        }

        // 8. Optionally install missing required dependencies.
        if install_deps && !graph.missing.is_empty() {
            let count = graph.missing.len();
            let question = if count == 1 {
                let miss = &graph.missing[0];
                let label = miss.to_name.as_deref().unwrap_or(&miss.to_project_id);
                format!("Download missing dependency \"{}\"?", label)
            } else {
                format!("Download {} missing dependencies?", count)
            };

            let go = assume_yes || progress.confirm(&question);
            if go {
                println!();
                let outcome = deps::install_missing(
                    &config.mods_dir,
                    &graph,
                    api,
                    cache,
                    config.loader.as_deref(),
                    config.game_version.as_deref(),
                    progress,
                )
                .await?;

                for dep in &outcome.installed {
                    tracing::info!(
                        "Installed {} {} as {}",
                        dep.name, dep.version, dep.filename
                    );
                }
                for (id, reason) in &outcome.failed {
                    tracing::warn!("Failed to install {}: {}", id, reason);
                }

                if !outcome.installed.is_empty() {
                    println!();
                    println!(
                        "Installed {} dependency file(s). Run `anvil list --deps` again to refresh the tree.",
                        outcome.installed.len()
                    );
                }
            } else {
                println!();
                println!("Skipped dependency download.");
            }
        }
    }

    Ok(())
}
