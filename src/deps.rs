//! Dependency graph over identified mods.
//!
//! Builds edges from each mod's Modrinth `dependencies` list, reports
//! missing required deps and installed incompatibilities, and renders a
//! per-mod tree for the CLI.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::cache::ApiCache;
use crate::error::{Error, Result};
use crate::types::{ApiClient, DependencyType, IdentifiedMod, ModVersion, ProgressRenderer};

// ── Graph types ─────────────────────────────────────────────────────────

/// One dependency relationship declared by an installed mod.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepEdge {
    /// Project id of the dependent mod.
    pub from_project_id: String,
    /// Display name of the dependent mod.
    pub from_name: String,
    /// Project id of the dependency target.
    pub to_project_id: String,
    /// Display name of the target when known (installed or resolved from
    /// Modrinth for a missing project); `None` when unresolved.
    pub to_name: Option<String>,
    /// Whether the target project is present among the identified mods.
    pub installed: bool,
    /// How the dependent declares the relationship.
    pub kind: DependencyType,
}

/// A required dependency that is not present among the identified mods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDep {
    pub dependent_project_id: String,
    pub dependent_name: String,
    pub to_project_id: String,
    /// Display name of the target when it can be resolved from Modrinth;
    /// `None` when the project lookup fails (deleted / private / offline).
    pub to_name: Option<String>,
    pub kind: DependencyType,
}

/// Two installed mods that declare each other (or one-way) incompatible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompatPair {
    pub project_id: String,
    pub project_name: String,
    pub conflicts_with_project_id: String,
    pub conflicts_with_name: String,
}

/// A dependency edge that cannot be resolved to a project id
/// (Modrinth allows version-id-only links).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedDep {
    pub from_project_id: String,
    pub from_name: String,
    pub version_id: String,
    pub kind: DependencyType,
}

/// Full dependency picture over a set of identified mods.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DepGraph {
    pub edges: Vec<DepEdge>,
    pub missing: Vec<MissingDep>,
    pub incompat: Vec<IncompatPair>,
    pub unresolved: Vec<UnresolvedDep>,
}

impl DepGraph {
    /// True when there are no missing-required or incompatibility issues.
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.incompat.is_empty()
    }

    /// Edges declared by `project_id`, sorted for stable display.
    pub fn edges_for(&self, project_id: &str) -> Vec<&DepEdge> {
        let mut out: Vec<&DepEdge> = self
            .edges
            .iter()
            .filter(|e| e.from_project_id == project_id)
            .collect();
        out.sort_by(|a, b| {
            kind_rank(a.kind)
                .cmp(&kind_rank(b.kind))
                .then_with(|| a.to_project_id.cmp(&b.to_project_id))
        });
        out
    }
}

// ── Build ───────────────────────────────────────────────────────────────

