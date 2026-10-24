mod common;

use std::path::PathBuf;

use anvil::filters::apply;
use anvil::types::{FilterOpts, IdentifiedMod, ModVersion};

fn make_mod(name: &str, version: &str, loaders: &[&str], game_versions: &[&str]) -> IdentifiedMod {
    IdentifiedMod {
        path: PathBuf::from(format!("/mods/{}.jar", name)),
        sha1: "abc123".into(),
        filename: format!("{}.jar", name),
        current_version: ModVersion {
            id: format!("ver-{}", name),
            project_id: format!("proj-{}", name),
            name: name.into(),
            version_number: version.into(),
            changelog: None,
            loaders: loaders.iter().map(|s| s.to_string()).collect(),
            game_versions: game_versions.iter().map(|s| s.to_string()).collect(),
            files: vec![],
            dependencies: vec![],
        },
    }
}

#[test]
fn empty_opts_passes_all() {
    let mods = vec![make_mod("Sodium", "0.5.11", &["fabric"], &["1.21"])];
    let opts = FilterOpts::default();
    let result = apply(&mods, &opts);
    assert_eq!(result.len(), 1);
}

#[test]
fn include_substring_match() {
    let mods = vec![
        make_mod("Sodium", "0.5.11", &["fabric"], &["1.21"]),
        make_mod("Iris", "1.8.0", &["fabric"], &["1.21"]),
    ];
    let opts = FilterOpts {
        include: vec!["sod".into()], // case-insensitive substring
        ..Default::default()
    };
    let result = apply(&mods, &opts);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].current_version.name, "Sodium");
}

#[test]
fn include_wildcard() {
    let mods = vec![
        make_mod("Sodium", "0.5.11", &["fabric"], &["1.21"]),
        make_mod("Sodium Extra", "0.6.0", &["fabric"], &["1.21"]),
        make_mod("Iris", "1.8.0", &["fabric"], &["1.21"]),
    ];
    let opts = FilterOpts {
        include: vec!["Sodium*".into()],
        ..Default::default()
    };
    let result = apply(&mods, &opts);
    assert_eq!(result.len(), 2);
}

#[test]
fn include_regex() {
    let mods = vec![
        make_mod("Sodium", "0.5.11", &["fabric"], &["1.21"]),
        make_mod("Iris", "1.8.0", &["fabric"], &["1.21"]),
    ];
    let opts = FilterOpts {
        include: vec!["/^(Sodium|Iris)$/".into()],
        ..Default::default()
    };
    let result = apply(&mods, &opts);
    assert_eq!(result.len(), 2);
}

#[test]
fn exclude_filter() {
    let mods = vec![
        make_mod("Sodium", "0.5.11", &["fabric"], &["1.21"]),
        make_mod("Iris", "1.8.0", &["fabric"], &["1.21"]),
    ];
    let opts = FilterOpts {
        exclude: vec!["iris".into()],
        ..Default::default()
    };
    let result = apply(&mods, &opts);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].current_version.name, "Sodium");
}

#[test]
fn loader_filter() {
    let mods = vec![
        make_mod("Sodium", "0.5.11", &["fabric"], &["1.21"]),
        make_mod("OptiFine", "1.9", &["forge"], &["1.21"]),
    ];
    let opts = FilterOpts {
        loader: Some("fabric".into()),
        ..Default::default()
    };
    let result = apply(&mods, &opts);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].current_version.name, "Sodium");
}

#[test]
fn game_version_filter() {
    let mods = vec![
        make_mod("Sodium", "0.5.11", &["fabric"], &["1.21"]),
        make_mod("OldMod", "1.0", &["fabric"], &["1.20.4"]),
    ];
    let opts = FilterOpts {
        game_version: Some("1.21".into()),
        ..Default::default()
    };
    let result = apply(&mods, &opts);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].current_version.name, "Sodium");
}

#[test]
fn combined_filters() {
    let mods = vec![
        make_mod("Sodium", "0.5.11", &["fabric"], &["1.21"]),
        make_mod("Sodium Extra", "0.6.0", &["fabric"], &["1.21"]),
        make_mod("Iris", "1.8.0", &["fabric"], &["1.21"]),
        make_mod("ForgeMod", "1.0", &["forge"], &["1.21"]),
    ];
    let opts = FilterOpts {
        include: vec!["Sodium*".into()],
        exclude: vec!["Extra".into()],
        loader: Some("fabric".into()),
        game_version: Some("1.21".into()),
    };
    let result = apply(&mods, &opts);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].current_version.name, "Sodium");
}

#[test]
fn include_matches_version_number() {
    let mods = vec![
        make_mod("Sodium", "0.5.11", &["fabric"], &["1.21"]),
        make_mod("Iris", "1.8.0", &["fabric"], &["1.21"]),
    ];
    let opts = FilterOpts {
        include: vec!["0.5".into()],
        ..Default::default()
    };
    let result = apply(&mods, &opts);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].current_version.name, "Sodium");
}

#[test]
fn loader_filter_exempts_resource_and_shader_packs() {
    let mut pack = make_mod("Complementary", "5.0", &["minecraft"], &["1.21"]);
    pack.path = PathBuf::from("/inst/shaderpacks/Complementary.zip");
    pack.filename = "Complementary.zip".into();

    let mod_ = make_mod("Sodium", "0.5.11", &["fabric"], &["1.21"]);

    let opts = FilterOpts {
        include: vec![],
        exclude: vec![],
        loader: Some("fabric".into()),
        game_version: None,
    };

    let result = apply(&[pack, mod_], &opts);
    let names: Vec<&str> = result
        .iter()
        .map(|m| m.current_version.name.as_str())
        .collect();

    // The mod survives (its loader matches); the pack survives too, because
    // loader tags are a mod-only concept.
    assert_eq!(names, vec!["Complementary", "Sodium"]);
}
