//! Epub folder scanning for epubl.
//!
//! Scans a directory for `.epub` files and extracts title/author from each
//! file's OPF manifest.  Falls back to the filename stem when the OPF is
//! absent or unreadable.

use serde::Serialize;
use std::fs;
use std::io::{BufReader, Read};
use std::path::Path;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Metadata for a single epub file on disk.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EpubInfo {
    /// Bare filename, e.g. `"great-expectations.epub"`.
    pub filename: String,
    /// Title extracted from the OPF `<dc:title>` element, or the filename
    /// stem when the OPF is absent/unreadable.
    pub title: String,
    /// Author extracted from the OPF `<dc:creator>` element, or an empty
    /// string when unavailable.
    pub author: String,
    /// File size in bytes.
    pub size_bytes: u64,
}

// ---------------------------------------------------------------------------
// Core logic (testable without Tauri)
// ---------------------------------------------------------------------------

/// Scans `folder_path` for `.epub` files and returns their metadata.
///
/// Returns an error if the directory cannot be read.
pub fn scan_folder(folder_path: &str) -> Result<Vec<EpubInfo>, String> {
    let dir = Path::new(folder_path);
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Cannot read folder {folder_path:?}: {e}"))?;

    let mut books: Vec<EpubInfo> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !path.extension()?.eq_ignore_ascii_case("epub") {
                return None;
            }
            let filename = path.file_name()?.to_string_lossy().into_owned();
            let size_bytes = entry.metadata().ok()?.len();
            let (title, author) = parse_epub_metadata(&path);
            Some(EpubInfo { filename, title, author, size_bytes })
        })
        .collect();

    books.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(books)
}

/// Opens an epub ZIP and extracts `(title, author)` from the OPF manifest.
///
/// Falls back to `(filename_stem, "")` on any error.
fn parse_epub_metadata(path: &Path) -> (String, String) {
    let fallback_title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return (fallback_title, String::new()),
    };

    let mut archive = match zip::ZipArchive::new(BufReader::new(file)) {
        Ok(a) => a,
        Err(_) => return (fallback_title, String::new()),
    };

    let opf_path = match find_opf_path(&mut archive) {
        Some(p) => p,
        None => return (fallback_title, String::new()),
    };

    let mut opf_content = String::new();
    {
        let mut opf_file = match archive.by_name(&opf_path) {
            Ok(f) => f,
            Err(_) => return (fallback_title, String::new()),
        };
        if opf_file.read_to_string(&mut opf_content).is_err() {
            return (fallback_title, String::new());
        }
    }

    let (title, author) = extract_dc_fields(&opf_content);
    let title = if title.is_empty() { fallback_title } else { title };
    (title, author)
}

/// Reads `META-INF/container.xml` to find the OPF root file path.
fn find_opf_path(
    archive: &mut zip::ZipArchive<impl std::io::Read + std::io::Seek>,
) -> Option<String> {
    let mut container = String::new();
    archive
        .by_name("META-INF/container.xml")
        .ok()?
        .read_to_string(&mut container)
        .ok()?;

    let marker = "full-path=\"";
    let start = container.find(marker)? + marker.len();
    let end = container[start..].find('"')? + start;
    Some(container[start..end].to_owned())
}

/// Extracts `dc:title` and `dc:creator` values from OPF XML text.
fn extract_dc_fields(xml: &str) -> (String, String) {
    (
        extract_xml_text(xml, "dc:title"),
        extract_xml_text(xml, "dc:creator"),
    )
}

/// Returns the trimmed text content of the first `<tag>…</tag>` in `xml`.
fn extract_xml_text(xml: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");

    let tag_start = match xml.find(&open) {
        Some(i) => i,
        None => return String::new(),
    };
    let content_start = match xml[tag_start..].find('>') {
        Some(i) => tag_start + i + 1,
        None => return String::new(),
    };
    let content_end = match xml[content_start..].find(&close) {
        Some(i) => content_start + i,
        None => return String::new(),
    };
    xml[content_start..content_end].trim().to_owned()
}

// ---------------------------------------------------------------------------
// Diff logic
// ---------------------------------------------------------------------------

/// Result of comparing the local epub folder against the eReader's folder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffResult {
    /// Books present locally but not on the eReader — candidates to copy.
    pub to_copy: Vec<EpubInfo>,
    /// Books present on both local and eReader (matched by filename).
    pub up_to_date: Vec<EpubInfo>,
}

/// Compares `local_folder` against `device_folder` and returns which epub
/// files need to be copied and which are already present on the device.
///
/// Comparison is by filename only — if a file with the same name exists on
/// the device it is considered up-to-date regardless of size or content.
///
/// Returns an error if either folder cannot be read.
pub fn diff_folders(local_folder: &str, device_folder: &str) -> Result<DiffResult, String> {
    let local_books = scan_folder(local_folder)?;

    // Collect device filenames into a set for O(1) lookup.
    let device_dir = std::path::Path::new(device_folder);
    let device_entries = fs::read_dir(device_dir)
        .map_err(|e| format!("Cannot read device folder {device_folder:?}: {e}"))?;

    let device_filenames: std::collections::HashSet<String> = device_entries
        .filter_map(|e| {
            let e = e.ok()?;
            let path = e.path();
            if path.extension()?.eq_ignore_ascii_case("epub") {
                Some(path.file_name()?.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();

    let mut to_copy = Vec::new();
    let mut up_to_date = Vec::new();

    for book in local_books {
        if device_filenames.contains(&book.filename) {
            up_to_date.push(book);
        } else {
            to_copy.push(book);
        }
    }

    Ok(DiffResult { to_copy, up_to_date })
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Returns the list of epub files in the given folder.
#[tauri::command]
pub fn list_epubs(folder_path: String) -> Result<Vec<EpubInfo>, String> {
    scan_folder(&folder_path)
}

/// Compares the local epub folder against the eReader folder and returns
/// which books need to be copied and which are already present.
#[tauri::command]
pub fn diff_epubs(
    local_folder: String,
    device_folder: String,
) -> Result<DiffResult, String> {
    diff_folders(&local_folder, &device_folder)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_dc_fields_parses_title_and_creator() {
        let opf = r#"<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
            <dc:title>My Book</dc:title>
            <dc:creator>John Doe</dc:creator>
        </metadata>"#;
        let (title, author) = extract_dc_fields(opf);
        assert_eq!(title, "My Book");
        assert_eq!(author, "John Doe");
    }

    #[test]
    fn extract_dc_fields_returns_empty_when_missing() {
        let (title, author) = extract_dc_fields("<metadata/>");
        assert_eq!(title, "");
        assert_eq!(author, "");
    }

    #[test]
    fn extract_xml_text_handles_attributes_on_opening_tag() {
        let xml = r#"<dc:creator opf:role="aut">Jane Austen</dc:creator>"#;
        assert_eq!(extract_xml_text(xml, "dc:creator"), "Jane Austen");
    }

    #[test]
    fn extract_xml_text_trims_whitespace() {
        let xml = "<dc:title>  Whitespace Title  </dc:title>";
        assert_eq!(extract_xml_text(xml, "dc:title"), "Whitespace Title");
    }
}
