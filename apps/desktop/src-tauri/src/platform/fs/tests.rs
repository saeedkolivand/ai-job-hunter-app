use super::*;
use tempfile::TempDir;

fn temp_files_in(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect()
}

#[test]
fn creates_a_new_file_with_exact_bytes_and_no_temp() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("test.json");
    write_atomic(&file, b"hello world").unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"hello world");
    assert!(temp_files_in(dir.path()).is_empty());
}

#[test]
fn replaces_an_existing_file() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("test.json");
    fs::write(&file, b"old").unwrap();
    write_atomic(&file, b"new").unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"new");
    assert!(temp_files_in(dir.path()).is_empty());
}

/// The rename fails (the target is a non-empty directory): the error comes
/// back, the target is untouched and no temp file is left behind.
#[test]
fn a_failed_rename_leaves_the_target_intact_and_no_temp() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("state.json");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep"), b"x").unwrap();

    assert!(write_atomic(&target, b"new").is_err());
    assert_eq!(fs::read(target.join("keep")).unwrap(), b"x");
    assert!(temp_files_in(dir.path()).is_empty());
}

/// Overlapping writers on one file (the scheduler and an IPC update, say)
/// must all succeed and leave one complete payload. With a shared temp name
/// they collided: on Unix one writer truncated a temp that had already become
/// the live file (a torn target); on Windows about 60 of these 160 writes
/// failed outright (measured). Unique temp names: zero failures.
#[test]
fn concurrent_writers_all_succeed_and_never_leave_a_torn_file() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("state.json");
    let payloads: Vec<Vec<u8>> = (0..8u8).map(|i| vec![b'a' + i; 64 * 1024]).collect();

    let failures = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for payload in &payloads {
            let (file, failures) = (&file, &failures);
            s.spawn(move || {
                for _ in 0..20 {
                    if write_atomic(file, payload).is_err() {
                        failures.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });

    assert_eq!(
        failures.load(Ordering::Relaxed),
        0,
        "concurrent writes failed"
    );
    let content = fs::read(&file).unwrap();
    assert!(
        payloads.contains(&content),
        "file is not one complete payload ({} bytes)",
        content.len()
    );
    assert!(temp_files_in(dir.path()).is_empty());
}
