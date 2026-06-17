//! Anvil — Minecraft Mod Updater (binary entry point).
//!
//! See `lib.rs` for the full crate documentation and public API.

use clap::Parser;
use tracing_subscriber::EnvFilter;

use anvil::api::ModrinthApi;
use anvil::cache::ApiCache;
use anvil::cli::{Cli, Command};
use anvil::config;
use anvil::error::Error;
use anvil::output;
use anvil::paths;
use anvil::updater;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── 1. Parse CLI (needed early for log-level hints) ──────────────────
    let cli = Cli::parse();

    // ── 2. Set up tracing subscriber ─────────────────────────────────────
    let default_level = match &cli.command {
        Some(Command::Update(args)) if args.common.verbose => "anvil=debug",
        Some(Command::Update(args)) if args.common.quiet => "anvil=error",
        Some(Command::List(args)) if args.common.verbose => "anvil=debug",
        Some(Command::List(args)) if args.common.quiet => "anvil=error",
        _ => "anvil=info",
    };

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_level(true)
        .with_ansi(false)
        .init();

    // ── 3. Load config file ──────────────────────────────────────────────
    let config_path = paths::config_dir().join("config.toml");
    let mut resolved = config::resolve(&cli, Some(&config_path))?;

    // ── 4. Build API client, cache, and progress renderer ────────────────
    let api = ModrinthApi::new()?;
    let cache = ApiCache::new();
    let progress = output::ConsoleProgress::new();

    // ── 4b. Interactive version/loader selection ─────────────────────────
    // Only prompt for update (default) and list subcommands, not rollback.
    let needs_prompts = !matches!(cli.command, Some(Command::Rollback));
    if needs_prompts
        && let Err(e) = anvil::interactive::prompt_if_needed(&mut resolved, &api).await
    {
        tracing::warn!("interactive prompt error: {} — continuing", e);
    }

    // ── 5. Dispatch on subcommand ────────────────────────────────────────
    match cli.command {
        Some(Command::Rollback) => {
            match anvil::backup::rollback(&resolved.mods_dir) {
                Ok(count) => {
                    println!("Restored {} mod(s) from backup.", count);
                }
                Err(Error::NoBackups) => {
                    eprintln!("No backups found in {}.", resolved.mods_dir.display());
                    anvil::interactive::pause_before_exit();
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Rollback failed: {}", e);
                    anvil::interactive::pause_before_exit();
                    std::process::exit(1);
                }
            }
        }

        Some(Command::List(_)) => {
            anvil::run_list(&resolved, &api, &cache, &progress).await?;
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
                    anvil::interactive::pause_before_exit();
                    std::process::exit(1);
                }
            }
        }
    }

    anvil::interactive::pause_before_exit();
    Ok(())
}
