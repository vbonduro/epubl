//! File copy backend for epubl.
//!
//! Copies a list of epub files from a local folder to an eReader folder,
//! emitting a `CopyEvent` after each file is written so the frontend can
//! show a progress indicator.

use serde::Serialize;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Progress event emitted after each file is successfully copied.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CopyEvent {
    /// Filename just copied, e.g. `"great-expectations.epub"`.
    pub filename: String,
    /// Number of files copied so far (including this one).
    pub files_done: u32,
    /// Total number of files to copy.
    pub files_total: u32,
    /// Cumulative bytes copied so far (including this file).
    pub bytes_copied: u64,
    /// Total bytes across all files to copy.
    pub bytes_total: u64,
}

// ---------------------------------------------------------------------------
// Core logic (testable without Tauri)
// ---------------------------------------------------------------------------

/// Copies `filenames` from `local_folder` to `device_folder`, calling
/// `on_progress` with a [`CopyEvent`] after each file is written.
///
/// Returns an error if any file cannot be read or written.  Files already
/// copied before the error are left in place on the device.
pub fn copy_files<F>(
    filenames: &[String],
    local_folder: &str,
    device_folder: &str,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(CopyEvent),
{
    if filenames.is_empty() {
        return Ok(());
    }

    let local_dir = Path::new(local_folder);
    let device_dir = Path::new(device_folder);

    // Pre-compute sizes so we can report bytes_total up front.
    let sizes: Vec<u64> = filenames
        .iter()
        .map(|name| {
            local_dir
                .join(name)
                .metadata()
                .map(|m| m.len())
                .map_err(|e| format!("Cannot stat {name:?}: {e}"))
        })
        .collect::<Result<_, _>>()?;

    let bytes_total: u64 = sizes.iter().sum();
    let files_total = filenames.len() as u32;
    let mut bytes_copied: u64 = 0;

    for (i, (name, &size)) in filenames.iter().zip(sizes.iter()).enumerate() {
        let src = local_dir.join(name);
        let dst = device_dir.join(name);

        fs::copy(&src, &dst)
            .map_err(|e| format!("Failed to copy {name:?}: {e}"))?;

        bytes_copied += size;

        on_progress(CopyEvent {
            filename: name.clone(),
            files_done: (i + 1) as u32,
            files_total,
            bytes_copied,
            bytes_total,
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri command
// ---------------------------------------------------------------------------

/// Copies the selected epub files from the local folder to the eReader,
/// emitting `copy-progress` events after each file and `copy-complete` on
/// success.
#[cfg(not(feature = "e2e-mock"))]
#[tauri::command]
pub fn copy_epubs(
    app: tauri::AppHandle,
    filenames: Vec<String>,
    local_folder: String,
    device_folder: String,
) -> Result<(), String> {
    use tauri::Emitter;

    copy_files(&filenames, &local_folder, &device_folder, |event| {
        // Non-fatal — if the frontend disconnects mid-copy we keep going.
        let _ = app.emit("copy-progress", &event);
    })?;

    let _ = app.emit("copy-complete", ());
    Ok(())
}

/// Mock version: sleeps briefly then emits copy-complete so E2E tests can
/// observe the Syncing… state before it resolves.
#[cfg(feature = "e2e-mock")]
#[tauri::command]
pub fn copy_epubs(
    app: tauri::AppHandle,
    filenames: Vec<String>,
    _local_folder: String,
    _device_folder: String,
) -> Result<(), String> {
    use tauri::Emitter;
    let total = filenames.len() as u32;
    for (i, filename) in filenames.iter().enumerate() {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let _ = app.emit("copy-progress", &CopyEvent {
            filename: filename.clone(),
            files_done: (i + 1) as u32,
            files_total: total,
            bytes_copied: ((i + 1) * 1000) as u64,
            bytes_total: (total * 1000) as u64,
        });
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    let _ = app.emit("copy-complete", ());
    Ok(())
}
