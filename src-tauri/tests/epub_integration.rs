use epubl_lib::epub::scan_folder;
use std::fs;
use tempfile::tempdir;

fn write_minimal_epub(dir: &std::path::Path, filename: &str, title: &str, author: &str) {
    use std::io::Write;

    let path = dir.join(filename);
    let file = fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);

    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    // Required by epub spec
    zip.start_file("mimetype", opts).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();

    zip.start_file("META-INF/container.xml", opts).unwrap();
    zip.write_all(
        br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
    )
    .unwrap();

    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>{title}</dc:title>
    <dc:creator>{author}</dc:creator>
    <dc:identifier id="uid">test-id</dc:identifier>
    <dc:language>en</dc:language>
  </metadata>
  <manifest/>
  <spine/>
</package>"#
    );
    zip.start_file("OEBPS/content.opf", opts).unwrap();
    zip.write_all(opf.as_bytes()).unwrap();

    zip.finish().unwrap();
}

#[test]
fn scan_returns_epub_from_folder() {
    let dir = tempdir().unwrap();
    write_minimal_epub(dir.path(), "book.epub", "My Title", "Jane Author");
    let results = scan_folder(dir.path().to_str().unwrap()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].filename, "book.epub");
}

#[test]
fn scan_extracts_title_and_author_from_opf() {
    let dir = tempdir().unwrap();
    write_minimal_epub(dir.path(), "book.epub", "Great Expectations", "Charles Dickens");
    let results = scan_folder(dir.path().to_str().unwrap()).unwrap();
    assert_eq!(results[0].title, "Great Expectations");
    assert_eq!(results[0].author, "Charles Dickens");
}

#[test]
fn scan_ignores_non_epub_files() {
    let dir = tempdir().unwrap();
    write_minimal_epub(dir.path(), "book.epub", "A Book", "An Author");
    fs::write(dir.path().join("readme.txt"), b"not an epub").unwrap();
    fs::write(dir.path().join("cover.jpg"), b"not an epub").unwrap();
    let results = scan_folder(dir.path().to_str().unwrap()).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn scan_returns_multiple_epubs() {
    let dir = tempdir().unwrap();
    write_minimal_epub(dir.path(), "a.epub", "Book A", "Author A");
    write_minimal_epub(dir.path(), "b.epub", "Book B", "Author B");
    write_minimal_epub(dir.path(), "c.epub", "Book C", "Author C");
    let results = scan_folder(dir.path().to_str().unwrap()).unwrap();
    assert_eq!(results.len(), 3);
}

#[test]
fn scan_returns_empty_for_empty_folder() {
    let dir = tempdir().unwrap();
    let results = scan_folder(dir.path().to_str().unwrap()).unwrap();
    assert!(results.is_empty());
}

#[test]
fn scan_returns_error_for_missing_folder() {
    let result = scan_folder("/nonexistent/path/that/does/not/exist");
    assert!(result.is_err());
}

#[test]
fn scan_falls_back_to_filename_when_opf_missing() {
    let dir = tempdir().unwrap();
    // A zip file with .epub extension but no OPF
    use std::io::Write;
    let path = dir.path().join("mystery.epub");
    let file = fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    zip.start_file("mimetype", opts).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();
    zip.finish().unwrap();

    let results = scan_folder(dir.path().to_str().unwrap()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].filename, "mystery.epub");
    // Falls back gracefully
    assert!(!results[0].title.is_empty());
}

#[test]
fn epub_info_includes_file_size() {
    let dir = tempdir().unwrap();
    write_minimal_epub(dir.path(), "book.epub", "A Title", "An Author");
    let results = scan_folder(dir.path().to_str().unwrap()).unwrap();
    assert!(results[0].size_bytes > 0);
}
