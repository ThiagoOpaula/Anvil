//! Launcher mods-folder discovery.
//!
//! Finds the mods folders that Linux (and Windows) users actually have, instead
//! of assuming `~/.minecraft/mods`. Every candidate is probed and only reported
//! when it exists — a wrong guess degrades to silence, never to a bad path.
//!
//! The traversal is pure and spec-driven (`discover_from`), so the whole
//! feature is testable on both platforms; only `root_specs()` is
//! platform-specific.
//!
//! Known gap: an instance whose game directory was relocated via
//! `instance.cfg` is **not** auto-detected. Add such a folder as a profile by
//! hand.

use std::path::{Path, PathBuf};

// ── Types ────────────────────────────────────────────────────────────────

/// Which launcher a candidate folder belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LauncherKind {
    Official,
    FlatpakOfficial,
    Prism,
    MultiMc,
    ModrinthApp,
    AtLauncher,
    GdLauncher,
}

impl LauncherKind {
    /// Human-readable name for tables and profile naming.
    pub fn label(self) -> &'static str {
        match self {
            LauncherKind::Official => "Minecraft Launcher",
            LauncherKind::FlatpakOfficial => "Minecraft Launcher (Flatpak)",
            LauncherKind::Prism => "Prism Launcher",
            LauncherKind::MultiMc => "MultiMC",
            LauncherKind::ModrinthApp => "Modrinth App",
            LauncherKind::AtLauncher => "ATLauncher",
            LauncherKind::GdLauncher => "GDLauncher",
        }
    }

    /// Short lower-case name used when proposing a profile name.
    pub fn slug(self) -> &'static str {
        match self {
            LauncherKind::Official => "vanilla",
            LauncherKind::FlatpakOfficial => "vanilla-flatpak",
            LauncherKind::Prism => "prism",
            LauncherKind::MultiMc => "multimc",
            LauncherKind::ModrinthApp => "modrinth",
            LauncherKind::AtLauncher => "atlauncher",
            LauncherKind::GdLauncher => "gdlauncher",
        }
    }
}

/// How a root directory maps onto a mods folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RootKind {
    /// The root **is** the mods folder.
    ModsDir,
    /// The mods folder is `root/mods`.
    GameDir,
    /// Expand one level; each instance has its own game dir (Prism/MultiMC/ATLauncher).
    InstancesDir,
    /// Expand one level; each profile has its own mods folder (Modrinth App).
    ProfilesDir,
}

/// A launcher root to probe, before any filesystem IO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootSpec {
    pub launcher: LauncherKind,
    pub root: PathBuf,
    pub kind: RootKind,
}

/// A mods folder that was found to exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredModsDir {
    /// The mods folder, canonicalized when resolvable.
    pub mods_dir: PathBuf,
    pub launcher: LauncherKind,
    /// The raw launcher root, kept for display.
    pub root: PathBuf,
    /// Number of `*.jar` files directly inside.
    pub jar_count: usize,
}

// ── Roots ────────────────────────────────────────────────────────────────

/// Every launcher root worth probing on this platform.
///
/// Reads no directories: `std::env` / `dirs` lookups only. Wider is better
/// here — a root that does not exist contributes nothing.
pub fn root_specs() -> Vec<RootSpec> {
    let mut specs = Vec::new();

    if let Some(home) = dirs::home_dir() {
        specs.push(RootSpec {
            launcher: LauncherKind::Official,
            root: home.join(".minecraft"),
            kind: RootKind::GameDir,
        });

        // The official launcher can point a profile at an arbitrary game dir.
        for game_dir in official_custom_game_dirs(&official_profiles_json_paths()) {
            specs.push(RootSpec {
                launcher: LauncherKind::Official,
                root: game_dir,
                kind: RootKind::GameDir,
            });
        }
    }

    if let Some(data) = dirs::data_dir() {
        specs.push(RootSpec {
            launcher: LauncherKind::Prism,
            root: data.join("PrismLauncher").join("instances"),
            kind: RootKind::InstancesDir,
        });
        // Both casings exist in the wild; probing both is cheap.
        specs.push(RootSpec {
            launcher: LauncherKind::MultiMc,
            root: data.join("MultiMC").join("instances"),
            kind: RootKind::InstancesDir,
        });
        specs.push(RootSpec {
            launcher: LauncherKind::MultiMc,
            root: data.join("multimc").join("instances"),
            kind: RootKind::InstancesDir,
        });
        specs.push(RootSpec {
            launcher: LauncherKind::ModrinthApp,
            root: data.join("ModrinthApp").join("profiles"),
            kind: RootKind::ProfilesDir,
        });
        specs.push(RootSpec {
            launcher: LauncherKind::AtLauncher,
            root: data.join("ATLauncher").join("instances"),
            kind: RootKind::InstancesDir,
        });
    }

    if let Some(config) = dirs::config_dir() {
        specs.push(RootSpec {
            launcher: LauncherKind::GdLauncher,
            root: config.join("gdlauncher_next").join("instances"),
            kind: RootKind::InstancesDir,
        });
    }

    // Flatpak rewrites XDG dirs into ~/.var/app, so those launchers' data is
    // invisible to dirs::data_dir().
    #[cfg(target_os = "linux")]
    if let Some(base) = flatpak_user_dir() {
        specs.push(RootSpec {
            launcher: LauncherKind::Prism,
            root: base.join("org.prismlauncher.PrismLauncher/data/PrismLauncher/instances"),
            kind: RootKind::InstancesDir,
        });
        specs.push(RootSpec {
            launcher: LauncherKind::ModrinthApp,
            root: base.join("com.modrinth.ModrinthApp/data/ModrinthApp/profiles"),
            kind: RootKind::ProfilesDir,
        });
        specs.push(RootSpec {
            launcher: LauncherKind::FlatpakOfficial,
            root: base.join("com.mojang.Minecraft/.minecraft"),
            kind: RootKind::GameDir,
        });
    }

    specs
}