/// Build a dependency graph from the identified mods.
///
/// Resolution is by `project_id`. Edges that only carry a `version_id`
/// are recorded as `unresolved` rather than guessed. Optional and embedded
/// deps never produce `missing` entries; incompatible produces an issue
/// only when the conflicting project is also installed.
pub fn build(mods: &[IdentifiedMod]) -> DepGraph {
    let installed: HashMap<&str, &IdentifiedMod> = mods
        .iter()
        .map(|m| (m.current_version.project_id.as_str(), m))
        .collect();

    let mut seen = HashSet::new();
    let mut graph = DepGraph::default();

    for m in mods {
        let from_id = m.current_version.project_id.as_str();
        let from_name = m.current_version.name.as_str();

        for dep in &m.current_version.dependencies {
            let Some(to_id) = dep.project_id.as_deref() else {
                if let Some(ref version_id) = dep.version_id {
                    let key = (from_id, "", version_id.as_str(), kind_key(dep.dependency_type));
                    if seen.insert(key) {
                        graph.unresolved.push(UnresolvedDep {
                            from_project_id: from_id.to_string(),
                            from_name: from_name.to_string(),
                            version_id: version_id.clone(),
                            kind: dep.dependency_type,
                        });
                    }
                }
                continue;
            };

            // Skip self-references — they are never actionable.
            if to_id == from_id {
                continue;
            }

            let edge_key = (from_id, to_id, "", kind_key(dep.dependency_type));
            if !seen.insert(edge_key) {
                continue;
            }

            let target = installed.get(to_id);
            let to_name = target.map(|t| t.current_version.name.clone());

            graph.edges.push(DepEdge {
                from_project_id: from_id.to_string(),
                from_name: from_name.to_string(),
                to_project_id: to_id.to_string(),
                to_name: to_name.clone(),
                installed: target.is_some(),
                kind: dep.dependency_type,
            });

            match dep.dependency_type {
                DependencyType::Required if target.is_none() => {
                    graph.missing.push(MissingDep {
                        dependent_project_id: from_id.to_string(),
                        dependent_name: from_name.to_string(),
                        to_project_id: to_id.to_string(),
                        to_name: None,
                        kind: dep.dependency_type,
                    });
                }
                DependencyType::Incompatible if let Some(t) = target => {
                    graph.incompat.push(IncompatPair {
                        project_id: from_id.to_string(),
                        project_name: from_name.to_string(),
                        conflicts_with_project_id: to_id.to_string(),
                        conflicts_with_name: t.current_version.name.clone(),
                    });
                }
                _ => {}
            }
        }
    }

    graph.missing.sort_by(|a, b| {
        a.dependent_name
            .cmp(&b.dependent_name)
            .then_with(|| a.to_project_id.cmp(&b.to_project_id))
    });
    graph.incompat.sort_by(|a, b| {
        a.project_name
            .cmp(&b.project_name)
            .then_with(|| a.conflicts_with_name.cmp(&b.conflicts_with_name))
    });

    graph
}

// ── Projection & diff ───────────────────────────────────────────────────

/// Apply `(project_id, new_version)` replacements onto a clone of `mods`.
/// Unmatched entries unchanged; updates for uninstalled projects ignored.
pub fn project_updates(
    mods: &[IdentifiedMod],
    updates: &[(String, ModVersion)],
) -> Vec<IdentifiedMod> {
    mods.iter()
        .map(|m| {
            let replacement = updates
                .iter()
                .find(|(project_id, _)| *project_id == m.current_version.project_id);
            match replacement {
                Some((_, new_version)) => IdentifiedMod {
                    current_version: new_version.clone(),
                    ..m.clone()
                },
                None => m.clone(),
            }
        })
        .collect()
}

/// Pairs in `after.incompat` whose `(project_id, conflicts_with_project_id)`
/// key is absent from `before.incompat`. Identity is the id pair, not struct
/// equality — display names can change across versions.
pub fn newly_incompatible(before: &DepGraph, after: &DepGraph) -> Vec<IncompatPair> {
    let existing: HashSet<(&str, &str)> = before
        .incompat
        .iter()
        .map(|p| (p.project_id.as_str(), p.conflicts_with_project_id.as_str()))
        .collect();

    after
        .incompat
        .iter()
        .filter(|p| {
            !existing.contains(&(p.project_id.as_str(), p.conflicts_with_project_id.as_str()))
        })
        .cloned()
        .collect()
}

// ── Rendering ───────────────────────────────────────────────────────────

