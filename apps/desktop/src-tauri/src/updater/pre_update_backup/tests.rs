use super::*;
use serde_json::json;

fn backup_names(data_dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(data_dir.join("backups").join("pre-update"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn writes_the_bundle_as_compact_json() {
    let tmp = tempfile::tempdir().unwrap();
    let bundle = json!({ "version": 1, "stores": { "a": [1, 2] } });
    let path = write_pre_update_backup(tmp.path(), "0.156.0", "2026-09-24", &bundle).unwrap();

    assert_eq!(
        path.file_name().unwrap().to_string_lossy(),
        "pre-update-0.156.0-2026-09-24.json"
    );
    let written = fs::read_to_string(&path).unwrap();
    assert!(!written.contains('\n'), "compact, not pretty-printed");
    assert_eq!(serde_json::from_str::<Value>(&written).unwrap(), bundle);
}

#[test]
fn keeps_only_the_newest_three_and_never_touches_the_users_files() {
    let tmp = tempfile::tempdir().unwrap();
    let user_dir = tmp.path().join("backups");
    fs::create_dir_all(&user_dir).unwrap();
    // Named like a backup on purpose: it still isn't the updater's to prune.
    fs::write(user_dir.join("pre-update-notes.json"), b"{}").unwrap();

    for (i, version) in ["0.150.0", "0.151.0", "0.152.0", "0.153.0"]
        .iter()
        .enumerate()
    {
        write_pre_update_backup(tmp.path(), version, &format!("2026-09-0{i}"), &json!({})).unwrap();
        // Distinct modified times, oldest first.
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert_eq!(
        backup_names(tmp.path()),
        vec![
            "pre-update-0.151.0-2026-09-01.json",
            "pre-update-0.152.0-2026-09-02.json",
            "pre-update-0.153.0-2026-09-03.json",
        ]
    );
    assert!(user_dir.join("pre-update-notes.json").exists());
}

#[test]
fn a_version_cannot_steer_the_file_name() {
    let tmp = tempfile::tempdir().unwrap();
    let path = write_pre_update_backup(tmp.path(), "../../evil", "2026-09-24", &json!({})).unwrap();
    assert_eq!(
        path.parent().unwrap(),
        tmp.path().join("backups").join("pre-update")
    );
    assert_eq!(
        path.file_name().unwrap().to_string_lossy(),
        "pre-update-....evil-2026-09-24.json"
    );
}

#[test]
fn fails_when_the_backups_path_is_not_a_directory() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("backups"), b"a file, not a dir").unwrap();
    assert!(write_pre_update_backup(tmp.path(), "0.1.0", "2026-09-24", &json!({})).is_err());
}
