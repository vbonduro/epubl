//! Simple rolling file logger for epubl.
//!
//! Writes timestamped lines to `<app-config-dir>/epubl.log`.
//! When the file exceeds `MAX_LOG_BYTES` it is renamed to `epubl.log.old`
//! before a fresh log is started (one generation of backup).
//!
//! All public functions are infallible — logging failures are silently ignored
//! so they never affect the user-facing behaviour of the app.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Maximum size of `epubl.log` before it is rotated (1 MiB).
const MAX_LOG_BYTES: u64 = 1024 * 1024;

// ---------------------------------------------------------------------------
// Global logger state
// ---------------------------------------------------------------------------

struct Logger {
    path: PathBuf,
}

static LOGGER: Mutex<Option<Logger>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Initialises the logger with the path to the app config directory.
///
/// Must be called once during app startup before any `log!` calls.
/// Safe to call multiple times — only the first call takes effect.
pub fn init(config_dir: &Path) {
    let path = config_dir.join("epubl.log");
    let mut guard = LOGGER.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(Logger { path });
    }
}

/// Writes a formatted log line.
///
/// Use the [`log!`] macro instead of calling this directly.
pub fn write_line(line: &str) {
    let guard = LOGGER.lock().unwrap_or_else(|e| e.into_inner());
    let Some(logger) = guard.as_ref() else { return };
    let _ = append(&logger.path, line);
}

/// Convenience macro — formats a message and writes it to the log.
///
/// ```rust
/// epubl_lib::log!("USB device seen: pnp={} model={}", pnp_id, model);
/// ```
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::log::write_line(&format!($($arg)*))
    };
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn append(path: &Path, line: &str) -> std::io::Result<()> {
    rotate_if_needed(path)?;

    let timestamp = timestamp_now();
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{timestamp} {line}")?;
    Ok(())
}

fn rotate_if_needed(path: &Path) -> std::io::Result<()> {
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() >= MAX_LOG_BYTES {
            let old = path.with_extension("log.old");
            // Overwrite any previous .old file.
            let _ = fs::remove_file(&old);
            fs::rename(path, &old)?;
        }
    }
    Ok(())
}

fn timestamp_now() -> String {
    // Use std::time — no chrono dependency needed for a simple timestamp.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Format as YYYY-MM-DD HH:MM:SS UTC (manual, no external crate).
    let s = secs;
    let sec = s % 60;
    let min = (s / 60) % 60;
    let hour = (s / 3600) % 24;
    let days = s / 86400; // days since 1970-01-01

    // Gregorian calendar conversion (good until ~year 2100).
    let (year, month, day) = days_to_ymd(days);

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02}Z")
}

/// Converts days since Unix epoch to (year, month, day).
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