#[cfg(target_os = "linux")]
fn flatpak_user_dir() -> Option<PathBuf> {
    std::env::var_os("FLATPAK_USER_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".var/app")))
}

fn official_profiles_json_paths() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let root = home.join(".minecraft");
    vec![
        root.join("launcher_profiles.json"),
        root.join("launcher_profiles_microsoft_store.json"),
    ]
}

/// Game directories from `launcher_profiles*.json`.
///
/// Accepts both shapes the official launcher has shipped:
/// `"gameDir": "/path"` and `"gameDir": {"path": "/path"}`. Unreadable,
/// malformed, or absent entries are skipped — this never returns an error.
pub fn official_custom_game_dirs(json_paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    for path in json_paths {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        let Some(profiles) = value.get("profiles").and_then(|p| p.as_object()) else {
            continue;
        };

        for profile in profiles.values() {
            let Some(game_dir) = profile.get("gameDir") else {
                continue;
            };
            match game_dir {
                serde_json::Value::String(s) => dirs.push(PathBuf::from(s)),
                serde_json::Value::Object(obj) => {
                    if let Some(s) = obj.get("path").and_then(|p| p.as_str()) {
                        dirs.push(PathBuf::from(s));
                    }
                }
                _ => {}
            }
        }
    }

    dirs
}

// ── Probing ──────────────────────────────────────────────────────────────

/// Probe the real filesystem using [`root_specs`].
pub fn discover() -> Vec<DiscoveredModsDir> {
    discover_from(&root_specs())
}

/// Probe `specs`, returning only mods folders that exist.
///
/// Pure with respect to `specs` — the only IO is reading the directories they
/// name. Results are deduplicated by canonical path (first spec wins) and
/// sorted by `(launcher, mods_dir)` so output is deterministic.
pub fn discover_from(specs: &[RootSpec]) -> Vec<DiscoveredModsDir> {
    let mut found: Vec<DiscoveredModsDir> = Vec::new();

    for spec in specs {
        for candidate in candidate_mods_dirs(spec) {
            let canonical = std::fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
            if found.iter().any(|d| d.mods_dir == canonical) {
                continue;
            }
            found.push(DiscoveredModsDir {
                jar_count: count_jars(&canonical),
                mods_dir: canonical,
                launcher: spec.launcher,
                root: spec.root.clone(),
            });
        }
    }

    found.sort_by(|a, b| (&a.launcher, &a.mods_dir).cmp(&(&b.launcher, &b.mods_dir)));
    found
}

/// The mods folders a spec could name, filtered to those that exist.
fn candidate_mods_dirs(spec: &RootSpec) -> Vec<PathBuf> {
    match spec.kind {
        RootKind::ModsDir => {
            if spec.root.is_dir() {
                vec![spec.root.clone()]
            } else {
                Vec::new()
            }
        }
        RootKind::GameDir => {
            let mods = spec.root.join("mods");
            if mods.is_dir() {
                vec![mods]
            } else {
                Vec::new()
            }
        }
        RootKind::InstancesDir => {
            expand_one_level(&spec.root, &["minecraft/mods", ".minecraft/mods", "mods"])
        }
        RootKind::ProfilesDir => expand_one_level(&spec.root, &["mods", ".minecraft/mods"]),
    }
}

/// One child directory per instance/profile; the first existing candidate wins,
/// so an instance is never reported twice.
///
/// Any IO error (missing root, permission denied, deleted instance) makes the
/// root contribute nothing rather than fail.
fn expand_one_level(root: &Path, candidates: &[&str]) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut children: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| !is_hidden(path))
        .filter(|path| path.is_dir())
        .collect();
    children.sort();

    children
        .into_iter()
        .filter_map(|child| {
            candidates
                .iter()
                .map(|rel| child.join(rel))
                .find(|candidate| candidate.is_dir())
        })
        .collect()
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().starts_with('.'))
        .unwrap_or(true)
}

fn count_jars(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("jar"))
                .unwrap_or(false)
        })
        .count()
}
