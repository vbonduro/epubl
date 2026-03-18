// Config schema for epubl
//
// Persisted as TOML in the platform config directory:
//   Windows : %APPDATA%\epubl\config.toml
//   Linux   : ~/.config/epubl/config.toml
//   macOS   : ~/Library/Application Support/epubl/config.toml
//
// Fields:
//   epub_folder   – Absolute path to the local folder that contains the user's
//                   epub files.  Must be set before the app can sync.
//
//   ereader_path  – Optional override for the eReader mount path.
//                   None (omitted in TOML) means auto-detection is used.
//
//   bookstore_url – URL opened when the user clicks the in-app bookstore link.
//                   Defaults to "https://www.amazon.com/ebooks".
//
//   first_run     – true until the user completes the setup wizard.
//                   Defaults to true; set to false by the frontend after setup.

use serde::{Deserialize, Serialize};
use std::fs;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub epub_folder: String,
    pub ereader_path: Option<String>,
    pub bookstore_url: String,
    pub first_run: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            epub_folder: String::new(),
            ereader_path: None,
            bookstore_url: String::from("https://www.amazon.com/ebooks"),
            first_run: true,
        }
    }
}

/// Returns the path to the config file, creating the config directory if it
/// does not yet exist.
fn config_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Could not resolve config directory: {e}"))?;

    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Could not create config directory: {e}"))?;

    Ok(config_dir.join("config.toml"))
}

/// Loads the config from disk.
///
/// Returns `Config::default()` when no config file exists (first run).
/// Returns `Err` if the file exists but cannot be parsed (corrupted).
pub fn load(app: &tauri::AppHandle) -> Result<Config, String> {
    let path = config_path(app)?;

    if !path.exists() {
        return Ok(Config::default());
    }

    let contents =
        fs::read_to_string(&path).map_err(|e| format!("Could not read config file: {e}"))?;

    toml::from_str(&contents).map_err(|e| format!("Config file is corrupted: {e}"))
}

/// Saves the config to disk atomically (write to `.tmp`, then rename).
pub fn save(app: &tauri::AppHandle, config: &Config) -> Result<(), String> {
    let path = config_path(app)?;
    let tmp_path = path.with_extension("toml.tmp");

    let contents =
        toml::to_string_pretty(config).map_err(|e| format!("Could not serialise config: {e}"))?;

    fs::write(&tmp_path, &contents)
        .map_err(|e| format!("Could not write temporary config file: {e}"))?;

    fs::rename(&tmp_path, &path)
        .map_err(|e| format!("Could not rename config file into place: {e}"))?;

    Ok(())
}

/// Tauri command: returns the current config, loading it from disk.
#[tauri::command]
pub fn get_config(app: tauri::AppHandle) -> Result<Config, String> {
    load(&app)
}

/// Tauri command: persists the supplied config to disk.
#[tauri::command]
pub fn set_config(app: tauri::AppHandle, config: Config) -> Result<(), String> {
    save(&app, &config)
}
