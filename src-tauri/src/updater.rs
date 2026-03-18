use tauri::Emitter;
#[cfg(not(feature = "e2e-update-mock"))]
use tauri_plugin_updater::UpdaterExt;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

// ---------------------------------------------------------------------------
// EventEmitter trait and implementations
// ---------------------------------------------------------------------------

/// Abstraction over Tauri event emission, enabling unit testing without
/// a real AppHandle.
pub trait EventEmitter: Send + Sync {
    fn emit_update_available(&self, info: UpdateInfo) -> Result<(), String>;
}

/// Production implementation that delegates to a real Tauri AppHandle.
pub struct TauriEmitter(pub tauri::AppHandle);

impl EventEmitter for TauriEmitter {
    fn emit_update_available(&self, info: UpdateInfo) -> Result<(), String> {
        self.0
            .emit("update-available", info)
            .map_err(|e| format!("Failed to emit update-available: {e}"))
    }
}

// ---------------------------------------------------------------------------
// Core logic
// ---------------------------------------------------------------------------

/// Handles the result of an update check.
///
/// If `update` is `Some`, emits the update-available event via `emitter`.
/// Prints to stderr on emission error; never panics.
/// If `update` is `None`, does nothing.
pub fn handle_update_result(update: Option<UpdateInfo>, emitter: &dyn EventEmitter) {
    if let Some(info) = update {
        if let Err(e) = emitter.emit_update_available(info) {
            eprintln!("[updater] failed to emit update-available: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// check_for_update — production entry point
// ---------------------------------------------------------------------------

/// Checks for updates in the background. Emits "update-available" event to
/// the frontend if a newer version is found. Silently ignores network errors
/// (no internet, server down, etc.) — never panics or blocks startup.
#[cfg(not(feature = "e2e-update-mock"))]
pub async fn check_for_update(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        match app.updater() {
            Err(e) => eprintln!("[updater] failed to get updater: {e}"),
            Ok(updater) => match updater.check().await {
                Err(e) => eprintln!("[updater] update check failed (non-fatal): {e}"),
                Ok(result) => {
                    let update_info = result.map(|update| UpdateInfo {
                        version: update.version.clone(),
                        notes: update.body.clone(),
                    });
                    let emitter = TauriEmitter(app.clone());
                    handle_update_result(update_info, &emitter);
                }
            },
        }
    });
}

/// Mock implementation for e2e testing — immediately emits version "9.9.9".
#[cfg(feature = "e2e-update-mock")]
pub async fn check_for_update(app: tauri::AppHandle) {
    let info = UpdateInfo {
        version: "9.9.9".to_string(),
        notes: None,
    };
    let emitter = TauriEmitter(app);
    handle_update_result(Some(info), &emitter);
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Records every emitted UpdateInfo for later inspection.
    struct SpyEmitter {
        emitted: Mutex<Vec<UpdateInfo>>,
    }

    impl SpyEmitter {
        fn new() -> Self {
            Self {
                emitted: Mutex::new(Vec::new()),
            }
        }

        fn emitted_items(&self) -> Vec<UpdateInfo> {
            self.emitted.lock().unwrap().clone()
        }
    }

    impl EventEmitter for SpyEmitter {
        fn emit_update_available(&self, info: UpdateInfo) -> Result<(), String> {
            self.emitted.lock().unwrap().push(info);
            Ok(())
        }
    }

    /// Always returns an error on emission.
    struct FailingEmitter;

    impl EventEmitter for FailingEmitter {
        fn emit_update_available(&self, _info: UpdateInfo) -> Result<(), String> {
            Err("Simulated emission failure".to_string())
        }
    }

    #[test]
    fn emits_update_available_when_update_is_some() {
        let emitter = SpyEmitter::new();
        let info = UpdateInfo {
            version: "1.2.3".to_string(),
            notes: None,
        };

        handle_update_result(Some(info), &emitter);

        assert_eq!(emitter.emitted_items().len(), 1);
    }

    #[test]
    fn does_not_emit_when_update_is_none() {
        let emitter = SpyEmitter::new();

        handle_update_result(None, &emitter);

        assert_eq!(emitter.emitted_items().len(), 0);
    }

    #[test]
    fn does_not_panic_when_emitter_returns_error() {
        let emitter = FailingEmitter;
        let info = UpdateInfo {
            version: "1.0.0".to_string(),
            notes: None,
        };

        // Must not panic.
        handle_update_result(Some(info), &emitter);
    }

    #[test]
    fn update_info_version_field_is_preserved() {
        let emitter = SpyEmitter::new();
        let info = UpdateInfo {
            version: "2.5.0".to_string(),
            notes: None,
        };

        handle_update_result(Some(info), &emitter);

        let items = emitter.emitted_items();
        assert_eq!(items[0].version, "2.5.0");
    }

    #[test]
    fn update_info_notes_none_is_preserved() {
        let emitter = SpyEmitter::new();
        let info = UpdateInfo {
            version: "1.0.0".to_string(),
            notes: None,
        };

        handle_update_result(Some(info), &emitter);

        let items = emitter.emitted_items();
        assert_eq!(items[0].notes, None);
    }

    #[test]
    fn update_info_notes_some_is_preserved() {
        let emitter = SpyEmitter::new();
        let info = UpdateInfo {
            version: "1.0.0".to_string(),
            notes: Some("Bug fixes and improvements".to_string()),
        };

        handle_update_result(Some(info), &emitter);

        let items = emitter.emitted_items();
        assert_eq!(items[0].notes, Some("Bug fixes and improvements".to_string()));
    }
}
