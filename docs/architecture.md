# epubl Architecture

epubl is a dead-simple Windows GUI desktop app that helps non-technical users load epub books onto their eReader. The developer works on Linux and cross-compiles for Windows via CI.

## Tech Stack

| Layer | Choice | Rationale |
|---|---|---|
| Framework | Tauri v2 | Small installer, built-in updater, Rust backend |
| Frontend | Svelte v5 + Vite | Compiled-away framework, minimal bundle, reactive |
| CI / Release | GitHub Actions `windows-latest` | Only supported path for NSIS installer from Linux |
| Auto-updates | `tauri-plugin-updater` + `latest.json` on GH Releases | No extra infrastructure |
| Installer | NSIS, `perUser` install mode | No admin elevation, installs to `%LOCALAPPDATA%` |
| USB detection | `wmi` crate — `Win32_DiskDrive` | Ergonomic async WMI queries from Rust |
| Device events | `windows-rs` — `CM_Register_Notification` | Truly event-driven, no window required |
| Safe eject | `windows-rs` — `DeviceIoControl` | Microsoft-maintained Win32 bindings |

## Bundle Size

Tauri uses the system WebView2 (pre-installed on Windows 10 1803+ and all Windows 11) rather than bundling a browser engine:

- NSIS installer: **~3–5 MB**
- Installed footprint: ~10–15 MB
- Compare: Electron = 70–150 MB installer

## Project Layout (planned)

```
epubl/
├── src/                  # Svelte frontend
│   ├── App.svelte
│   ├── lib/
│   │   ├── Library.svelte      # Local epub list panel
│   │   ├── Device.svelte       # eReader status + file list
│   │   └── SyncControls.svelte # Copy, eject, progress
│   └── main.ts
├── src-tauri/            # Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── config.rs           # Read/write app config
│   │   ├── epub.rs             # Folder scanning + diff logic
│   │   ├── usb.rs              # eReader detection + eject
│   │   └── updater.rs          # Auto-update check on startup
│   ├── Cargo.toml
│   └── tauri.conf.json
├── .github/
│   └── workflows/
│       └── release.yml         # Build + publish Windows installer
└── docs/
    └── architecture.md
```

## Cross-Compilation

Local Linux → Windows cross-compilation is **not supported** for producing an NSIS installer — Tauri's build tooling assumes a Windows host for that step.

**Development workflow:**
1. Write and iterate Rust backend on Linux
2. Gate all Win32 code with `#[cfg(target_os = "windows")]`; provide stub implementations on Linux so the codebase compiles locally
3. Push to GitHub → CI builds the Windows NSIS installer on a `windows-latest` runner
4. `tauri-apps/tauri-action` handles build + upload to GitHub Release

## Auto-Updates

`tauri-plugin-updater` checks a `latest.json` manifest on startup and prompts the user if a new version is available.

**Update manifest format** (`latest.json`, uploaded as a GitHub Release asset):

```json
{
  "version": "1.2.0",
  "notes": "Fixed USB detection on Windows 11",
  "pub_date": "2025-06-01T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<base64-minisign-signature>",
      "url": "https://github.com/vbonduro/epubl/releases/download/v1.2.0/epubl_1.2.0_x64-setup.exe"
    }
  }
}
```

**Signing setup:**
- Generate keypair: `cargo tauri signer generate -w ~/.tauri/epubl.key`
- Private key → GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY`
- Public key → `tauri.conf.json` under `plugins.updater.pubkey`
- Signing is mandatory in Tauri — cannot be disabled in production builds

## USB eReader Detection

Detection uses a hybrid approach: event-driven arrival notification + a one-shot WMI query to identify the device.

**On device arrival:**
1. `CM_Register_Notification` (via `windows-rs`) fires a callback with no polling overhead
2. Follow-up WMI query walks `Win32_DiskDrive → Win32_DiskDriveToDiskPartition → Win32_LogicalDiskToPartition → Win32_LogicalDisk` to get the drive letter
3. Check `PNPDeviceID` for known vendor IDs; fall back to `Model` substring matching
4. Emit Tauri event `ereader-connected { drive_letter, model, vendor }` to the frontend

**Known eReader USB Vendor IDs** (in `PNPDeviceID` as `VID_xxxx`):

| VID | Manufacturer | Devices |
|---|---|---|
| `0x1949` | Amazon | All Kindle models |
| `0x2080` | Rakuten Kobo | Clara, Libra, Sage, Elipsa, etc. |
| `0x0525` / `0x2899` | PocketBook | Various models |
| `0xFDE8` | Bookeen | Cybook series |
| `0x2207` | Onyx Boox | Rockchip VID — also match on `Model` name |

**Simpler fallback** if `CM_Register_Notification` proves complex: WMI `__InstanceCreationEvent WITHIN 2` for `Win32_DiskDrive`. Introduces up to 2s detection latency, but acceptable for a sync tool and requires no unsafe code.

## Safe Eject Sequence

Via `DeviceIoControl` (windows-rs):

1. Open handle to volume: `\\.\E:`
2. `FSCTL_LOCK_VOLUME` — prevents new I/O (fails if files are open; show user-friendly error)
3. `FSCTL_DISMOUNT_VOLUME` — flushes and unmounts the filesystem
4. `IOCTL_STORAGE_EJECT_MEDIA` — signals hardware to eject

## Cargo Dependencies (Windows-only)

```toml
[target.'cfg(target_os = "windows")'.dependencies]
wmi = "0.14"
windows = { version = "0.58", features = [
    "Win32_Devices_DeviceAndDriverInstallation",
    "Win32_Storage_FileSystem",
    "Win32_System_Ioctl",
    "Win32_System_Wmi",
] }
```

WMI COM calls must run inside `tokio::task::spawn_blocking` — COM initialization is thread-affine and cannot be called directly from a Tokio async context.

## Gotchas

- **WebView2**: Pre-installed on Windows 10 1803+ and all Windows 11. Include the WebView2 bootstrapper in the NSIS config (`webviewInstallMode`) for older Windows 10 edge cases.
- **Volume locking**: `FSCTL_LOCK_VOLUME` fails if any process has open file handles on the eReader. Handle gracefully with a clear error message ("Close any open files on the device and try again").
- **NSIS `perUser` mode**: Installs to `%LOCALAPPDATA%`, no admin prompt. Auto-updates also apply per-user without elevation.
- **`#[cfg]` guards**: All Win32/WMI code must be gated so the project builds on Linux for development. Provide `Ok(())` or `Err("not supported on this platform")` stubs.
