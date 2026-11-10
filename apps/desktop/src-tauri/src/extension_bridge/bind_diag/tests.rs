use super::*;

#[test]
fn describe_includes_port_and_os_code_only() {
    let e = io::Error::from_raw_os_error(10048);
    let s = describe(47615, &e);
    assert!(s.starts_with("47615: os error 10048"), "{s}");
}

#[test]
fn record_read_clear_roundtrip() {
    let dir = tempfile::TempDir::new().unwrap();
    record(dir.path(), &["1: a".into(), "2: b".into()]);
    let got = read(dir.path());
    assert_eq!(got.as_deref(), Some("1: a; 2: b"));
    clear(dir.path());
    assert_eq!(read(dir.path()), None);
}

#[test]
fn read_rejects_oversized_control_chars_and_foreign_text() {
    let dir = tempfile::TempDir::new().unwrap();
    let f = dir.path().join(BIND_DIAG_FILE);
    for bad in [
        "47615: os error 10048 (AddrInUse); ".repeat(100),
        "47615: os error 1 (X)\x1b[31m".to_string(),
        "47615: \u{9b}evil".to_string(),
        "hello world".to_string(),
        String::new(),
    ] {
        std::fs::write(&f, bad).unwrap();
        assert_eq!(read(dir.path()), None);
    }
    std::fs::write(&f, "47615: os error 10048 (AddrInUse); 47616: AddrInUse\n").unwrap();
    assert_eq!(
        read(dir.path()).as_deref(),
        Some("47615: os error 10048 (AddrInUse); 47616: AddrInUse")
    );
}

#[test]
fn read_rejects_a_directory_in_place_of_the_file() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join(BIND_DIAG_FILE)).unwrap();
    assert_eq!(read(dir.path()), None);
}