/// Render a per-mod dependency tree (one mod per root) as display lines.
///
/// Mods are sorted by name. Each outgoing edge appears as a child line with
/// kind and install status. Mods without declared deps still appear.
pub fn format_tree(mods: &[IdentifiedMod], graph: &DepGraph) -> Vec<String> {
    let mut by_name: BTreeMap<&str, &IdentifiedMod> = BTreeMap::new();
    for m in mods {
        by_name
            .entry(m.current_version.name.as_str())
            .or_insert(m);
    }

    let mut lines = Vec::new();
    if by_name.is_empty() {
        lines.push(String::from("(no identified mods)"));
        return lines;
    }

    let mut first = true;
    for (name, m) in by_name {
        if !first {
            lines.push(String::new());
        }
        first = false;

        lines.push(format!(
            "{} {}",
            name, m.current_version.version_number
        ));

        let edges = graph.edges_for(&m.current_version.project_id);
        let unresolved: Vec<_> = graph
            .unresolved
            .iter()
            .filter(|u| u.from_project_id == m.current_version.project_id)
            .collect();

        if edges.is_empty() && unresolved.is_empty() {
            lines.push(String::from("  (no declared dependencies)"));
            continue;
        }

        let total = edges.len() + unresolved.len();
        for (i, edge) in edges.iter().enumerate() {
            let branch = if i + 1 == total { "└─" } else { "├─" };
            let label = edge.to_name.as_deref().unwrap_or(&edge.to_project_id);
            let status = if edge.installed {
                "✓"
            } else if edge.kind == DependencyType::Required {
                "✗ MISSING"
            } else {
                "— not installed"
            };
            lines.push(format!(
                "  {} {} ({}) {}",
                branch,
                label,
                kind_label(edge.kind),
                status
            ));
        }

        for (i, u) in unresolved.iter().enumerate() {
            let branch = if i + 1 == total { "└─" } else { "├─" };
            lines.push(format!(
                "  {} version {} ({}) ? unresolved",
                branch,
                u.version_id,
                kind_label(u.kind)
            ));
        }
    }

    lines
}

/// Render missing-required and incompatibility issue lines.
/// Returns `None` when the graph is clean.
pub fn format_issues(graph: &DepGraph) -> Option<Vec<String>> {
    if graph.is_clean() {
        return None;
    }

    let mut lines = Vec::new();

    if !graph.missing.is_empty() {
        lines.push(String::from("Missing required dependencies:"));
        for miss in &graph.missing {
            let label = miss.to_name.as_deref().unwrap_or(&miss.to_project_id);
            lines.push(format!("  ! {} requires {}", miss.dependent_name, label));
        }
    }

    if !graph.incompat.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(String::from("Incompatibilities (both installed):"));
        for pair in &graph.incompat {
            lines.push(format!(
                "  ! {} conflicts with {}",
                pair.project_name, pair.conflicts_with_name
            ));
        }
    }

    Some(lines)
}

// ── Display-name resolution ─────────────────────────────────────────────

/// Fill in display names for edges whose target is not installed.
///
/// Looks up each unique unresolved project id via the project cache or
/// `GET /project/{id}`. Failures leave `to_name` as `None` so callers can
/// fall back to the raw project id.
pub async fn resolve_display_names(
    graph: &mut DepGraph,
    api: &dyn ApiClient,
    cache: &ApiCache,
) {
    use futures::stream::{self, StreamExt};

    let unique_ids: Vec<String> = {
        let mut seen = HashSet::new();
        let mut ids = Vec::new();
        for edge in &graph.edges {
            if edge.to_name.is_none()
                && seen.insert(edge.to_project_id.as_str())
            {
                ids.push(edge.to_project_id.clone());
            }
        }
        for miss in &graph.missing {
            if miss.to_name.is_none()
                && seen.insert(miss.to_project_id.as_str())
            {
                ids.push(miss.to_project_id.clone());
            }
        }
        ids
    };

    if unique_ids.is_empty() {
        return;
    }

    let lookups: Vec<(String, Option<String>)> = stream::iter(unique_ids)
        .map(|id| async move {
            let name = match cache.get_project(&id) {
                Ok(Some(p)) => Some(p.title),
                _ => match api.get_project(&id).await {
                    Ok(p) => {
                        let _ = cache.set_project(&id, &p);
                        Some(p.title)
                    }
                    Err(e) => {
                        tracing::debug!(
                            project_id = %id,
                            error = %e,
                            "could not resolve dependency display name"
                        );
                        None
                    }
                },
            };
            (id, name)
        })
        .buffer_unordered(4)
        .collect()
        .await;

    for (id, name) in lookups {
        let Some(name) = name else { continue };
        for edge in graph
            .edges
            .iter_mut()
            .filter(|e| e.to_project_id == id && e.to_name.is_none())
        {
            edge.to_name = Some(name.clone());
        }
        for miss in graph
            .missing
            .iter_mut()
            .filter(|m| m.to_project_id == id && m.to_name.is_none())
        {
            miss.to_name = Some(name.clone());
        }
    }
}

// ── Install missing dependencies ────────────────────────────────────────

