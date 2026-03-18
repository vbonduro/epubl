use tauri_plugin_updater::UpdaterExt;
use tauri::Emitter;

#[derive(serde::Serialize, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

/// Checks for updates in the background. Emits "update-available" event to
/// the frontend if a newer version is found. Silently ignores network errors
/// (no internet, server down, etc.) — never panics or blocks startup.
pub async fn check_for_update(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        match app.updater() {
            Err(e) => eprintln!("[updater] failed to get updater: {e}"),
            Ok(updater) => match updater.check().await {
                Err(e) => eprintln!("[updater] update check failed (non-fatal): {e}"),
                Ok(None) => {}, // already up to date
                Ok(Some(update)) => {
                    let info = UpdateInfo {
                        version: update.version.clone(),
                        notes: update.body.clone(),
                    };
                    if let Err(e) = app.emit("update-available", info) {
                        eprintln!("[updater] failed to emit update-available: {e}");
                    }
                }
            }
        }
    });
}
