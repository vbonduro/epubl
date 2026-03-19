use epubl_lib::config::{load_from_path, save_to_path, Config};
use tempfile::tempdir;

#[test]
fn config_round_trip_persists_all_fields() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");

    let original = Config {
        epub_folder: "/home/user/books".to_string(),
        ereader_path: Some("/media/kindle".to_string()),
        bookstore_url: "https://custom.bookstore.example.com".to_string(),
        support_email: "help@example.com".to_string(),
        first_run: false,
    };

    save_to_path(&path, &original).unwrap();
    let loaded = load_from_path(&path).unwrap();

    assert_eq!(original, loaded);
}

#[test]
fn config_round_trip_with_default_values() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");

    let default_config = Config::default();
    save_to_path(&path, &default_config).unwrap();
    let loaded = load_from_path(&path).unwrap();

    assert_eq!(default_config, loaded);
}

#[test]
fn config_round_trip_overwrites_previous_save() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");

    let first = Config {
        epub_folder: "/first/path".to_string(),
        ereader_path: None,
        bookstore_url: "https://first.example.com".to_string(),
        support_email: String::new(),
        first_run: true,
    };
    save_to_path(&path, &first).unwrap();

    let second = Config {
        epub_folder: "/second/path".to_string(),
        ereader_path: Some("/media/kobo".to_string()),
        bookstore_url: "https://second.example.com".to_string(),
        support_email: "me@example.com".to_string(),
        first_run: false,
    };
    save_to_path(&path, &second).unwrap();

    let loaded = load_from_path(&path).unwrap();
    assert_eq!(loaded, second);
    assert_ne!(loaded, first);
}

#[test]
fn corrupted_file_returns_error_on_load() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");

    std::fs::write(&path, b"not valid toml ][[[").unwrap();

    let result = load_from_path(&path);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("corrupted"),
        "Error message should mention 'corrupted', got: {err}"
    );
}

#[test]
fn no_tmp_file_left_after_save() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let tmp_path = path.with_extension("toml.tmp");

    save_to_path(&path, &Config::default()).unwrap();

    assert!(path.exists(), "config.toml should exist after save");
    assert!(
        !tmp_path.exists(),
        "config.toml.tmp should not remain after a successful save"
    );
}
