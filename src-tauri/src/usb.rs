//! USB eReader detection backend for epubl.
//!
//! On Windows, uses WMI event subscriptions to watch for USB mass-storage
//! devices being connected and disconnected.  When a known eReader is
//! detected the module walks the WMI association chain
//!
//!   Win32_DiskDrive
//!     → Win32_DiskDriveToDiskPartition
//!       → Win32_LogicalDiskToPartition
//!         → Win32_LogicalDisk
//!
//! to resolve the drive letter, then emits a Tauri event to the frontend.
//!
//! On non-Windows platforms every public entry-point is a no-op stub so the
//! project compiles without changes on Linux / macOS developer machines.

use serde::Serialize;

// ---------------------------------------------------------------------------
// Shared data types (compiled on every platform)
// ---------------------------------------------------------------------------

/// Information about a detected eReader that is sent to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EReaderInfo {
    /// Drive letter assigned by the OS, e.g. `"E:"`.
    pub drive_letter: String,
    /// Device model string reported by the OS, e.g. `"Kindle Internal Storage"`.
    pub model: String,
    /// Normalised vendor name: `"Kindle"`, `"Kobo"`, `"PocketBook"`, `"Bookeen"`,
    /// or `"Unknown"`.
    pub vendor: String,
}

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::EReaderInfo;

    use serde::Deserialize;
    use tauri::Emitter;
    use tokio::task;
    use wmi::{COMLibrary, WMIConnection, WMIError};

    // -----------------------------------------------------------------------
    // WMI row types
    // -----------------------------------------------------------------------

    /// Minimal projection of `Win32_DiskDrive` used for detection.
    #[derive(Debug, Deserialize)]
    #[serde(rename = "Win32_DiskDrive")]
    #[serde(rename_all = "PascalCase")]
    struct Win32DiskDrive {
        #[serde(rename = "DeviceID")]
        device_id: String,
        #[serde(rename = "PNPDeviceID")]
        pnp_device_id: String,
        model: String,
    }

    /// Projection of an `__InstanceCreationEvent` / `__InstanceDeletionEvent`
    /// that wraps a `Win32_DiskDrive` as its `TargetInstance`.
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct DiskEvent {
        target_instance: Win32DiskDrive,
    }

    /// Used when walking the association chain to the logical disk.
    #[derive(Debug, Deserialize)]
    #[serde(rename = "Win32_DiskDriveToDiskPartition")]
    #[serde(rename_all = "PascalCase")]
    struct DiskDriveToPartition {
        dependent: String, // WMI object path of Win32_DiskPartition
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename = "Win32_LogicalDiskToPartition")]
    #[serde(rename_all = "PascalCase")]
    struct LogicalDiskToPartition {
        antecedent: String, // WMI object path of Win32_DiskPartition
        dependent: String,  // WMI object path of Win32_LogicalDisk
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename = "Win32_LogicalDisk")]
    #[serde(rename_all = "PascalCase")]
    struct Win32LogicalDisk {
        #[serde(rename = "DeviceID")]
        device_id: String, // e.g. "E:"
    }

    // -----------------------------------------------------------------------
    // Vendor / device identification helpers
    // -----------------------------------------------------------------------

    /// Returns the normalised vendor name for a disk drive.
    ///
    /// Identification is performed in two steps:
    /// 1. Check `pnp_device_id` for well-known USB Vendor IDs (most reliable).
    /// 2. Fall back to a case-insensitive substring match against `model`.
    pub(super) fn identify_vendor(pnp_device_id: &str, model: &str) -> &'static str {
        let pnp_upper = pnp_device_id.to_uppercase();
        let model_upper = model.to_uppercase();

        // --- VID-based identification ---
        if pnp_upper.contains("VID_1949") {
            return "Kindle";
        }
        if pnp_upper.contains("VID_2080") {
            return "Kobo";
        }
        if pnp_upper.contains("VID_0525") || pnp_upper.contains("VID_2899") {
            return "PocketBook";
        }
        if pnp_upper.contains("VID_FDE8") {
            return "Bookeen";
        }

        // --- Model-name fallback ---
        if model_upper.contains("KINDLE") {
            return "Kindle";
        }
        if model_upper.contains("KOBO") {
            return "Kobo";
        }
        if model_upper.contains("POCKETBOOK") {
            return "PocketBook";
        }
        if model_upper.contains("BOOKEEN") {
            return "Bookeen";
        }

        "Unknown"
    }

    /// Returns `true` when the drive matches a known eReader vendor.
    pub(super) fn is_ereader(pnp_device_id: &str, model: &str) -> bool {
        identify_vendor(pnp_device_id, model) != "Unknown"
    }

    // -----------------------------------------------------------------------
    // Drive-letter resolution
    // -----------------------------------------------------------------------

    /// Walks the WMI association chain from a `Win32_DiskDrive` `DeviceID` to
    /// the first logical drive letter (e.g. `"E:"`).
    ///
    /// Returns `None` when no logical disk can be found — for example when the
    /// device has no formatted partition that Windows has mounted yet.
    pub(super) async fn get_drive_letter(
        device_id: &str,
        wmi: &WMIConnection,
    ) -> Option<String> {
        // Escape backslashes so they survive inside the WQL string literal.
        let escaped = device_id.replace('\\', r"\\");

        // Step 1: Win32_DiskDrive → Win32_DiskDriveToDiskPartition
        let query = format!(
            "SELECT Dependent FROM Win32_DiskDriveToDiskPartition \
             WHERE Antecedent='Win32_DiskDrive.DeviceID=\"{escaped}\"'"
        );
        let partitions: Vec<DiskDriveToPartition> = wmi.raw_query(&query).ok()?;
        if partitions.is_empty() {
            return None;
        }

        // Step 2: For each partition walk → Win32_LogicalDiskToPartition
        for part in &partitions {
            // `part.dependent` is a full WMI object path such as
            // `Win32_DiskPartition.DeviceID="Disk #0, Partition #0"`.
            // We use it verbatim as the Antecedent filter value.
            let antecedent_path = &part.dependent;
            let query2 = format!(
                "SELECT Dependent FROM Win32_LogicalDiskToPartition \
                 WHERE Antecedent='{}'",
                antecedent_path.replace('\'', "\\'")
            );
            let logical_links: Vec<LogicalDiskToPartition> =
                wmi.raw_query(&query2).ok().unwrap_or_default();

            for link in &logical_links {
                // `link.dependent` is e.g. `Win32_LogicalDisk.DeviceID="E:"`.
                // Extract the drive letter from the object path.
                if let Some(drive) = extract_device_id_from_path(&link.dependent) {
                    return Some(drive);
                }
            }
        }

        None
    }

    /// Parses a WMI object path like `Win32_LogicalDisk.DeviceID="E:"` and
    /// returns just the `DeviceID` value (`"E:"`).
    fn extract_device_id_from_path(object_path: &str) -> Option<String> {
        let key = "DeviceID=\"";
        let start = object_path.find(key)? + key.len();
        let rest = &object_path[start..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    // -----------------------------------------------------------------------
    // One-shot query — list currently connected eReaders
    // -----------------------------------------------------------------------

    /// Returns all currently connected eReaders.
    ///
    /// Runs synchronous WMI calls inside `spawn_blocking` because COM is
    /// thread-affine and must not be invoked from an async context directly.
    pub async fn list_ereaders() -> Result<Vec<EReaderInfo>, String> {
        task::spawn_blocking(|| {
            let com = COMLibrary::new().map_err(|e| format!("COM init failed: {e}"))?;
            let wmi =
                WMIConnection::new(com).map_err(|e| format!("WMI connect failed: {e}"))?;

            let drives: Vec<Win32DiskDrive> = wmi
                .query()
                .map_err(|e| format!("WMI query failed: {e}"))?;

            // We need to await `get_drive_letter` inside spawn_blocking, so
            // create a dedicated single-threaded Tokio runtime here.
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio runtime build failed: {e}"))?;

            let mut results = Vec::new();
            for drive in drives {
                if !is_ereader(&drive.pnp_device_id, &drive.model) {
                    continue;
                }
                let vendor = identify_vendor(&drive.pnp_device_id, &drive.model);
                let drive_letter = rt
                    .block_on(get_drive_letter(&drive.device_id, &wmi))
                    .unwrap_or_default();

                results.push(EReaderInfo {
                    drive_letter,
                    model: drive.model.clone(),
                    vendor: vendor.to_string(),
                });
            }

            Ok(results)
        })
        .await
        .map_err(|e| format!("spawn_blocking join error: {e}"))?
    }

    // -----------------------------------------------------------------------
    // Background event watcher
    // -----------------------------------------------------------------------

    /// Spawns a background Tokio task that watches for USB disk-drive events
    /// via WMI and emits `ereader-connected` / `ereader-disconnected` Tauri
    /// events to the frontend.
    ///
    /// Returns immediately; the watcher runs for the lifetime of the process.
    pub async fn watch_ereader(app: tauri::AppHandle) {
        tokio::spawn(async move {
            let result = task::spawn_blocking(move || watch_blocking(app)).await;
            if let Err(e) = result {
                eprintln!("[usb] watcher task panicked: {e}");
            }
        });
    }

    /// Blocking WMI event loop — must run on a thread where COM is initialised
    /// (guaranteed by `spawn_blocking`).
    ///
    /// Subscribes to `__InstanceCreationEvent` and `__InstanceDeletionEvent`
    /// for `Win32_DiskDrive` with a 2-second polling interval.  Spins up two
    /// OS threads — one per event type — so neither subscription starves the
    /// other.
    fn watch_blocking(app: tauri::AppHandle) -> Result<(), WMIError> {
        // Build the WQL event queries.
        let create_wql = "SELECT * FROM __InstanceCreationEvent WITHIN 2 \
                           WHERE TargetInstance ISA 'Win32_DiskDrive'";
        let delete_wql = "SELECT * FROM __InstanceDeletionEvent WITHIN 2 \
                           WHERE TargetInstance ISA 'Win32_DiskDrive'";

        // Each subscription needs its own COM + WMI connection because WMI
        // connections are not Send.
        let com_create = COMLibrary::new()?;
        let wmi_create = WMIConnection::new(com_create)?;
        let creation_iter = wmi_create.notification::<DiskEvent>(create_wql)?;

        let com_delete = COMLibrary::new()?;
        let wmi_delete = WMIConnection::new(com_delete)?;
        let deletion_iter = wmi_delete.notification::<DiskEvent>(delete_wql)?;

        let app_create = app.clone();
        let create_thread =
            std::thread::spawn(move || handle_creation_events(creation_iter, app_create));

        let app_delete = app;
        let delete_thread =
            std::thread::spawn(move || handle_deletion_events(deletion_iter, app_delete));

        // Block until both threads exit (they run indefinitely unless WMI errors).
        let _ = create_thread.join();
        let _ = delete_thread.join();

        Ok(())
    }

    /// Processes `__InstanceCreationEvent` items and emits `ereader-connected`.
    fn handle_creation_events(
        iter: impl Iterator<Item = Result<DiskEvent, WMIError>>,
        app: tauri::AppHandle,
    ) {
        // A tiny single-threaded runtime for the async drive-letter lookup.
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("[usb] tokio runtime build failed in creation handler: {e}");
                return;
            }
        };

        // A dedicated WMI connection for the drive-letter walk (the iterator's
        // connection must stay alive for the subscription lifetime and is not
        // available here).
        let com = match COMLibrary::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[usb] COM init failed in creation thread: {e}");
                return;
            }
        };
        let wmi = match WMIConnection::new(com) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[usb] WMI connect failed in creation thread: {e}");
                return;
            }
        };

        for event in iter {
            match event {
                Err(e) => {
                    eprintln!("[usb] WMI creation event error: {e}");
                    break;
                }
                Ok(ev) => {
                    let drive = &ev.target_instance;
                    if !is_ereader(&drive.pnp_device_id, &drive.model) {
                        continue;
                    }

                    let vendor = identify_vendor(&drive.pnp_device_id, &drive.model);
                    // The drive letter may not be available instantly; we try
                    // once and send an empty string if not yet mounted.
                    let drive_letter = rt
                        .block_on(get_drive_letter(&drive.device_id, &wmi))
                        .unwrap_or_default();

                    let info = EReaderInfo {
                        drive_letter,
                        model: drive.model.clone(),
                        vendor: vendor.to_string(),
                    };

                    if let Err(e) = app.emit("ereader-connected", &info) {
                        eprintln!("[usb] failed to emit ereader-connected: {e}");
                    }
                }
            }
        }
    }

    /// Processes `__InstanceDeletionEvent` items and emits `ereader-disconnected`.
    fn handle_deletion_events(
        iter: impl Iterator<Item = Result<DiskEvent, WMIError>>,
        app: tauri::AppHandle,
    ) {
        for event in iter {
            match event {
                Err(e) => {
                    eprintln!("[usb] WMI deletion event error: {e}");
                    break;
                }
                Ok(ev) => {
                    let drive = &ev.target_instance;
                    if !is_ereader(&drive.pnp_device_id, &drive.model) {
                        continue;
                    }

                    // The drive letter is gone by the time the deletion event
                    // fires, so we send an empty string for that field.
                    let info = EReaderInfo {
                        drive_letter: String::new(),
                        model: drive.model.clone(),
                        vendor: identify_vendor(&drive.pnp_device_id, &drive.model)
                            .to_string(),
                    };

                    if let Err(e) = app.emit("ereader-disconnected", &info) {
                        eprintln!("[usb] failed to emit ereader-disconnected: {e}");
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Eject
    // -----------------------------------------------------------------------

    /// Safely ejects the volume at `drive_letter` (e.g. `"E:"`).
    ///
    /// Sequence:
    /// 1. Open a handle to the raw volume (`\\.\E:`).
    /// 2. `FSCTL_LOCK_VOLUME`   — exclusive lock; fails when files are open.
    /// 3. `FSCTL_DISMOUNT_VOLUME` — flush and unmount the filesystem.
    /// 4. `IOCTL_STORAGE_EJECT_MEDIA` — signal hardware to eject.
    /// 5. Close handle (automatic on drop via `CloseHandle`).
    pub fn eject_ereader(drive_letter: &str) -> Result<(), String> {
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, FILE_ACCESS_RIGHTS, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        };
        use windows::Win32::System::IO::DeviceIoControl;
        use windows::Win32::System::Ioctl::{
            FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME, IOCTL_STORAGE_EJECT_MEDIA,
        };
        use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};

        // Build the volume path: `\\.\E:` encoded as a null-terminated wide string.
        // Strip any trailing backslash that may be present in the drive letter.
        let letter = drive_letter.trim_end_matches('\\');
        let volume_path: Vec<u16> = format!(r"\\.\{}", letter)
            .encode_utf16()
            .chain(std::iter::once(0u16))
            .collect();

        // Step 1: open the volume.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(volume_path.as_ptr()),
                FILE_ACCESS_RIGHTS(GENERIC_READ.0 | GENERIC_WRITE.0),
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                Default::default(),
                None,
            )
        }
        .map_err(|e| format!("Could not open volume '{}': {}", letter, e))?;

        if handle == INVALID_HANDLE_VALUE {
            return Err(format!("Could not open volume '{}': invalid handle", letter));
        }

        // Helper: run a zero-argument DeviceIoControl call.
        let ioctl = |code: u32, error_msg: &str| -> Result<(), String> {
            unsafe {
                DeviceIoControl(handle, code, None, 0, None, 0, None, None)
            }
            .map_err(|e| format!("{}: {}", error_msg, e))
        };

        // Step 2: lock the volume — fails if any process has files open.
        ioctl(FSCTL_LOCK_VOLUME, "placeholder").map_err(|_| {
            "Could not lock device — close any open files on the eReader and try again"
                .to_string()
        })?;

        // Step 3: dismount / flush the filesystem.
        if let Err(e) = ioctl(FSCTL_DISMOUNT_VOLUME, "Could not dismount volume") {
            // Best-effort close before returning the error.
            unsafe { let _ = CloseHandle(handle); }
            return Err(e);
        }

        // Step 4: eject the media.
        if let Err(e) = ioctl(IOCTL_STORAGE_EJECT_MEDIA, "Could not eject media") {
            unsafe { let _ = CloseHandle(handle); }
            return Err(e);
        }

        // Step 5: close the handle.
        unsafe { let _ = CloseHandle(handle); }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Non-Windows stubs
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
mod platform_stubs {
    use super::EReaderInfo;

    /// No-op on non-Windows platforms.
    pub async fn watch_ereader(_app: tauri::AppHandle) {}

    /// Always returns an empty list on non-Windows platforms.
    pub async fn list_ereaders() -> Result<Vec<EReaderInfo>, String> {
        Ok(vec![])
    }

    pub fn eject_ereader(_drive_letter: &str) -> Result<(), String> {
        Err("USB eject not supported on this platform".to_string())
    }
}

// ---------------------------------------------------------------------------
// Re-export the platform-appropriate implementations under a unified API
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
pub use windows_impl::{eject_ereader, list_ereaders, watch_ereader};

#[cfg(not(target_os = "windows"))]
pub use platform_stubs::{eject_ereader, list_ereaders, watch_ereader};

// ---------------------------------------------------------------------------
// Tauri commands (always compiled — registered in main.rs / lib.rs)
// ---------------------------------------------------------------------------

/// Returns a snapshot of all currently connected eReaders.
///
/// On non-Windows platforms the list is always empty.
#[tauri::command]
pub async fn get_connected_ereaders() -> Result<Vec<EReaderInfo>, String> {
    list_ereaders().await
}

/// Ejects the eReader at `drive_letter`.
///
/// Currently returns an error on all platforms; full implementation pending.
#[tauri::command]
pub fn eject(drive_letter: String) -> Result<(), String> {
    eject_ereader(&drive_letter)
}
