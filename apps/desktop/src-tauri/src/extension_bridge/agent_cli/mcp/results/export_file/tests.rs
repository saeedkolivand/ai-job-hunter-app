use super::*;

fn export_payload(filename: &str) -> Value {
    json!({
        "dispatched": true, "namespace": "documents", "command": EXPORT_COMMAND,
        "data": {
            "data": "JVBERi0xLjQ=", "dataEncoding": "base64",
            "mimeType": "application/pdf", "filename": filename,
        },
    })
}

#[test]
fn export_becomes_path_and_bytes_with_decoded_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut p = export_payload("cv.pdf");
    write_export(dir.path(), &mut p);
    let d = &p["data"];
    assert!(d.get("data").is_none() && d.get("dataEncoding").is_none());
    assert_eq!(d["bytes"], 8);
    assert_eq!(d["filename"], "cv.pdf");
    assert_eq!(d["mimeType"], "application/pdf");
    let path = Path::new(d["path"].as_str().unwrap());
    assert!(path.starts_with(dir.path()));
    assert_eq!(std::fs::read(path).unwrap(), b"%PDF-1.4");
}

#[test]
fn traversal_filename_stays_inside_dir() {
    let dir = tempfile::TempDir::new().unwrap();
    for evil in [
        "../../evil.pdf",
        "..\\..\\evil.pdf",
        "/etc/passwd",
        "..",
        "",
    ] {
        let mut p = export_payload(evil);
        write_export(dir.path(), &mut p);
        let path = PathBuf::from(p["data"]["path"].as_str().unwrap());
        assert_eq!(path.parent().unwrap(), dir.path(), "{evil}");
        assert!(path.exists());
    }
    assert!(!dir.path().parent().unwrap().join("evil.pdf").exists());
}

#[test]
fn same_filename_twice_never_clobbers() {
    let dir = tempfile::TempDir::new().unwrap();
    let (mut a, mut b) = (export_payload("cv.pdf"), export_payload("cv.pdf"));
    write_export(dir.path(), &mut a);
    write_export(dir.path(), &mut b);
    assert_ne!(a["data"]["path"], b["data"]["path"]);
}

#[test]
fn non_export_and_bad_payloads_are_untouched() {
    let dir = tempfile::TempDir::new().unwrap();
    for original in [
        json!({"dispatched": true, "data": {"data": [1, 2], "x": 1}}),
        json!({"dispatched": false, "error": "rate_limited"}),
        json!({"data": {"data": "!!notb64!!", "dataEncoding": "base64"}}),
    ] {
        let mut p = original.clone();
        write_export(dir.path(), &mut p);
        assert_eq!(p, original);
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn unwritable_dir_falls_back_to_inline_shape() {
    let dir = tempfile::TempDir::new().unwrap();
    let blocker = dir.path().join("file");
    std::fs::write(&blocker, b"x").unwrap();
    let mut p = export_payload("cv.pdf");
    let original = p.clone();
    write_export(&blocker.join("sub"), &mut p);
    assert_eq!(p, original);
}

/// A shared-`/tmp` dir another user could read (or pre-created) is refused, and a file we write
/// is owner-only.
#[cfg(unix)]
#[test]
fn group_or_world_accessible_dir_is_refused_and_files_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::TempDir::new().unwrap();
    let shared = dir.path().join("shared");
    std::fs::create_dir(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o777)).unwrap();
    let mut p = export_payload("cv.pdf");
    let original = p.clone();
    write_export(&shared, &mut p);
    assert_eq!(p, original);

    let private = dir.path().join("private");
    write_export(&private, &mut p);
    let path = p["data"]["path"].as_str().expect("written");
    let mode = std::fs::metadata(path).unwrap().permissions().mode();
    assert_eq!(mode & 0o077, 0, "export file must be owner-only");
}

#[test]
fn verb_other_than_export_is_skipped() {
    let verb = Verb::Call {
        namespace: "documents".into(),
        command: "documents_list".into(),
        input: json!({}),
        confirm: None,
    };
    let mut p = export_payload("cv.pdf");
    let original = p.clone();
    export_to_file(&verb, &mut p);
    assert_eq!(p, original);
}
