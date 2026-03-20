use epubl_lib::copy::{copy_files, CopyEvent};
use std::fs;
use std::sync::Mutex;
use tempfile::tempdir;

fn write_file(dir: &std::path::Path, name: &str, size: usize) {
    fs::write(dir.join(name), vec![0u8; size]).unwrap();
}

/// Collects CopyEvents emitted during a copy_files call.
struct EventCollector {
    events: Mutex<Vec<CopyEvent>>,
}

impl EventCollector {
    fn new() -> Self {
        Self { events: Mutex::new(Vec::new()) }
    }

    fn push(&self, e: CopyEvent) {
        self.events.lock().unwrap().push(e);
    }

    fn all(&self) -> Vec<CopyEvent> {
        self.events.lock().unwrap().clone()
    }
}

// ---------------------------------------------------------------------------
// copy_files tests
// ---------------------------------------------------------------------------

#[test]
fn copy_single_file_to_device() {
    let local = tempdir().unwrap();
    let device = tempdir().unwrap();
    write_file(local.path(), "book.epub", 1024);

    let collector = EventCollector::new();
    copy_files(
        &["book.epub".to_string()],
        local.path().to_str().unwrap(),
        device.path().to_str().unwrap(),
        |e| collector.push(e),
    )
    .unwrap();

    assert!(device.path().join("book.epub").exists());
}

#[test]
fn copy_multiple_files_to_device() {
    let local = tempdir().unwrap();
    let device = tempdir().unwrap();
    write_file(local.path(), "a.epub", 512);
    write_file(local.path(), "b.epub", 1024);
    write_file(local.path(), "c.epub", 2048);

    let collector = EventCollector::new();
    copy_files(
        &[
            "a.epub".to_string(),
            "b.epub".to_string(),
            "c.epub".to_string(),
        ],
        local.path().to_str().unwrap(),
        device.path().to_str().unwrap(),
        |e| collector.push(e),
    )
    .unwrap();

    assert!(device.path().join("a.epub").exists());
    assert!(device.path().join("b.epub").exists());
    assert!(device.path().join("c.epub").exists());
}

#[test]
fn progress_events_emitted_once_per_file() {
    let local = tempdir().unwrap();
    let device = tempdir().unwrap();
    write_file(local.path(), "a.epub", 256);
    write_file(local.path(), "b.epub", 256);

    let collector = EventCollector::new();
    copy_files(
        &["a.epub".to_string(), "b.epub".to_string()],
        local.path().to_str().unwrap(),
        device.path().to_str().unwrap(),
        |e| collector.push(e),
    )
    .unwrap();

    let events = collector.all();
    assert_eq!(events.len(), 2);
}

#[test]
fn progress_events_have_correct_files_total() {
    let local = tempdir().unwrap();
    let device = tempdir().unwrap();
    write_file(local.path(), "a.epub", 256);
    write_file(local.path(), "b.epub", 256);

    let collector = EventCollector::new();
    copy_files(
        &["a.epub".to_string(), "b.epub".to_string()],
        local.path().to_str().unwrap(),
        device.path().to_str().unwrap(),
        |e| collector.push(e),
    )
    .unwrap();

    let events = collector.all();
    for e in &events {
        assert_eq!(e.files_total, 2);
    }
}

#[test]
fn progress_events_files_done_increments() {
    let local = tempdir().unwrap();
    let device = tempdir().unwrap();
    write_file(local.path(), "a.epub", 256);
    write_file(local.path(), "b.epub", 256);
    write_file(local.path(), "c.epub", 256);

    let collector = EventCollector::new();
    copy_files(
        &[
            "a.epub".to_string(),
            "b.epub".to_string(),
            "c.epub".to_string(),
        ],
        local.path().to_str().unwrap(),
        device.path().to_str().unwrap(),
        |e| collector.push(e),
    )
    .unwrap();

    let events = collector.all();
    let done_counts: Vec<u32> = events.iter().map(|e| e.files_done).collect();
    assert_eq!(done_counts, vec![1, 2, 3]);
}

#[test]
fn progress_event_carries_current_filename() {
    let local = tempdir().unwrap();
    let device = tempdir().unwrap();
    write_file(local.path(), "my-book.epub", 256);

    let collector = EventCollector::new();
    copy_files(
        &["my-book.epub".to_string()],
        local.path().to_str().unwrap(),
        device.path().to_str().unwrap(),
        |e| collector.push(e),
    )
    .unwrap();

    let events = collector.all();
    assert_eq!(events[0].filename, "my-book.epub");
}

#[test]
fn progress_event_bytes_total_sums_all_files() {
    let local = tempdir().unwrap();
    let device = tempdir().unwrap();
    write_file(local.path(), "a.epub", 100);
    write_file(local.path(), "b.epub", 200);

    let collector = EventCollector::new();
    copy_files(
        &["a.epub".to_string(), "b.epub".to_string()],
        local.path().to_str().unwrap(),
        device.path().to_str().unwrap(),
        |e| collector.push(e),
    )
    .unwrap();

    let events = collector.all();
    for e in &events {
        assert_eq!(e.bytes_total, 300);
    }
}

#[test]
fn progress_event_bytes_copied_accumulates() {
    let local = tempdir().unwrap();
    let device = tempdir().unwrap();
    write_file(local.path(), "a.epub", 100);
    write_file(local.path(), "b.epub", 200);

    let collector = EventCollector::new();
    copy_files(
        &["a.epub".to_string(), "b.epub".to_string()],
        local.path().to_str().unwrap(),
        device.path().to_str().unwrap(),
        |e| collector.push(e),
    )
    .unwrap();

    let events = collector.all();
    assert_eq!(events[0].bytes_copied, 100);
    assert_eq!(events[1].bytes_copied, 300);
}

#[test]
fn error_on_missing_source_file() {
    let local = tempdir().unwrap();
    let device = tempdir().unwrap();

    let collector = EventCollector::new();
    let result = copy_files(
        &["nonexistent.epub".to_string()],
        local.path().to_str().unwrap(),
        device.path().to_str().unwrap(),
        |e| collector.push(e),
    );

    assert!(result.is_err());
}

#[test]
fn error_on_missing_device_folder() {
    let local = tempdir().unwrap();
    write_file(local.path(), "book.epub", 256);

    let collector = EventCollector::new();
    let result = copy_files(
        &["book.epub".to_string()],
        local.path().to_str().unwrap(),
        "/no/such/device/folder",
        |e| collector.push(e),
    );

    assert!(result.is_err());
}

#[test]
fn empty_filenames_list_is_a_no_op() {
    let local = tempdir().unwrap();
    let device = tempdir().unwrap();

    let collector = EventCollector::new();
    copy_files(
        &[],
        local.path().to_str().unwrap(),
        device.path().to_str().unwrap(),
        |e| collector.push(e),
    )
    .unwrap();

    assert_eq!(collector.all().len(), 0);
}
