//! CLI argument parsing via clap derive.
//!
//! Defines the top-level `Cli` struct, subcommands, and their arguments.
//! Help text lives in doc comments (clap renders them at runtime).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// ⚒ Anvil — Minecraft Mod Updater
///
/// Scans a mods folder, identifies each JAR via SHA1 hash against the
/// Modrinth API, checks for newer versions matching the same loader and
/// game version, and downloads updates (backing up old files by default).
#[derive(Parser)]
#[command(name = "anvil", version, about)]
pub struct Cli {
    /// Path to the Minecraft mods folder.
    ///
    /// Defaults to the standard Minecraft mods directory for your platform
    /// (e.g. %APPDATA%/.minecraft/mods on Windows).
    #[arg(long, global = true, value_name = "PATH")]
    pub mods_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Available subcommands.
#[derive(Subcommand)]
pub enum Command {
    /// Check for and download mod updates (default if no subcommand given).
    Update(UpdateArgs),
    /// Scan and list identified mods in a table.
    List(ListArgs),
    /// Restore mods from the latest backup.
    Rollback,
}

// ── Shared flags (update + list) ──────────────────────────────────────────

/// Arguments shared between the `update` and `list` subcommands.
#[derive(Args)]
pub struct CommonArgs {
    /// Target a specific Minecraft version (e.g. "1.21.1").
    /// If not set, the loader and game version are detected from each mod.
    #[arg(long, value_name = "VERSION")]
    pub game_version: Option<String>,

    /// Force a specific mod loader (fabric, forge, quilt, neoforge).
    /// If not set, the loader is detected from each mod individually.
    #[arg(long, value_name = "LOADER")]
    pub loader: Option<String>,

    /// Only process mods whose slug or name matches this pattern.
    /// Can be passed multiple times (mods matching ANY pattern are included).
    #[arg(long = "include", value_name = "PATTERN")]
    pub include: Vec<String>,

    /// Skip mods whose slug or name matches this pattern.
    /// Can be passed multiple times (mods matching ANY pattern are excluded).
    #[arg(long = "exclude", value_name = "PATTERN")]
    pub exclude: Vec<String>,

    /// Print more detailed output (conflicts with --quiet).
    #[arg(short, long, conflicts_with = "quiet")]
    pub verbose: bool,

    /// Suppress all output except errors (conflicts with --verbose).
    #[arg(short, long, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Check for updates but do not download anything.
    #[arg(long)]
    pub dry_run: bool,
}

// ── Update-only flags ────────────────────────────────────────────────────

/// Arguments specific to the `update` subcommand.
#[derive(Args)]
pub struct UpdateArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    /// Skip the backup step (by default, old JARs are backed up before
    /// being replaced).
    #[arg(long)]
    pub no_backup: bool,

    /// Maximum number of mods to update in a single run.
    #[arg(long, value_name = "N")]
    pub max_updates: Option<usize>,

    /// Skip the confirmation prompt before downloading.
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Show the changelog (when available) for each updated mod.
    #[arg(long)]
    pub changelog: bool,
}

// ── List flags ───────────────────────────────────────────────────────────

/// Arguments specific to the `list` subcommand.
#[derive(Args)]
pub struct ListArgs {
    #[command(flatten)]
    pub common: CommonArgs,
}
