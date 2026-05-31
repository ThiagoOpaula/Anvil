//! Cross-platform path resolution for mods, config, and cache directories.
//!
//! Uses the `dirs` crate for standard platform directories. Does **not**
//! create directories — just returns the paths.

use std::path::PathBuf;

/// Default Minecraft mods directory for the current platform.
///
/// - Windows: `%APPDATA%/.minecraft/mods`
/// - Linux: `$XDG_DATA_HOME/.minecraft/mods` → `~/.minecraft/mods`
/// - macOS: `~/Library/Application Support/minecraft/mods`
/// - Fallback (any platform): `./mods`
pub fn default_mods_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".minecraft")
            .join("mods")
    }

    #[cfg(target_os = "macos")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("minecraft")
            .join("mods")
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(xdg_data) = std::env::var_os("XDG_DATA_HOME") {
            PathBuf::from(xdg_data).join(".minecraft").join("mods")
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".minecraft")
                .join("mods")
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        PathBuf::from("./mods")
    }
}

/// Configuration directory for anvil.
///
/// - Windows: `%APPDATA%/anvil`
/// - Linux: `$XDG_CONFIG_HOME/anvil` → `~/.config/anvil`
/// - macOS: `~/Library/Application Support/anvil`
/// - Fallback (any platform): `./anvil`
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("anvil")
}

/// Cache directory for anvil.
///
/// - Windows: `%LOCALAPPDATA%/anvil/cache`
/// - Linux: `$XDG_CACHE_HOME/anvil` → `~/.cache/anvil`
/// - macOS: `~/Library/Caches/anvil`
/// - Fallback (any platform): `./anvil/cache`
pub fn cache_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("anvil")
            .join("cache")
    }

    #[cfg(not(target_os = "windows"))]
    {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("anvil")
    }
}