/// Result of one successfully downloaded dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledDep {
    pub project_id: String,
    pub name: String,
    pub version: String,
    pub filename: String,
}

/// Result of [`install_missing`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallOutcome {
    pub installed: Vec<InstalledDep>,
    /// `(project_id, reason)` pairs that could not be installed.
    pub failed: Vec<(String, String)>,
}

/// Options for [`install_projects`].
#[derive(Debug, Clone, Default)]
pub struct InstallOptions<'a> {
    /// Preferred mod loader filter (e.g. `"fabric"`).
    pub loader: Option<&'a str>,
    /// Preferred Minecraft version filter (e.g. `"1.21.1"`).
    pub game_version: Option<&'a str>,
    /// Project ids already present among identified mods — never downloaded.
    pub already_installed: HashSet<String>,
}

impl InstallOutcome {
    pub fn is_empty(&self) -> bool {
        self.installed.is_empty() && self.failed.is_empty()
    }
}

/// Download every missing **required** dependency (and their transitive
/// required deps) into `mods_dir`.
///
/// For each project, the newest version matching `loader` / `game_version`
/// is preferred; if none match, the newest overall version is used. Files
/// whose target filename already exists in `mods_dir` are skipped.
pub async fn install_missing(
    mods_dir: &Path,
    graph: &DepGraph,
    api: &dyn ApiClient,
    cache: &ApiCache,
    loader: Option<&str>,
    game_version: Option<&str>,
    progress: &dyn ProgressRenderer,
) -> Result<InstallOutcome> {
    let project_ids: Vec<String> = {
        let mut seen = HashSet::new();
        graph
            .missing
            .iter()
            .filter(|m| m.kind == DependencyType::Required)
            .filter_map(|m| {
                if seen.insert(m.to_project_id.as_str()) {
                    Some(m.to_project_id.clone())
                } else {
                    None
                }
            })
            .collect()
    };

    // Ids already installed in the folder (so we never re-download them).
    let installed_ids: HashSet<String> = graph
        .edges
        .iter()
        .filter(|e| e.installed)
        .map(|e| e.to_project_id.clone())
        .collect();

    install_projects(
        mods_dir,
        project_ids,
        InstallOptions {
            loader,
            game_version,
            already_installed: installed_ids,
        },
        api,
        cache,
        progress,
    )
    .await
}

/// Download the given project ids (plus transitive required deps) into
/// `mods_dir`.
pub async fn install_projects(
    mods_dir: &Path,
    project_ids: Vec<String>,
    options: InstallOptions<'_>,
    api: &dyn ApiClient,
    cache: &ApiCache,
    progress: &dyn ProgressRenderer,
) -> Result<InstallOutcome> {
    let InstallOptions {
        loader,
        game_version,
        already_installed,
    } = options;
    let mut queue = project_ids;
    let mut installed_ids = already_installed;
    let mut outcome = InstallOutcome::default();
    let mut visited: HashSet<String> = HashSet::new();

    if queue.is_empty() {
        return Ok(outcome);
    }

    progress.start_phase("Installing dependencies", queue.len() as u64);

    while let Some(project_id) = queue.pop() {
        if !visited.insert(project_id.clone()) {
            progress.increment(1);
            continue;
        }

        match install_one(
            mods_dir,
            &project_id,
            api,
            cache,
            loader,
            game_version,
        )
        .await
        {
            Ok((deps, Some(meta))) => {
                for dep_id in deps {
                    if !visited.contains(&dep_id) && !installed_ids.contains(&dep_id) {
                        queue.push(dep_id);
                    }
                }
                tracing::info!(
                    project_id = %meta.project_id,
                    name = %meta.name,
                    version = %meta.version,
                    "installed dependency"
                );
                installed_ids.insert(project_id);
                outcome.installed.push(meta);
            }
            // Already on disk — nothing to download, but still walk its deps.
            Ok((deps, None)) => {
                for dep_id in deps {
                    if !visited.contains(&dep_id) && !installed_ids.contains(&dep_id) {
                        queue.push(dep_id);
                    }
                }
                installed_ids.insert(project_id);
            }
            Err(e) => {
                tracing::warn!(
                    project_id = %project_id,
                    error = %e,
                    "failed to install dependency"
                );
                outcome.failed.push((project_id, e.to_string()));
            }
        }
        progress.increment(1);
    }

    progress.finish_phase();
    Ok(outcome)
}

