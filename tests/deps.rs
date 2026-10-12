//! Integration tests for `anvil::deps` — dependency graph construction
//! and tree/issue rendering.

mod common;

use anvil::deps;
use anvil::types::{Dependency, DependencyType, IdentifiedMod};
use common::helpers::{make_identified_mod, make_version, sha1_hex};

fn dep(project_id: &str, kind: DependencyType) -> Dependency {
    Dependency {
        version_id: None,
        project_id: Some(project_id.to_string()),
        dependency_type: kind,
    }
}

fn dep_version_only(version_id: &str, kind: DependencyType) -> Dependency {
    Dependency {
        version_id: Some(version_id.to_string()),
        project_id: None,
        dependency_type: kind,
    }
}

fn with_deps(mut m: IdentifiedMod, deps: Vec<Dependency>) -> IdentifiedMod {
    m.current_version.dependencies = deps;
    m
}

// ── build() ─────────────────────────────────────────────────────────────

#[test]
fn build_empty_graph_for_mods_without_deps() {
    let mods = vec![
        make_identified_mod("sodium.jar", "AAA", "Sodium", "0.6.1"),
        make_identified_mod("lithium.jar", "BBB", "Lithium", "0.13.0"),
    ];

    let graph = deps::build(&mods);

    assert!(graph.edges.is_empty());
    assert!(graph.missing.is_empty());
    assert!(graph.incompat.is_empty());
    assert!(graph.unresolved.is_empty());
    assert!(graph.is_clean());
}

#[test]
fn build_resolves_installed_required_dep() {
    let mods = vec![
        with_deps(
            make_identified_mod("iris.jar", "BBB", "Iris", "1.8.2"),
            vec![dep("AAA", DependencyType::Required)],
        ),
        make_identified_mod("sodium.jar", "AAA", "Sodium", "0.6.1"),
    ];

    let graph = deps::build(&mods);

    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].from_project_id, "BBB");
    assert_eq!(graph.edges[0].to_project_id, "AAA");
    assert_eq!(graph.edges[0].to_name.as_deref(), Some("Sodium"));
    assert_eq!(graph.edges[0].kind, DependencyType::Required);
    assert!(graph.missing.is_empty());
    assert!(graph.is_clean());
}

#[test]
fn build_reports_missing_required_dep() {
    let mods = vec![with_deps(
        make_identified_mod("create.jar", "CCC", "Create", "0.5.1"),
        vec![dep("FLYWHEEL", DependencyType::Required)],
    )];

    let graph = deps::build(&mods);

    assert_eq!(graph.edges.len(), 1);
    assert!(graph.edges[0].to_name.is_none());
    assert_eq!(graph.missing.len(), 1);
    assert_eq!(graph.missing[0].dependent_name, "Create");
    assert_eq!(graph.missing[0].to_project_id, "FLYWHEEL");
    assert!(!graph.is_clean());
}

#[test]
fn build_ignores_missing_optional_dep() {
    let mods = vec![with_deps(
        make_identified_mod("mod.jar", "AAA", "Mod A", "1.0.0"),
        vec![dep("OPTIONAL_MOD", DependencyType::Optional)],
    )];

    let graph = deps::build(&mods);

    assert_eq!(graph.edges.len(), 1);
    assert!(graph.missing.is_empty());
    assert!(graph.is_clean());
}

#[test]
fn build_ignores_missing_embedded_dep() {
    let mods = vec![with_deps(
        make_identified_mod("mod.jar", "AAA", "Mod A", "1.0.0"),
        vec![dep("BUNDLED", DependencyType::Embedded)],
    )];

    let graph = deps::build(&mods);

    assert_eq!(graph.edges.len(), 1);
    assert!(graph.missing.is_empty());
    assert!(graph.is_clean());
}

#[test]
fn build_reports_installed_incompatibility() {
    let mods = vec![
        with_deps(
            make_identified_mod("mod-a.jar", "AAA", "Mod A", "1.0.0"),
            vec![dep("BBB", DependencyType::Incompatible)],
        ),
        make_identified_mod("mod-b.jar", "BBB", "Mod B", "2.0.0"),
    ];

    let graph = deps::build(&mods);

    assert_eq!(graph.incompat.len(), 1);
    assert_eq!(graph.incompat[0].project_name, "Mod A");
    assert_eq!(graph.incompat[0].conflicts_with_name, "Mod B");
    assert!(!graph.is_clean());
}

