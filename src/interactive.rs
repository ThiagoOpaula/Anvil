//! Interactive terminal prompts for version and loader selection,
//! plus a pause-before-exit guard for interactive sessions.
//!
//! Prompts only appear when stdout is a TTY and the corresponding config
//! field is `None` (not set via CLI or config file). Esc / Ctrl+C is
//! treated as "skip" — the field stays `None` and the updater falls back
//! to auto-detection per mod.

use dialoguer::{theme::ColorfulTheme, FuzzySelect, Select};

use crate::config::ResolvedConfig;
use crate::types::ApiClient;

// ── Fallback data (offline / API error) ───────────────────────────────────

/// Curated list of recent stable Minecraft versions when the Modrinth API
/// is unreachable.
const FALLBACK_GAME_VERSIONS: &[&str] = &[
    "1.21.5",
    "1.21.4",
    "1.21.3",
    "1.21.1",
    "1.21",
    "1.20.6",
    "1.20.4",
    "1.20.2",
    "1.20.1",
    "1.20",
    "1.19.4",
    "1.19.2",
    "1.19",
    "1.18.2",
];

/// Common mod loaders when the Modrinth API is unreachable.
const FALLBACK_LOADERS: &[&str] = &["fabric", "forge", "quilt", "neoforge"];

// ── Public API ────────────────────────────────────────────────────────────

/// Check whether both stdout and stdin are connected to a terminal.
pub fn is_interactive() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal() && std::io::stdin().is_terminal()
}

/// Prompt for game version and loader if not already set in config.
///
/// Only runs when `is_interactive()` returns `true`. Individual prompts are
/// skipped when the corresponding `ResolvedConfig` field is already `Some(...)`
/// (set via CLI flag or config file).
///
/// # Errors
///
/// Returns `Ok(())` even when prompts fail (e.g. terminal too narrow) —
/// failures are logged as warnings and the tool continues with auto-detect.
pub async fn prompt_if_needed(
    config: &mut ResolvedConfig,
    api: &dyn ApiClient,
) -> crate::error::Result<()> {
    if !is_interactive() {
        tracing::debug!("not a TTY — skipping interactive prompts");
        return Ok(());
    }

    // ── Game version ──────────────────────────────────────────────────
    if config.game_version.is_none() {
        match prompt_game_version(api).await {
            Ok(Some(version)) => {
                tracing::info!(%version, "game version selected interactively");
                config.game_version = Some(version);
            }
            Ok(None) => {
                tracing::info!("game version selection skipped — will auto-detect per mod");
            }
            Err(e) => {
                tracing::warn!(
                    "game version prompt failed: {} — will auto-detect per mod",
                    e
                );
            }
        }
    }

    // ── Loader ────────────────────────────────────────────────────────
    if config.loader.is_none() {
        match prompt_loader(api).await {
            Ok(Some(loader)) => {
                let normalized = loader.to_lowercase();
                tracing::info!(%normalized, "loader selected interactively");
                config.loader = Some(normalized);
            }
            Ok(None) => {
                tracing::info!("loader selection skipped — will auto-detect per mod");
            }
            Err(e) => {
                tracing::warn!("loader prompt failed: {} — will auto-detect per mod", e);
            }
        }
    }

    Ok(())
}

/// Wait for the user to press Enter before exiting.
///
/// Only shows the prompt when `is_interactive()` is `true`. This gives the
/// user time to read the final output before the terminal window closes.
pub fn pause_before_exit() {
    if !is_interactive() {
        return;
    }

    use std::io::{self, BufRead, Write};
    print!("\nPress Enter to exit...");
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().lock().read_line(&mut line);
}

// ── Internal prompt helpers ───────────────────────────────────────────────

async fn prompt_game_version(api: &dyn ApiClient) -> crate::error::Result<Option<String>> {
    let versions = fetch_game_versions(api).await;

    if versions.is_empty() {
        tracing::warn!("no game versions available — skipping prompt");
        return Ok(None);
    }

    let mut items: Vec<String> = versions.clone();
    items.push("(auto-detect from each mod)".to_string());

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select Minecraft version (type to filter, Esc to skip):")
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|e| crate::error::Error::Other(format!("selection prompt failed: {}", e)))?;

    match selection {
        Some(idx) if idx < versions.len() => Ok(Some(versions[idx].clone())),
        _ => Ok(None), // auto-detect sentinel or Esc
    }
}

async fn prompt_loader(api: &dyn ApiClient) -> crate::error::Result<Option<String>> {
    let loaders = fetch_loaders(api).await;

    if loaders.is_empty() {
        tracing::warn!("no loaders available — skipping prompt");
        return Ok(None);
    }

    let mut items: Vec<String> = loaders.clone();
    items.push("(auto-detect from each mod)".to_string());

    // Default to fabric if it's in the list (most common loader).
    let default_idx = loaders.iter().position(|l| l == "fabric").unwrap_or(0);

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select mod loader:")
        .items(&items)
        .default(default_idx)
        .interact_opt()
        .map_err(|e| crate::error::Error::Other(format!("selection prompt failed: {}", e)))?;

    match selection {
        Some(idx) if idx < loaders.len() => Ok(Some(loaders[idx].clone())),
        _ => Ok(None),
    }
}

// ── Data fetching (API with fallback) ─────────────────────────────────────

async fn fetch_game_versions(api: &dyn ApiClient) -> Vec<String> {
    match api.get_game_versions().await {
        Ok(versions) if !versions.is_empty() => versions,
        other => {
            if let Err(ref e) = other {
                tracing::warn!("failed to fetch game versions from Modrinth API: {}", e);
            }
            tracing::info!("using fallback game version list");
            FALLBACK_GAME_VERSIONS
                .iter()
                .map(|s| s.to_string())
                .collect()
        }
    }
}

async fn fetch_loaders(api: &dyn ApiClient) -> Vec<String> {
    match api.get_loaders().await {
        Ok(loaders) if !loaders.is_empty() => loaders,
        other => {
            if let Err(ref e) = other {
                tracing::warn!("failed to fetch loaders from Modrinth API: {}", e);
            }
            tracing::info!("using fallback loader list");
            FALLBACK_LOADERS.iter().map(|s| s.to_string()).collect()
        }
    }
}

#[cfg(test)]
#[path = "tests/test_interactive.rs"]
mod tests;