/// Download one project's matching version into `mods_dir`.
///
/// Always returns the transitive required project ids. The second element
/// is `None` when the target JAR is already on disk (nothing downloaded).
async fn install_one(
    mods_dir: &Path,
    project_id: &str,
    api: &dyn ApiClient,
    cache: &ApiCache,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<(Vec<String>, Option<InstalledDep>)> {
    let loaders: Vec<String> = loader.map(|l| vec![l.to_string()]).unwrap_or_default();
    let game_versions: Vec<String> = game_version
        .map(|g| vec![g.to_string()])
        .unwrap_or_default();

    // Prefer a filtered version; fall back to unfiltered if nothing matches.
    let mut versions = api
        .get_project_versions(project_id, &loaders, &game_versions)
        .await?;
    if versions.is_empty() && (!loaders.is_empty() || !game_versions.is_empty()) {
        versions = api.get_project_versions(project_id, &[], &[]).await?;
    }

    let version = versions.into_iter().next().ok_or_else(|| {
        Error::Other(format!(
            "no versions available for project {}",
            project_id
        ))
    })?;

    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| {
            Error::Other(format!(
                "version {} of {} has no downloadable files",
                version.version_number, project_id
            ))
        })?;

    let dest = mods_dir.join(&file.filename);
    if dest.exists() {
        return Ok((transitive_required(&version), None));
    }

    let progress_cb = |downloaded: u64, total: Option<u64>| {
        if let Some(t) = total {
            tracing::debug!("dep download: {} / {} bytes", downloaded, t);
        } else {
            tracing::debug!("dep download: {} bytes", downloaded);
        }
    };

    let actual_sha1 = api.download_file(&file.url, &dest, &progress_cb).await?;
    if actual_sha1 != file.hashes.sha1 {
        let _ = std::fs::remove_file(&dest);
        return Err(Error::Other(format!(
            "SHA1 mismatch for {}: expected {}, got {}",
            file.filename, file.hashes.sha1, actual_sha1
        )));
    }

    let name = display_name_for(project_id, &version, api, cache).await;
    Ok((
        transitive_required(&version),
        Some(InstalledDep {
            project_id: project_id.to_string(),
            name,
            version: version.version_number.clone(),
            filename: file.filename.clone(),
        }),
    ))
}

/// Required dependency project ids declared by `version`.
fn transitive_required(version: &ModVersion) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for dep in &version.dependencies {
        if dep.dependency_type != DependencyType::Required {
            continue;
        }
        if let Some(id) = dep.project_id.as_deref()
            && seen.insert(id)
        {
            out.push(id.to_string());
        }
    }
    out
}

/// Best-effort display name: version title, then project cache/API.
async fn display_name_for(
    project_id: &str,
    version: &ModVersion,
    api: &dyn ApiClient,
    cache: &ApiCache,
) -> String {
    // Prefer the human title from project metadata when available.
    if let Ok(Some(p)) = cache.get_project(project_id) {
        return p.title;
    }
    match api.get_project(project_id).await {
        Ok(p) => {
            let _ = cache.set_project(project_id, &p);
            p.title
        }
        Err(_) => version.name.clone(),
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn kind_label(kind: DependencyType) -> &'static str {
    match kind {
        DependencyType::Required => "required",
        DependencyType::Optional => "optional",
        DependencyType::Incompatible => "incompatible",
        DependencyType::Embedded => "embedded",
    }
}

fn kind_rank(kind: DependencyType) -> u8 {
    match kind {
        DependencyType::Required => 0,
        DependencyType::Optional => 1,
        DependencyType::Incompatible => 2,
        DependencyType::Embedded => 3,
    }
}

/// Discriminant for dedup keys (DependencyType is Copy + PartialEq but not Hash).
fn kind_key(kind: DependencyType) -> u8 {
    kind_rank(kind)
}