#[test]
fn build_ignores_incompatible_when_target_not_installed() {
    let mods = vec![with_deps(
        make_identified_mod("mod-a.jar", "AAA", "Mod A", "1.0.0"),
        vec![dep("NOT_INSTALLED", DependencyType::Incompatible)],
    )];

    let graph = deps::build(&mods);

    assert!(graph.incompat.is_empty());
    assert!(graph.is_clean());
}

#[test]
fn build_records_version_only_dep_as_unresolved() {
    let mods = vec![with_deps(
        make_identified_mod("mod.jar", "AAA", "Mod A", "1.0.0"),
        vec![dep_version_only("ver-123", DependencyType::Required)],
    )];

    let graph = deps::build(&mods);

    assert!(graph.edges.is_empty());
    assert!(graph.unresolved.len() == 1);
    assert_eq!(graph.unresolved[0].version_id, "ver-123");
    // Unresolved edges never count as "missing".
    assert!(graph.missing.is_empty());
    assert!(graph.is_clean());
}

#[test]
fn build_skips_self_dependency() {
    let mods = vec![with_deps(
        make_identified_mod("mod.jar", "AAA", "Mod A", "1.0.0"),
        vec![dep("AAA", DependencyType::Required)],
    )];

    let graph = deps::build(&mods);

    assert!(graph.edges.is_empty());
    assert!(graph.missing.is_empty());
    assert!(graph.is_clean());
}

#[test]
fn build_dedupes_identical_edges() {
    let mods = vec![with_deps(
        make_identified_mod("mod.jar", "AAA", "Mod A", "1.0.0"),
        vec![
            dep("BBB", DependencyType::Required),
            dep("BBB", DependencyType::Required),
        ],
    )];

    let graph = deps::build(&mods);

    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.missing.len(), 1);
}

#[test]
fn build_is_independent_of_mod_order() {
    let a = with_deps(
        make_identified_mod("iris.jar", "BBB", "Iris", "1.8.2"),
        vec![dep("AAA", DependencyType::Required)],
    );
    let b = make_identified_mod("sodium.jar", "AAA", "Sodium", "0.6.1");

    let g1 = deps::build(&[a.clone(), b.clone()]);
    let g2 = deps::build(&[b, a]);

    assert_eq!(g1, g2);
}

// ── format_tree() ───────────────────────────────────────────────────────

#[test]
fn format_tree_shows_installed_and_missing_children() {
    let mods = vec![
        with_deps(
            make_identified_mod("iris.jar", "BBB", "Iris", "1.8.2"),
            vec![
                dep("AAA", DependencyType::Required),
                dep("ZZZ", DependencyType::Required),
            ],
        ),
        make_identified_mod("sodium.jar", "AAA", "Sodium", "0.6.1"),
    ];

    let graph = deps::build(&mods);
    let lines = deps::format_tree(&mods, &graph);
    let text = lines.join("\n");

    assert!(text.contains("Iris 1.8.2"), "tree:\n{}", text);
    assert!(text.contains("Sodium 0.6.1"), "tree:\n{}", text);
    assert!(text.contains("Sodium (required) ✓"), "tree:\n{}", text);
    assert!(text.contains("ZZZ (required) ✗ MISSING"), "tree:\n{}", text);
}

#[test]
fn format_tree_lists_mods_without_deps() {
    let mods = vec![make_identified_mod("sodium.jar", "AAA", "Sodium", "0.6.1")];
    let graph = deps::build(&mods);
    let text = deps::format_tree(&mods, &graph).join("\n");

    assert!(text.contains("(no declared dependencies)"), "tree:\n{}", text);
}

#[test]
fn format_tree_empty_input() {
    let graph = deps::build(&[]);
    let text = deps::format_tree(&[], &graph).join("\n");
    assert!(text.contains("(no identified mods)"), "tree:\n{}", text);
}

// ── format_issues() ─────────────────────────────────────────────────────

#[test]
fn format_issues_none_when_clean() {
    let mods = vec![make_identified_mod("sodium.jar", "AAA", "Sodium", "0.6.1")];
    let graph = deps::build(&mods);
    assert!(deps::format_issues(&graph).is_none());
}

