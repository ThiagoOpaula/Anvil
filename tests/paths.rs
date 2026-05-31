mod common;

use anvil::paths::*;

#[test]
fn default_mods_dir_ends_with_minecraft_mods() {
    let dir = default_mods_dir();
    let path_str = dir.to_string_lossy().to_lowercase();
    // On all supported platforms, the path should contain minecraft/mods
    // or just end with "mods" on fallback.
    assert!(
        path_str.contains("minecraft") || path_str.ends_with("mods"),
        "expected path to contain minecraft or end with mods, got: {}",
        dir.display()
    );
}

#[test]
fn config_dir_ends_with_anvil() {
    let dir = config_dir();
    assert!(
        dir.ends_with("anvil")
            || dir.to_string_lossy().to_lowercase().contains("anvil"),
        "expected config dir to end with anvil, got: {}",
        dir.display()
    );
}

#[test]
fn cache_dir_contains_anvil() {
    let dir = cache_dir();
    assert!(
        dir.to_string_lossy()
            .to_lowercase()
            .contains("anvil"),
        "expected cache dir to contain anvil, got: {}",
        dir.display()
    );
}

#[test]
fn paths_are_absolute_or_relative() {
    // All path functions should return something non-empty.
    assert!(!default_mods_dir().as_os_str().is_empty());
    assert!(!config_dir().as_os_str().is_empty());
    assert!(!cache_dir().as_os_str().is_empty());
}
