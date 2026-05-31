//! Anvil — Minecraft Mod Updater.
//!
//! Scans a mods folder, identifies each JAR via SHA1 hash against the
//! Modrinth API, checks for newer versions matching the same loader and
//! game version, and downloads updates (backing up old files by default).
//!
//! Named after the anvil — Minecraft's repair-and-upgrade block.

mod api;
mod backup;
mod cache;
mod cli;
mod config;
mod error;
mod filters;
mod locking;
mod output;
mod paths;
mod scanner;
mod types;
mod updater;

use std::path::PathBuf;

use clap::Parser;
use futures::stream::{self, StreamExt};
use tracing_subscriber::EnvFilter;

use crate::cache::ApiCache;
use crate::cli::{Cli, Command};
use crate::config::ResolvedConfig;
use crate::error::{Error, Result};
use crate::types::{ApiClient, FilterOpts, IdentifiedMod, ProgressRenderer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── 1. Parse CLI (needed early for log-level hints) ──────────────────
    let cli = Cli::parse();

    // ── 2. Set up tracing subscriber ─────────────────────────────────────
    let default_level = match &cli.command {
        Some(Command::Update(args)) if args.common.verbose => "mod_updater=debug",
        Some(Command::Update(args)) if args.common.quiet => "mod_updater=error",
        Some(Command::List(args)) if args.common.verbose => "mod_updater=debug",
        Some(Command::List(args)) if args.common.quiet => "mod_updater=error",
        _ => "mod_updater=info",
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_level(true)
        .init();

    // ── 3. Load config file ──────────────────────────────────────────────
    let config_path = paths::config_dir().join("config.toml");
    let resolved = config::resolve(&cli, Some(&config_path))?;

    // ── 4. Build API client, cache, and progress renderer ────────────────
    let api = api::ModrinthApi::new()?;
    let cache = cache::ApiCache::new();
    let progress = output::ConsoleProgress::new();

    // ── 5. Dispatch on subcommand ────────────────────────────────────────
    match cli.command {
        Some(Command::Rollback) => {
            match backup::rollback(&resolved.mods_dir) {
                Ok(count) => {
                    println!("Restored {} mod(s) from backup.", count);
                }
                Err(Error::NoBackups) => {
                    eprintln!("No backups found in {}.", resolved.mods_dir.display());
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Rollback failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Some(Command::List(_)) => {
            run_list(&resolved, &api, &cache, &progress).await?;
        }

        Some(Command::Update(_)) | None => {
            match updater::run(&resolved, &api, &cache, &progress).await {
                Ok(_summary) => {
                    // Summary is already printed by updater::run.
                }
                Err(Error::Cancelled) => {
                    tracing::info!("operation cancelled.");
                }
                Err(e) => {
                    tracing::error!("update failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}

/// Run the scanner pipeline but stop after identification — print a table
/// of identified mods and exit.  Used by the `list` subcommand.
async fn run_list(
    config: &ResolvedConfig,
    api: &dyn ApiClient,
    cache: &ApiCache,
    progress: &dyn ProgressRenderer,
) -> Result<()> {
    // 1. Find JARs
    let jars = scanner::find_jars(&config.mods_dir)?;
    if jars.is_empty() {
        println!("No JAR files found in {}.", config.mods_dir.display());
        return Ok(());
    }

    // 2. Hash all
    progress.start_phase("Hashing", jars.len() as u64);
    let mut hashes: Vec<(PathBuf, String)> = Vec::with_capacity(jars.len());
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

    // 5. Print table: columns = ["Filename", "Name", "Version", "Loader", "Game Versions"]
    let headers: &[&str] = &["Filename", "Name", "Version", "Loader", "Game Versions"];
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
            m.current_version.name.clone(),
            m.current_version.version_number.clone(),
            loaders,
            game_versions,
        ]);
    }

    if rows.is_empty() {
        println!("No mods matched the current filters.");
    } else {
        progress.print_table(headers, &rows);
    }

    // 6. Print summary counts
    println!();
    println!(
        "Total JARs: {}  |  Identified: {}  |  Unknown: {}  |  Filtered out: {}  |  Shown: {}",
        jars.len(),
        before_filter,
        unknown_count,
        filtered_out,
        filtered.len(),
    );

    Ok(())
}
