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
use std::path::Path;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

// ---------------------------------------------------------------------------
// Path-based functions (testable without AppHandle)
// ---------------------------------------------------------------------------

/// Loads the config from a specific file path.
///
/// Returns `Config::default()` when no config file exists (first run).
/// Returns `Err` if the file exists but cannot be parsed (corrupted).
pub fn load_from_path(path: &Path) -> Result<Config, String> {
    if !path.exists() {
        return Ok(Config::default());
    }

    let contents =
        fs::read_to_string(path).map_err(|e| format!("Could not read config file: {e}"))?;

    toml::from_str(&contents).map_err(|e| format!("Config file is corrupted: {e}"))
}

/// Saves the config to a specific file path atomically (write to `.tmp`, then rename).
pub fn save_to_path(path: &Path, config: &Config) -> Result<(), String> {
    // Ensure the parent directory exists.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create config directory: {e}"))?;
    }

    let tmp_path = path.with_extension("toml.tmp");

    let contents =
        toml::to_string_pretty(config).map_err(|e| format!("Could not serialise config: {e}"))?;

    fs::write(&tmp_path, &contents)
        .map_err(|e| format!("Could not write temporary config file: {e}"))?;

    fs::rename(&tmp_path, path)
        .map_err(|e| format!("Could not rename config file into place: {e}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// AppHandle-based wrappers
// ---------------------------------------------------------------------------

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
    load_from_path(&path)
}

/// Saves the config to disk atomically (write to `.tmp`, then rename).
pub fn save(app: &tauri::AppHandle, config: &Config) -> Result<(), String> {
    let path = config_path(app)?;
    save_to_path(&path, config)
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_are_returned_when_no_config_file_exists() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let config = load_from_path(&path).unwrap();
        let default = Config::default();

        assert_eq!(config, default);
    }

    #[test]
    fn saved_config_can_be_read_back_with_identical_values() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let original = Config {
            epub_folder: "/home/user/books".to_string(),
            ereader_path: Some("/media/kindle".to_string()),
            bookstore_url: "https://example.com".to_string(),
            first_run: false,
        };

        save_to_path(&path, &original).unwrap();
        let loaded = load_from_path(&path).unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn first_run_flag_is_true_by_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let config = load_from_path(&path).unwrap();
        assert!(config.first_run);
    }

    #[test]
    fn first_run_can_be_set_to_false_and_persisted() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut config = Config::default();
        config.first_run = false;
        save_to_path(&path, &config).unwrap();

        let loaded = load_from_path(&path).unwrap();
        assert!(!loaded.first_run);
    }

    #[test]
    fn epub_folder_path_survives_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut config = Config::default();
        config.epub_folder = "/some/path/to/ebooks".to_string();
        save_to_path(&path, &config).unwrap();

        let loaded = load_from_path(&path).unwrap();
        assert_eq!(loaded.epub_folder, "/some/path/to/ebooks");
    }

    #[test]
    fn ereader_path_none_survives_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut config = Config::default();
        config.ereader_path = None;
        save_to_path(&path, &config).unwrap();

        let loaded = load_from_path(&path).unwrap();
        assert_eq!(loaded.ereader_path, None);
    }

    #[test]
    fn ereader_path_some_survives_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut config = Config::default();
        config.ereader_path = Some("/media/KINDLE".to_string());
        save_to_path(&path, &config).unwrap();

        let loaded = load_from_path(&path).unwrap();
        assert_eq!(loaded.ereader_path, Some("/media/KINDLE".to_string()));
    }

    #[test]
    fn bookstore_url_override_survives_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut config = Config::default();
        config.bookstore_url = "https://books.example.com".to_string();
        save_to_path(&path, &config).unwrap();

        let loaded = load_from_path(&path).unwrap();
        assert_eq!(loaded.bookstore_url, "https://books.example.com");
    }

    #[test]
    fn corrupted_toml_returns_err_not_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        fs::write(&path, b"this is not valid toml = [[[").unwrap();

        let result = load_from_path(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("corrupted"),
            "Error message should contain 'corrupted', got: {err}"
        );
    }

    #[test]
    fn atomic_write_does_not_leave_tmp_file_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let tmp_path = path.with_extension("toml.tmp");

        save_to_path(&path, &Config::default()).unwrap();

        assert!(path.exists(), "config.toml should exist after save");
        assert!(
            !tmp_path.exists(),
            "config.toml.tmp should not exist after successful save"
        );
    }
}