#[test]
fn format_issues_lists_missing_and_incompat() {
    let mods = vec![
        with_deps(
            make_identified_mod("create.jar", "CCC", "Create", "0.5.1"),
            vec![dep("FLYWHEEL", DependencyType::Required)],
        ),
        with_deps(
            make_identified_mod("mod-a.jar", "AAA", "Mod A", "1.0.0"),
            vec![dep("BBB", DependencyType::Incompatible)],
        ),
        make_identified_mod("mod-b.jar", "BBB", "Mod B", "2.0.0"),
    ];

    let graph = deps::build(&mods);
    let issues = deps::format_issues(&graph).expect("issues expected");
    let text = issues.join("\n");

    assert!(text.contains("Missing required dependencies:"), "{}", text);
    assert!(text.contains("Create requires FLYWHEEL"), "{}", text);
    assert!(text.contains("Incompatibilities (both installed):"), "{}", text);
    assert!(text.contains("Mod A conflicts with Mod B"), "{}", text);
}

// ── resolve_display_names() ──────────────────────────────────────────────

#[tokio::test]
async fn resolve_display_names_fills_missing_titles() {
    use common::mocks::MockApi;
    use anvil::cache::ApiCache;
    use common::helpers::make_project;

    let mods = vec![with_deps(
        make_identified_mod("create.jar", "CCC", "Create", "0.5.1"),
        vec![dep("FLYWHEEL", DependencyType::Required)],
    )];

    let mut graph = deps::build(&mods);
    assert!(graph.missing[0].to_name.is_none());

    let api = MockApi::new();
    api.set_project("FLYWHEEL", make_project("FLYWHEEL", "flywheel", "Flywheel"));
    let cache = ApiCache::new();

    deps::resolve_display_names(&mut graph, &api, &cache).await;

    assert_eq!(graph.missing[0].to_name.as_deref(), Some("Flywheel"));
    assert_eq!(graph.edges[0].to_name.as_deref(), Some("Flywheel"));

    // Issues and tree now show the human title.
    let issues = deps::format_issues(&graph).expect("issues");
    assert!(issues.iter().any(|l| l.contains("Create requires Flywheel")));
    let tree = deps::format_tree(&mods, &graph).join("\n");
    assert!(tree.contains("Flywheel (required) ✗ MISSING"), "tree:\n{}", tree);
}

#[tokio::test]
async fn resolve_display_names_keeps_id_when_lookup_fails() {
    use common::mocks::MockApi;
    use anvil::cache::ApiCache;

    let mods = vec![with_deps(
        make_identified_mod("create.jar", "CCC", "Create", "0.5.1"),
        vec![dep("GHOST_PROJECT", DependencyType::Required)],
    )];

    let mut graph = deps::build(&mods);
    let api = MockApi::new(); // project_map empty → get_project errors
    let cache = ApiCache::new();

    deps::resolve_display_names(&mut graph, &api, &cache).await;

    assert!(graph.missing[0].to_name.is_none());
    let issues = deps::format_issues(&graph).expect("issues");
    assert!(issues.iter().any(|l| l.contains("Create requires GHOST_PROJECT")));
}

// ── install_missing() ────────────────────────────────────────────────────

