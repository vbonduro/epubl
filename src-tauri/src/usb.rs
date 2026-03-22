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
// Vendor / device identification helpers (unconditionally compiled, pure functions)
// ---------------------------------------------------------------------------

#[allow(dead_code)] // functions are called from windows_impl; unused on Linux
pub(crate) mod identification {
    /// Returns the normalised vendor name for a disk drive.
    ///
    /// Identification is performed in two steps:
    /// 1. Check `pnp_device_id` for well-known USB Vendor IDs (most reliable).
    /// 2. Fall back to a case-insensitive substring match against `model`.
    pub fn identify_vendor(pnp_device_id: &str, model: &str) -> &'static str {
        let pnp_upper = pnp_device_id.to_uppercase();
        let model_upper = model.to_uppercase();

        // --- VID-based identification ---
        if pnp_upper.contains("VID_1949") {
            return "Kindle";
        }
        if pnp_upper.contains("VID_2080") || pnp_upper.contains("VID_4173") {
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
    pub fn is_ereader(pnp_device_id: &str, model: &str) -> bool {
        identify_vendor(pnp_device_id, model) != "Unknown"
    }

    /// Parses a WMI object path like `Win32_LogicalDisk.DeviceID="E:"` and
    /// returns just the `DeviceID` value (`"E:"`).
    pub fn extract_device_id_from_path(object_path: &str) -> Option<String> {
        let key = "DeviceID=\"";
        let start = object_path.find(key)? + key.len();
        let rest = &object_path[start..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }
}

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::EReaderInfo;
    use super::identification::{extract_device_id_from_path, identify_vendor, is_ereader};

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
        dependent: String, // WMI object path of Win32_LogicalDisk (e.g. `Win32_LogicalDisk.DeviceID="E:"`)
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
            let rt: tokio::runtime::Runtime = tokio::runtime::Builder::new_current_thread()
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
        // WMI connections are COM-based and !Send — they must be created on the
        // thread that will use them. We spawn two threads upfront and let each
        // initialise its own COM + WMI connection and subscribe to events.
        let app_create = app.clone();
        let create_thread = std::thread::spawn(move || {
            let com = COMLibrary::new()?;
            let wmi = WMIConnection::new(com)?;
            let iter = wmi.raw_notification::<DiskEvent>(
                "SELECT * FROM __InstanceCreationEvent WITHIN 2 \
                 WHERE TargetInstance ISA 'Win32_DiskDrive'",
            )?;
            handle_creation_events(iter, app_create);
            Ok::<(), WMIError>(())
        });

        let app_delete = app;
        let delete_thread = std::thread::spawn(move || {
            let com = COMLibrary::new()?;
            let wmi = WMIConnection::new(com)?;
            let iter = wmi.raw_notification::<DiskEvent>(
                "SELECT * FROM __InstanceDeletionEvent WITHIN 2 \
                 WHERE TargetInstance ISA 'Win32_DiskDrive'",
            )?;
            handle_deletion_events(iter, app_delete);
            Ok::<(), WMIError>(())
        });

        // Block until both threads exit (they run indefinitely unless WMI errors).
        if let Err(e) = create_thread.join() {
            eprintln!("[usb] creation watcher thread panicked: {e:?}");
        }
        if let Err(e) = delete_thread.join() {
            eprintln!("[usb] deletion watcher thread panicked: {e:?}");
        }

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

    use windows::Win32::Foundation::{CloseHandle, HANDLE};

    /// RAII wrapper that closes a Win32 `HANDLE` on drop.
    ///
    /// This eliminates the need for manual `CloseHandle` calls and ensures the
    /// handle is always released, even on early return from errors.
    struct OwnedHandle(HANDLE);

    impl OwnedHandle {
        /// Open a raw volume device for exclusive I/O control.
        ///
        /// # Safety
        /// `CreateFileW` is an FFI call with complex preconditions; this
        /// function is the single unsafe boundary in the eject path.
        fn open_volume(volume_path: &[u16]) -> Result<Self, String> {
            use windows::core::PCWSTR;
            use windows::Win32::Storage::FileSystem::{
                CreateFileW, FILE_FLAG_NO_BUFFERING, FILE_SHARE_READ, FILE_SHARE_WRITE,
                OPEN_EXISTING,
            };
            use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};

            // SAFETY: `volume_path` is a valid null-terminated wide string
            // pointing to a volume path (`\\.\X:`). The windows-rs v0.58 wrapper
            // checks for INVALID_HANDLE_VALUE internally and returns Err on failure.
            // The returned handle is immediately wrapped in `OwnedHandle` so it
            // cannot be leaked.
            let handle = unsafe {
                CreateFileW(
                    PCWSTR(volume_path.as_ptr()),
                    GENERIC_READ.0 | GENERIC_WRITE.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_NO_BUFFERING,
                    None,
                )
            }
            .map_err(|e| format!("Could not open volume: {e}"))?;

            Ok(OwnedHandle(handle))
        }

        /// Issue a zero-argument `DeviceIoControl` call on this handle.
        fn ioctl(&self, code: u32, error_msg: &str) -> Result<(), String> {
            use windows::Win32::System::IO::DeviceIoControl;
            // SAFETY: `self.0` is a valid, open volume handle obtained from
            // `open_volume`. The ioctl codes used here (FSCTL_LOCK_VOLUME,
            // FSCTL_DISMOUNT_VOLUME, IOCTL_STORAGE_EJECT_MEDIA) all take no
            // input/output buffer, which we express with `None` / 0.
            unsafe { DeviceIoControl(self.0, code, None, 0, None, 0, None, None) }
                .map_err(|e| format!("{error_msg}: {e}"))
        }
    }

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: `self.0` is a valid handle that has not yet been closed
            // (we never duplicate or close it manually elsewhere).
            unsafe { let _ = CloseHandle(self.0); }
        }
    }

    /// Safely ejects the volume at `drive_letter` (e.g. `"E:"`).
    ///
    /// Sequence:
    /// 1. Open a handle to the raw volume (`\\.\E:`).
    /// 2. `FSCTL_LOCK_VOLUME`   — exclusive lock; fails when files are open.
    /// 3. `FSCTL_DISMOUNT_VOLUME` — flush and unmount the filesystem.
    /// 4. `IOCTL_STORAGE_EJECT_MEDIA` — signal hardware to eject.
    /// 5. Handle closed automatically when `OwnedHandle` drops.
    pub fn eject_ereader(drive_letter: &str) -> Result<(), String> {
        use windows::Win32::System::Ioctl::{
            FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME, IOCTL_STORAGE_EJECT_MEDIA,
        };

        // Build the volume path: `\\.\E:` encoded as a null-terminated wide string.
        // Strip any trailing backslash that may be present in the drive letter.
        let letter = drive_letter.trim_end_matches('\\');
        let volume_path: Vec<u16> = format!(r"\\.\{letter}")
            .encode_utf16()
            .chain(std::iter::once(0u16))
            .collect();

        let handle = OwnedHandle::open_volume(&volume_path)
            .map_err(|e| format!("Could not open volume '{letter}': {e}"))?;

        // Lock — fails if any process has files open.
        handle.ioctl(FSCTL_LOCK_VOLUME, "placeholder").map_err(|_| {
            "Could not lock device — close any open files on the eReader and try again"
                .to_string()
        })?;

        // Dismount / flush the filesystem.
        handle.ioctl(FSCTL_DISMOUNT_VOLUME, "Could not dismount volume")?;

        // Signal hardware to eject.
        handle.ioctl(IOCTL_STORAGE_EJECT_MEDIA, "Could not eject media")?;

        // `handle` drops here, automatically closing the Win32 HANDLE.
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
    #[cfg(not(feature = "e2e-mock"))]
    pub async fn list_ereaders() -> Result<Vec<EReaderInfo>, String> {
        Ok(vec![])
    }

    /// Returns a hardcoded Kindle device when the e2e-mock feature is enabled.
    #[cfg(feature = "e2e-mock")]
    pub async fn list_ereaders() -> Result<Vec<EReaderInfo>, String> {
        Ok(vec![EReaderInfo {
            drive_letter: "E:".to_string(),
            model: "Kindle Internal Storage".to_string(),
            vendor: "Kindle".to_string(),
        }])
    }

    #[cfg(not(feature = "e2e-mock"))]
    pub fn eject_ereader(_drive_letter: &str) -> Result<(), String> {
        Err("USB eject not supported on this platform".to_string())
    }

    #[cfg(feature = "e2e-mock")]
    pub fn eject_ereader(_drive_letter: &str) -> Result<(), String> {
        Err("Mocked eject error".to_string())
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

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::usb::identification::*;

    #[test]
    fn kindle_identified_by_vid() {
        assert_eq!(identify_vendor("USB\\VID_1949&PID_0004", ""), "Kindle");
    }

    #[test]
    fn kobo_identified_by_vid_2080() {
        assert_eq!(identify_vendor("USB\\VID_2080&PID_0002", ""), "Kobo");
    }

    #[test]
    fn kobo_identified_by_vid_4173() {
        assert_eq!(identify_vendor("USB\\VID_4173&PID_0001", ""), "Kobo");
    }

    #[test]
    fn kobo_identified_by_koboeread_model_name() {
        assert_eq!(identify_vendor("USB\\VID_FFFF&PID_0000", "KOBOeReader"), "Kobo");
    }

    #[test]
    fn pocketbook_identified_by_first_vid() {
        assert_eq!(identify_vendor("USB\\VID_0525&PID_A4A5", ""), "PocketBook");
    }

    #[test]
    fn pocketbook_identified_by_second_vid() {
        assert_eq!(identify_vendor("USB\\VID_2899&PID_0001", ""), "PocketBook");
    }

    #[test]
    fn bookeen_identified_by_vid() {
        assert_eq!(identify_vendor("USB\\VID_FDE8&PID_0001", ""), "Bookeen");
    }

    #[test]
    fn kindle_identified_by_model_name_fallback() {
        assert_eq!(identify_vendor("USB\\VID_FFFF&PID_0000", "Kindle Internal Storage"), "Kindle");
    }

    #[test]
    fn kobo_identified_by_model_name_fallback() {
        assert_eq!(identify_vendor("USB\\VID_FFFF&PID_0000", "Kobo eReader"), "Kobo");
    }

    #[test]
    fn unknown_device_not_classified_as_ereader() {
        assert_eq!(identify_vendor("USB\\VID_DEAD&PID_BEEF", "Generic USB Drive"), "Unknown");
    }

    #[test]
    fn identification_is_case_insensitive_for_model() {
        assert_eq!(identify_vendor("USB\\VID_FFFF&PID_0000", "kindle paperwhite"), "Kindle");
        assert_eq!(identify_vendor("USB\\VID_FFFF&PID_0000", "KOBO CLARA"), "Kobo");
    }

    #[test]
    fn vid_takes_priority_over_conflicting_model_name() {
        // VID_1949 = Kindle, but model says "Kobo" — VID wins.
        assert_eq!(identify_vendor("USB\\VID_1949&PID_0004", "Kobo eReader"), "Kindle");
    }

    #[test]
    fn is_ereader_returns_true_for_kindle_vid() {
        assert!(is_ereader("USB\\VID_1949&PID_0004", ""));
    }

    #[test]
    fn is_ereader_returns_false_for_unknown_vid_and_model() {
        assert!(!is_ereader("USB\\VID_DEAD&PID_BEEF", "Generic USB Drive"));
    }

    #[test]
    fn extract_device_id_from_well_formed_object_path() {
        let path = r#"Win32_LogicalDisk.DeviceID="E:""#;
        assert_eq!(extract_device_id_from_path(path), Some("E:".to_string()));
    }

    #[test]
    fn extract_device_id_returns_none_for_missing_key() {
        let path = "Win32_LogicalDisk.SomethingElse=\"E:\"";
        assert_eq!(extract_device_id_from_path(path), None);
    }

    #[test]
    fn extract_device_id_returns_none_for_unterminated_quote() {
        let path = "Win32_LogicalDisk.DeviceID=\"E:";
        assert_eq!(extract_device_id_from_path(path), None);
    }

    #[test]
    fn extract_device_id_handles_drive_letter_with_colon() {
        let path = r#"Win32_LogicalDisk.DeviceID="C:""#;
        let result = extract_device_id_from_path(path);
        assert_eq!(result, Some("C:".to_string()));
        assert!(result.unwrap().contains(':'));
    }
}
