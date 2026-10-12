mod common;

use common::helpers::unique_temp_dir;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anvil::discover::{
    DiscoveredModsDir, LauncherKind, RootKind, RootSpec, discover_from, official_custom_game_dirs,
    root_specs,
};

// ── Helpers ─────────────────────────────────────────────────────────

/// A fresh, empty temp directory (removed first if a stale one exists).
fn temp_root(label: &str) -> PathBuf {
    let dir = unique_temp_dir(label);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn spec(launcher: LauncherKind, root: &Path, kind: RootKind) -> RootSpec {
    RootSpec {
        launcher,
        root: root.to_path_buf(),
        kind,
    }
}

/// Expected paths are compared canonically, since `discover_from` canonicalizes
/// and some platforms (macOS `/var` → `/private/var`) resolve temp dirs.
fn canon(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn mods_dirs(found: &[DiscoveredModsDir]) -> Vec<PathBuf> {
    found.iter().map(|d| d.mods_dir.clone()).collect()
}

// ── RootKind behaviour ──────────────────────────────────────────────

#[test]
fn discover_from_empty_specs_returns_nothing() {
    assert!(discover_from(&[]).is_empty());
}

#[test]
fn discover_from_missing_root_is_silent() {
    let root = unique_temp_dir("discover-missing");
    let _ = fs::remove_dir_all(&root);

    let specs = [
        spec(LauncherKind::Official, &root, RootKind::GameDir),
        spec(LauncherKind::Prism, &root, RootKind::InstancesDir),
        spec(LauncherKind::ModrinthApp, &root, RootKind::ProfilesDir),
        spec(LauncherKind::Official, &root, RootKind::ModsDir),
    ];
    assert!(discover_from(&specs).is_empty());
}

#[test]
fn discover_from_file_root_is_silent() {
    let root = temp_root("discover-file-root");

    // A regular file named "mods" is not a mods folder.
    fs::write(root.join("mods"), b"not a directory").unwrap();
    let specs = [spec(LauncherKind::Official, &root, RootKind::GameDir)];
    assert!(discover_from(&specs).is_empty());

    // Same for a ModsDir spec pointed straight at a file.
    let file = root.join("just-a-file");
    fs::write(&file, b"x").unwrap();
    let specs = [spec(LauncherKind::Official, &file, RootKind::ModsDir)];
    assert!(discover_from(&specs).is_empty());
}

#[test]
fn discover_from_mods_dir_kind_hits_root() {
    let root = temp_root("discover-modsdir");
    fs::write(root.join("sodium.jar"), b"jar").unwrap();
    fs::write(root.join("iris.jar"), b"jar").unwrap();
    fs::write(root.join("readme.txt"), b"not a jar").unwrap();

    let specs = [spec(LauncherKind::Official, &root, RootKind::ModsDir)];
    let found = discover_from(&specs);

    assert_eq!(mods_dirs(&found), vec![canon(&root)]);
    assert_eq!(found[0].jar_count, 2);
    assert_eq!(found[0].launcher, LauncherKind::Official);
    assert_eq!(found[0].root, root);
}

#[test]
fn discover_from_game_dir_kind_requires_mods_child() {
    let root = temp_root("discover-gamedir");
    let specs = [spec(LauncherKind::Official, &root, RootKind::GameDir)];
    assert!(discover_from(&specs).is_empty());

    fs::create_dir_all(root.join("mods")).unwrap();
    let found = discover_from(&specs);
    assert_eq!(mods_dirs(&found), vec![canon(&root.join("mods"))]);
}

#[test]
fn discover_from_instances_dir_prefers_minecraft_then_dot_minecraft_then_mods() {
    let root = temp_root("discover-instances");

    let all_three = root.join("all-three");
    fs::create_dir_all(all_three.join("minecraft/mods")).unwrap();
    fs::create_dir_all(all_three.join(".minecraft/mods")).unwrap();
    fs::create_dir_all(all_three.join("mods")).unwrap();

    let dot_minecraft = root.join("dot-minecraft");
    fs::create_dir_all(dot_minecraft.join(".minecraft/mods")).unwrap();
    fs::create_dir_all(dot_minecraft.join("mods")).unwrap();

    let bare_mods = root.join("bare-mods");
    fs::create_dir_all(bare_mods.join("mods")).unwrap();

    let specs = [spec(LauncherKind::Prism, &root, RootKind::InstancesDir)];
    let found = discover_from(&specs);

    assert_eq!(
        mods_dirs(&found),
        vec![
            canon(&all_three.join("minecraft/mods")),
            canon(&bare_mods.join("mods")),
            canon(&dot_minecraft.join(".minecraft/mods")),
        ]
    );
}

#[test]
fn discover_from_instances_dir_does_not_double_count() {
    let root = temp_root("discover-no-double");
    let instance = root.join("instance");
    fs::create_dir_all(instance.join("minecraft/mods")).unwrap();
    fs::create_dir_all(instance.join("mods")).unwrap();

    let specs = [spec(LauncherKind::Prism, &root, RootKind::InstancesDir)];
    let found = discover_from(&specs);
    assert_eq!(found.len(), 1, "one instance must contribute one folder");
}

#[test]
fn discover_from_instances_dir_skips_dotdirs_and_files() {
    let root = temp_root("discover-skip");

    // Hidden instance directory — not a launcher instance.
    fs::create_dir_all(root.join(".tmp/minecraft/mods")).unwrap();
    // Plain file at the instance level.
    fs::write(root.join("instgroups.json"), b"{}").unwrap();
    // A real instance.
    fs::create_dir_all(root.join("real/minecraft/mods")).unwrap();

    let specs = [spec(LauncherKind::Prism, &root, RootKind::InstancesDir)];
    let found = discover_from(&specs);

    assert_eq!(
        mods_dirs(&found),
        vec![canon(&root.join("real/minecraft/mods"))]
    );
}

#[test]
fn discover_from_profiles_dir_hits_child_mods() {
    let root = temp_root("discover-profiles");

    let first = root.join("first");
    fs::create_dir_all(first.join("mods")).unwrap();
    fs::create_dir_all(first.join(".minecraft/mods")).unwrap();

    let second = root.join("second");
    fs::create_dir_all(second.join("mods")).unwrap();

    let specs = [spec(
        LauncherKind::ModrinthApp,
        &root,
        RootKind::ProfilesDir,
    )];
    let found = discover_from(&specs);

    // "mods" wins over ".minecraft/mods" within a profile; one row per profile.
    assert_eq!(
        mods_dirs(&found),
        vec![canon(&first.join("mods")), canon(&second.join("mods"))]
    );
}

// ── Dedup + ordering ────────────────────────────────────────────────

#[test]
fn discover_from_dedups_identical_paths() {
    let root = temp_root("discover-dedup");
    fs::create_dir_all(root.join("mods")).unwrap();

    let specs = [
        spec(LauncherKind::Official, &root, RootKind::GameDir),
        spec(LauncherKind::Prism, &root, RootKind::GameDir),
    ];
    let found = discover_from(&specs);

    assert_eq!(found.len(), 1);
    // The first spec wins and keeps its identity for display.
    assert_eq!(found[0].launcher, LauncherKind::Official);
    assert_eq!(found[0].mods_dir, canon(&root.join("mods")));
}

#[cfg(unix)]
#[test]
fn discover_from_dedups_symlink_to_same_dir() {
    let real = temp_root("discover-symlink-real");
    fs::create_dir_all(real.join("mods")).unwrap();

    let link_root = temp_root("discover-symlink-link");
    let link = link_root.join("instance");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let specs = [
        spec(LauncherKind::Official, &real, RootKind::GameDir),
        spec(LauncherKind::Prism, &link_root, RootKind::InstancesDir),
    ];
    let found = discover_from(&specs);

    assert_eq!(found.len(), 1, "a symlinked mods folder is one folder");
}

#[test]
fn discover_from_output_is_deterministically_ordered() {
    let root = temp_root("discover-order");

    // Created in an order that does not match the expected sort.
    for name in ["zeta", "alpha", "mid"] {
        fs::create_dir_all(root.join(name).join("minecraft/mods")).unwrap();
    }

    let specs = [spec(LauncherKind::Prism, &root, RootKind::InstancesDir)];
    let first = discover_from(&specs);
    let second = discover_from(&specs);

    assert_eq!(first, second, "repeated runs must agree");
    assert_eq!(
        mods_dirs(&first),
        vec![
            canon(&root.join("alpha/minecraft/mods")),
            canon(&root.join("mid/minecraft/mods")),
            canon(&root.join("zeta/minecraft/mods")),
        ]
    );
}

#[test]
fn discover_from_never_reports_nonexistent_path() {
    let root = temp_root("discover-invariant");

    fs::create_dir_all(root.join("present/minecraft/mods")).unwrap();
    fs::create_dir_all(root.join("empty-instance")).unwrap();
    fs::write(root.join("not-a-dir"), b"x").unwrap();

    let specs = [
        spec(LauncherKind::Prism, &root, RootKind::InstancesDir),
        spec(
            LauncherKind::Official,
            &root.join("gone"),
            RootKind::GameDir,
        ),
        spec(LauncherKind::Official, &root, RootKind::GameDir),
    ];

    for entry in discover_from(&specs) {
        assert!(
            entry.mods_dir.exists(),
            "reported a nonexistent path: {}",
            entry.mods_dir.display()
        );
        assert!(entry.mods_dir.is_dir());
    }
}

// ── official_custom_game_dirs ───────────────────────────────────────

#[test]
fn official_custom_game_dirs_parses_string_and_object_gameDir() {
    let root = temp_root("discover-launcher-profiles");
    let json = root.join("launcher_profiles.json");
    fs::write(
        &json,
        r#"{
            "profiles": {
                "one": { "name": "one", "gameDir": "/games/one" },
                "two": { "name": "two", "gameDir": { "path": "/games/two" } }
            }
        }"#,
    )
    .unwrap();

    let mut dirs = official_custom_game_dirs(&[json]);
    dirs.sort();
    assert_eq!(
        dirs,
        vec![PathBuf::from("/games/one"), PathBuf::from("/games/two")]
    );
}

#[test]
fn official_custom_game_dirs_tolerates_missing_and_malformed() {
    let root = temp_root("discover-launcher-bad");

    let missing = root.join("does-not-exist.json");
    let malformed = root.join("malformed.json");
    fs::write(&malformed, b"{ not json at all").unwrap();
    let no_profiles = root.join("no-profiles.json");
    fs::write(&no_profiles, br#"{"profiles": "not an object"}"#).unwrap();

    assert!(official_custom_game_dirs(&[missing, malformed, no_profiles]).is_empty());
    assert!(official_custom_game_dirs(&[]).is_empty());
}

#[test]
fn official_custom_game_dirs_ignores_profiles_without_game_dir() {
    let root = temp_root("discover-launcher-empty");
    let json = root.join("launcher_profiles.json");
    fs::write(
        &json,
        r#"{
            "profiles": {
                "default": { "name": "1.21", "lastVersionId": "1.21" },
                "custom": { "name": "custom", "gameDir": "/games/custom" }
            }
        }"#,
    )
    .unwrap();

    let dirs = official_custom_game_dirs(&[json]);
    assert_eq!(dirs, vec![PathBuf::from("/games/custom")]);
}

// ── root_specs ──────────────────────────────────────────────────────

#[test]
fn root_specs_have_no_duplicate_root_and_kind() {
    // Assert uniqueness only — never a count, since the flatpak block is
    // cfg-gated and $HOME varies.
    let specs = root_specs();
    let mut seen = HashSet::new();
    for spec in &specs {
        assert!(
            seen.insert((spec.root.clone(), spec.kind)),
            "duplicate root/kind: {:?}",
            spec
        );
    }
}