#[tokio::test]
async fn install_missing_downloads_required_dep() {
    use common::mocks::{MockApi, MockProgress};
    use common::helpers::{make_file, make_project, setup_temp_mods_dir};
    use anvil::cache::ApiCache;
    use std::fs;

    let content = b"flywheel-jar-bytes";
    let sha1 = sha1_hex(content);

    let dir = setup_temp_mods_dir(&[("create-0.5.jar", b"create-bytes")]);
    let mods = vec![with_deps(
        make_identified_mod("create-0.5.jar", "CCC", "Create", "0.5.1"),
        vec![dep("FLYWHEEL", DependencyType::Required)],
    )];
    let graph = deps::build(&mods);

    let api = MockApi::new();
    api.set_project("FLYWHEEL", make_project("FLYWHEEL", "flywheel", "Flywheel"));
    api.set_project_versions(
        "FLYWHEEL",
        vec![make_version(
            "ver-fw",
            "FLYWHEEL",
            "Flywheel",
            "0.10.0",
            vec![make_file("https://example.test/fw.jar", "flywheel-0.10.0.jar", sha1.as_str())],
        )],
    );
    api.set_download_content(content.to_vec(), None);

    let cache = ApiCache::new();
    let progress = MockProgress::new();

    let outcome = deps::install_missing(&dir, &graph, &api, &cache, Some("fabric"), Some("1.21"), &progress)
        .await
        .unwrap();

    assert_eq!(outcome.installed.len(), 1);
    assert_eq!(outcome.installed[0].name, "Flywheel");
    assert_eq!(outcome.installed[0].filename, "flywheel-0.10.0.jar");
    assert!(outcome.failed.is_empty());
    assert!(dir.join("flywheel-0.10.0.jar").exists());
    assert_eq!(api.download_calls.lock().unwrap().len(), 1);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn install_missing_skips_existing_file_and_still_walks_transitive() {
    use common::mocks::{MockApi, MockProgress};
    use common::helpers::{make_file, make_project, setup_temp_mods_dir};
    use anvil::cache::ApiCache;
    use std::fs;

    let content = b"indium-jar";
    let sha1 = sha1_hex(content);

    let dir = setup_temp_mods_dir(&[
        ("create-0.5.jar", b"create-bytes"),
        ("flywheel-0.10.0.jar", b"already-there"),
    ]);

    let mods = vec![with_deps(
        make_identified_mod("create-0.5.jar", "CCC", "Create", "0.5.1"),
        vec![dep("FLYWHEEL", DependencyType::Required)],
    )];
    let graph = deps::build(&mods);

    let mut fw = make_version(
        "ver-fw",
        "FLYWHEEL",
        "Flywheel",
        "0.10.0",
        vec![make_file("https://example.test/fw.jar", "flywheel-0.10.0.jar", "00")],
    );
    fw.dependencies = vec![dep("INDIUM", DependencyType::Required)];

    let api = MockApi::new();
    api.set_project("FLYWHEEL", make_project("FLYWHEEL", "flywheel", "Flywheel"));
    api.set_project("INDIUM", make_project("INDIUM", "indium", "Indium"));
    api.set_project_versions("FLYWHEEL", vec![fw]);
    api.set_project_versions(
        "INDIUM",
        vec![make_version(
            "ver-in",
            "INDIUM",
            "Indium",
            "1.0.0",
            vec![make_file("https://example.test/in.jar", "indium-1.0.0.jar", sha1.as_str())],
        )],
    );
    api.set_download_content(content.to_vec(), None);

    let cache = ApiCache::new();
    let progress = MockProgress::new();

    let outcome = deps::install_missing(&dir, &graph, &api, &cache, None, None, &progress)
        .await
        .unwrap();

    // Flywheel already on disk → not re-downloaded; Indium (transitive) is.
    assert!(outcome.installed.iter().all(|d| d.project_id != "FLYWHEEL"));
    assert!(outcome.installed.iter().any(|d| d.project_id == "INDIUM"));
    // Only Indium was downloaded.
    assert_eq!(api.download_calls.lock().unwrap().len(), 1);
    assert!(dir.join("indium-1.0.0.jar").exists());

    let _ = fs::remove_dir_all(&dir);
}

// ── edges_for() ─────────────────────────────────────────────────────────

#[test]
fn edges_for_filters_by_dependent_and_sorts_by_kind() {
    let mods = vec![
        with_deps(
            make_identified_mod("mod.jar", "AAA", "Mod A", "1.0.0"),
            vec![
                dep("CCC", DependencyType::Optional),
                dep("BBB", DependencyType::Required),
            ],
        ),
        make_identified_mod("b.jar", "BBB", "Mod B", "1.0.0"),
        make_identified_mod("c.jar", "CCC", "Mod C", "1.0.0"),
    ];

    let graph = deps::build(&mods);
    let edges = graph.edges_for("AAA");

    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].kind, DependencyType::Required);
    assert_eq!(edges[0].to_project_id, "BBB");
    assert_eq!(edges[1].kind, DependencyType::Optional);
    assert_eq!(edges[1].to_project_id, "CCC");
}

// ── project_updates() ───────────────────────────────────────────────────

