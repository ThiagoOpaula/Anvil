//! Include/exclude, loader, and game-version filtering for identified mods.

use regex::Regex;
use tracing;

use crate::types::{FilterOpts, IdentifiedMod};

/// Apply include/exclude and loader/game-version filters.
///
/// Filters are applied in this order, each narrowing the set:
///
/// 1. **Include** — keep only mods matching at least one include pattern
///    (no-op when `opts.include` is empty).
/// 2. **Exclude** — drop mods matching any exclude pattern.
/// 3. **Loader** — drop mods whose `current_version.loaders` doesn't contain
///    the requested loader (no-op when `opts.loader` is `None`).
/// 4. **Game version** — drop mods whose `current_version.game_versions`
///    doesn't contain the requested game version (no-op when
///    `opts.game_version` is `None`).
///
/// Returns a new `Vec` containing only the mods that passed every filter.
/// Filtered-out mods are logged at `tracing::debug!` level.
pub fn apply(mods: &[IdentifiedMod], opts: &FilterOpts) -> Vec<IdentifiedMod> {
    let mut result: Vec<IdentifiedMod> = Vec::new();

    for m in mods {
        // ── 1. Include filter ──────────────────────────────────────────
        if !opts.include.is_empty() && !any_pattern_matches(m, &opts.include) {
            tracing::debug!(
                filename = %m.filename,
                name = %m.current_version.name,
                version = %m.current_version.version_number,
                "filtering out — no include pattern matched"
            );
            continue;
        }

        // ── 2. Exclude filter ──────────────────────────────────────────
        if !opts.exclude.is_empty() && any_pattern_matches(m, &opts.exclude) {
            tracing::debug!(
                filename = %m.filename,
                name = %m.current_version.name,
                version = %m.current_version.version_number,
                "filtering out — matched an exclude pattern"
            );
            continue;
        }

        // ── 3. Loader filter ───────────────────────────────────────────
        if let Some(ref loader) = opts.loader {
            let has_loader = m
                .current_version
                .loaders
                .iter()
                .any(|l| l.eq_ignore_ascii_case(loader));
            if !has_loader {
                tracing::debug!(
                    filename = %m.filename,
                    required_loader = %loader,
                    available_loaders = ?m.current_version.loaders,
                    "filtering out — loader mismatch"
                );
                continue;
            }
        }

        // ── 4. Game version filter ─────────────────────────────────────
        if let Some(ref game_version) = opts.game_version {
            let has_version = m
                .current_version
                .game_versions
                .iter()
                .any(|v| v == game_version);
            if !has_version {
                tracing::debug!(
                    filename = %m.filename,
                    required_game_version = %game_version,
                    available_game_versions = ?m.current_version.game_versions,
                    "filtering out — game version mismatch"
                );
                continue;
            }
        }

        result.push(m.clone());
    }

    result
}

// ── Pattern Matching Helpers ──────────────────────────────────────────────

/// Returns `true` when `m` matches at least one pattern from `patterns`.
///
/// Matching is performed against two fields:
/// - `current_version.name` (display name)
/// - `current_version.version_number`
fn any_pattern_matches(m: &IdentifiedMod, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|p| matches_pattern(&m.current_version.name, p)
              || matches_pattern(&m.current_version.version_number, p))
}

/// Pattern matching logic used by include/exclude filtering.
///
/// Rules (checked in order):
/// 1. If the pattern starts and ends with `/`, treat the inner text as a
///    regular expression (e.g. `/sodium.*/`).
/// 2. Otherwise, translate `*` into regex `.*`, escape the rest, and perform
///    a **case-insensitive** substring match.
fn matches_pattern(haystack: &str, pattern: &str) -> bool {
    // Regex mode: /pattern/
    if pattern.starts_with('/') && pattern.ends_with('/') && pattern.len() >= 2 {
        let regex_str = &pattern[1..pattern.len() - 1];
        if let Ok(re) = Regex::new(regex_str) {
            return re.is_match(haystack);
        }
        // Invalid regex — fall through to substring fallback.
    }

    // Default mode: wildcard-aware, case-insensitive substring match.
    let escaped = regex::escape(pattern);
    let with_wildcards = escaped.replace("\\*", ".*");
    let full_pattern = format!("(?i){}", with_wildcards);
    if let Ok(re) = Regex::new(&full_pattern) {
        return re.is_match(haystack);
    }

    // Absolute last resort — simple case-insensitive contains.
    haystack.to_lowercase().contains(&pattern.to_lowercase())
}

