use super::*;

#[test]
fn describe_includes_port_and_os_code_only() {
    let e = io::Error::from_raw_os_error(10048);
    let s = describe(47615, &e);
    assert!(s.starts_with("47615: os error 10048"), "{s}");
}

#[test]
fn record_read_clear_roundtrip_and_cli_message() {
    let dir = tempfile::TempDir::new().unwrap();
    record(dir.path(), &["1: a".into(), "2: b".into()]);
    let got = read(dir.path());
    assert_eq!(got.as_deref(), Some("1: a; 2: b"));
    let msg = cli_message("1: a; 2: b");
    assert!(
        msg.contains("47615-47620") && msg.contains("1: a; 2: b"),
        "{msg}"
    );
    clear(dir.path());
    assert_eq!(read(dir.path()), None);
}