#[test]
fn project_updates_replaces_matching_version() {
    let original = make_identified_mod("mod-a.jar", "AAA", "Mod A", "1.0.0");
    let mods = vec![original.clone()];

    let mut latest = make_version("ver-a2", "AAA", "Mod A", "2.0.0", vec![]);
    latest.dependencies = vec![dep("BBB", DependencyType::Required)];

    let projected = deps::project_updates(&mods, &[(String::from("AAA"), latest)]);

    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].current_version.id, "ver-a2");
    assert_eq!(projected[0].current_version.version_number, "2.0.0");
    assert_eq!(projected[0].current_version.dependencies.len(), 1);
    assert_eq!(
        projected[0].current_version.dependencies[0]
            .project_id
            .as_deref(),
        Some("BBB")
    );

    // On-disk identity still comes from the original entry.
    assert_eq!(projected[0].path, original.path);
    assert_eq!(projected[0].sha1, original.sha1);
    assert_eq!(projected[0].filename, original.filename);
}

#[test]
fn project_updates_leaves_unmatched_mods_unchanged() {
    let untouched = make_identified_mod("sodium.jar", "AAA", "Sodium", "0.6.1");
    let updated = make_identified_mod("iris.jar", "BBB", "Iris", "1.8.2");
    let mods = vec![untouched.clone(), updated];

    let latest = make_version("ver-b2", "BBB", "Iris", "1.9.0", vec![]);
    let projected = deps::project_updates(&mods, &[(String::from("BBB"), latest)]);

    assert_eq!(projected.len(), 2);
    assert_eq!(projected[0].path, untouched.path);
    assert_eq!(projected[0].sha1, untouched.sha1);
    assert_eq!(projected[0].filename, untouched.filename);
    assert_eq!(
        projected[0].current_version.id,
        untouched.current_version.id
    );
    assert_eq!(projected[0].current_version.project_id, "AAA");
    assert_eq!(projected[0].current_version.version_number, "0.6.1");
    assert_eq!(projected[1].current_version.version_number, "1.9.0");
}

#[test]
fn project_updates_ignores_update_for_uninstalled_project() {
    let installed = make_identified_mod("mod-a.jar", "AAA", "Mod A", "1.0.0");
    let mods = vec![installed.clone()];

    let ghost = make_version("ver-zzz", "ZZZ", "Ghost", "9.9.9", vec![]);
    let projected = deps::project_updates(&mods, &[(String::from("ZZZ"), ghost)]);

    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].current_version.project_id, "AAA");
    assert_eq!(
        projected[0].current_version.id,
        installed.current_version.id
    );
    assert_eq!(projected[0].current_version.version_number, "1.0.0");
}

// ── newly_incompatible() ────────────────────────────────────────────────

#[test]
fn newly_incompatible_empty_when_graphs_equal() {
    let mods = vec![
        with_deps(
            make_identified_mod("mod-a.jar", "AAA", "Mod A", "1.0.0"),
            vec![dep("BBB", DependencyType::Incompatible)],
        ),
        make_identified_mod("mod-b.jar", "BBB", "Mod B", "2.0.0"),
    ];

    let graph = deps::build(&mods);
    let introduced = deps::newly_incompatible(&graph, &graph);

    assert_eq!(graph.incompat.len(), 1);
    assert!(introduced.is_empty());
}

#[test]
fn newly_incompatible_reports_pair_introduced_by_update() {
    let mods = vec![
        make_identified_mod("mod-a.jar", "AAA", "Mod A", "1.0.0"),
        make_identified_mod("mod-b.jar", "BBB", "Mod B", "2.0.0"),
    ];

    let before = deps::build(&mods);
    assert!(before.is_clean());

    let mut latest = make_version("ver-a2", "AAA", "Mod A", "2.0.0", vec![]);
    latest.dependencies = vec![dep("BBB", DependencyType::Incompatible)];
    let projected = deps::project_updates(&mods, &[(String::from("AAA"), latest)]);
    let after = deps::build(&projected);

    let introduced = deps::newly_incompatible(&before, &after);

    assert_eq!(introduced.len(), 1);
    assert_eq!(introduced[0].project_id, "AAA");
    assert_eq!(introduced[0].project_name, "Mod A");
    assert_eq!(introduced[0].conflicts_with_project_id, "BBB");
    assert_eq!(introduced[0].conflicts_with_name, "Mod B");
}

