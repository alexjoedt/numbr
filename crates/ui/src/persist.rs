use std::fs;
use std::io;
use std::path::PathBuf;

use crate::model::Settings;

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.as_os_str().is_empty())
        .map(PathBuf::from)
}

fn config_dir() -> PathBuf {
    env_path("XDG_CONFIG_HOME")
        .unwrap_or_else(|| home_dir().join(".config"))
        .join("numbr")
}

fn xdg_data_base() -> PathBuf {
    env_path("XDG_DATA_HOME").unwrap_or_else(|| home_dir().join(".local").join("share"))
}

fn home_dir() -> PathBuf {
    env_path("HOME").unwrap_or_else(|| PathBuf::from("."))
}

/// Returns the path to `$XDG_DATA_HOME/numbr/session.txt`, falling back to
/// `$HOME/.local/share/numbr/session.txt`.
fn session_path() -> PathBuf {
    xdg_data_base().join("numbr").join("session.txt")
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.toml")
}

/// Persist the editor content to disk.
pub fn save(content: &str) -> io::Result<()> {
    let path = session_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    atomic_write(&path, content)
}

/// Load the previously saved session. Returns `None` if no session file exists
/// or if reading fails.
pub fn load() -> Option<String> {
    fs::read_to_string(session_path()).ok()
}

/// Persist settings to `~/.config/numbr/settings.toml`.
pub fn save_settings(settings: &Settings) -> io::Result<()> {
    let path = settings_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let content =
        toml::to_string(settings).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    atomic_write(&path, &content)
}

/// Load settings from `~/.config/numbr/settings.toml`.
/// Falls back to `Settings::default()` on any error.
pub fn load_settings() -> Settings {
    fs::read_to_string(settings_path())
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

fn atomic_write(path: &std::path::Path, content: &str) -> io::Result<()> {
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, content)?;
    fs::rename(tmp_path, path)
}
