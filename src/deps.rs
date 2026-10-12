//! Dependency graph over identified mods.
//!
//! Builds edges from each mod's Modrinth `dependencies` list, reports
//! missing required deps and installed incompatibilities, and renders a
//! per-mod tree for the CLI.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::types::{DependencyType, IdentifiedMod, ModVersion};

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
    /// Display name of the target when it is installed; `None` when missing.
    pub to_name: Option<String>,
    /// How the dependent declares the relationship.
    pub kind: DependencyType,
}

/// A required dependency that is not present among the identified mods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDep {
    pub dependent_project_id: String,
    pub dependent_name: String,
    pub to_project_id: String,
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
                kind: dep.dependency_type,
            });

            match dep.dependency_type {
                DependencyType::Required if target.is_none() => {
                    graph.missing.push(MissingDep {
                        dependent_project_id: from_id.to_string(),
                        dependent_name: from_name.to_string(),
                        to_project_id: to_id.to_string(),
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
            let status = if edge.to_name.is_some() {
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
            lines.push(format!(
                "  ! {} requires {}",
                miss.dependent_name, miss.to_project_id
            ));
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