#[test]
fn newly_incompatible_ignores_preexisting_conflict() {
    let mods = vec![
        with_deps(
            make_identified_mod("mod-a.jar", "AAA", "Mod A", "1.0.0"),
            vec![dep("BBB", DependencyType::Incompatible)],
        ),
        make_identified_mod("mod-b.jar", "BBB", "Mod B", "2.0.0"),
        make_identified_mod("mod-c.jar", "CCC", "Mod C", "1.0.0"),
    ];

    let before = deps::build(&mods);
    assert_eq!(before.incompat.len(), 1);

    // Unrelated update: Mod C moves on, the conflict stays as it was.
    let latest_c = make_version("ver-c2", "CCC", "Mod C", "2.0.0", vec![]);
    let projected = deps::project_updates(&mods, &[(String::from("CCC"), latest_c)]);
    let after = deps::build(&projected);

    assert_eq!(after.incompat.len(), 1);
    assert!(deps::newly_incompatible(&before, &after).is_empty());
}

#[test]
fn newly_incompatible_ignores_removed_conflict() {
    let conflicting = vec![
        with_deps(
            make_identified_mod("mod-a.jar", "AAA", "Mod A", "1.0.0"),
            vec![dep("BBB", DependencyType::Incompatible)],
        ),
        make_identified_mod("mod-b.jar", "BBB", "Mod B", "2.0.0"),
    ];
    let resolved = vec![
        make_identified_mod("mod-a.jar", "AAA", "Mod A", "2.0.0"),
        make_identified_mod("mod-b.jar", "BBB", "Mod B", "2.0.0"),
    ];

    let before = deps::build(&conflicting);
    let after = deps::build(&resolved);

    assert_eq!(before.incompat.len(), 1);
    assert!(after.is_clean());
    assert!(deps::newly_incompatible(&before, &after).is_empty());
}

#[test]
fn newly_incompatible_matches_on_project_ids_not_names() {
    let before_mods = vec![
        with_deps(
            make_identified_mod("mod-a.jar", "AAA", "Mod A", "1.0.0"),
            vec![dep("BBB", DependencyType::Incompatible)],
        ),
        make_identified_mod("mod-b.jar", "BBB", "Mod B", "2.0.0"),
    ];

    // Same project ids, both display names changed.
    let after_mods = vec![
        with_deps(
            make_identified_mod("mod-a.jar", "AAA", "Mod A Renamed", "2.0.0"),
            vec![dep("BBB", DependencyType::Incompatible)],
        ),
        make_identified_mod("mod-b.jar", "BBB", "Mod B Renamed", "3.0.0"),
    ];

    let before = deps::build(&before_mods);
    let after = deps::build(&after_mods);

    // Struct equality would call this a brand-new pair; the id pair says no.
    assert_ne!(
        after.incompat[0].project_name,
        before.incompat[0].project_name
    );
    assert_ne!(
        after.incompat[0].conflicts_with_name,
        before.incompat[0].conflicts_with_name
    );
    assert!(deps::newly_incompatible(&before, &after).is_empty());
}

#[test]
fn newly_incompatible_detects_two_updates_conflicting() {
    let mods = vec![
        make_identified_mod("mod-a.jar", "AAA", "Mod A", "1.0.0"),
        make_identified_mod("mod-b.jar", "BBB", "Mod B", "1.0.0"),
    ];

    let before = deps::build(&mods);
    assert!(before.is_clean());

    // Both mods are updated; the conflict only exists in the new versions.
    let mut latest_a = make_version("ver-a2", "AAA", "Mod A", "2.0.0", vec![]);
    latest_a.dependencies = vec![dep("BBB", DependencyType::Incompatible)];
    let latest_b = make_version("ver-b2", "BBB", "Mod B", "2.0.0", vec![]);

    let projected = deps::project_updates(
        &mods,
        &[
            (String::from("AAA"), latest_a),
            (String::from("BBB"), latest_b),
        ],
    );
    let after = deps::build(&projected);

    let introduced = deps::newly_incompatible(&before, &after);

    // One direction only — a mutually declared conflict yields two pairs.
    assert_eq!(introduced.len(), 1);
    assert_eq!(introduced[0].project_id, "AAA");
    assert_eq!(introduced[0].conflicts_with_project_id, "BBB");
}
