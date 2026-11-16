use std::path::Path;

use super::*;
use crate::error::AppError;

/// The ONLY test that touches the process-wide failure list (it drains it at the end),
/// and none may set `IPC_LOCKED`: it never resets and would poison the lib test binary.
#[test]
fn a_failed_open_is_recorded_with_its_label_and_code_and_a_good_one_is_not() {
    assert_eq!(take_failures(), None, "nothing recorded yet");

    let ok = open_or_record("documents", "document store", Ok(7));
    assert_eq!(ok, Some(7));
    assert_eq!(take_failures(), None, "a successful open records nothing");

    let failed: Option<u8> = open_or_record(
        "ai_provider_config",
        "ai provider config store",
        Err(AppError::Storage("boom".into())),
    );
    assert_eq!(failed, None);
    let _: Option<u8> = open_or_record("spend", "spend store", Err(AppError::Config("x".into())));

    let expected = vec![
        (
            "ai_provider_config",
            AppError::Storage(String::new()).code(),
        ),
        ("spend", AppError::Config(String::new()).code()),
    ];
    assert_eq!(take_failures(), Some(expected));
    assert_eq!(take_failures(), None, "taking drains the list");
}

#[test]
fn dialog_text_names_stores_codes_and_folder_but_not_an_error_message() {
    let text = dialog_text(
        &[("documents", "STORAGE"), ("spend", "STORAGE")],
        Path::new("data-dir-for-test"),
    );
    assert!(text.contains("documents (STORAGE)"));
    assert!(text.contains("spend (STORAGE)"));
    assert!(text.contains("data-dir-for-test"));
    assert!(!text.contains("reinstall"));
}

/// `abort_if_failed` and the handler gate need a live `App`/`Invoke`, which a unit test
/// cannot build, so these pin that the wiring is still present in the source.
#[test]
fn the_abort_and_the_gate_are_still_wired_in() {
    assert!(include_str!("../setup.rs").contains("store_open::abort_if_failed(app, &data_dir)"));
    assert!(include_str!("../handler.rs").contains("ipc_gate::gate("));
    assert!(include_str!("../handler.rs").contains("store_open::ipc_locked(),"));
    assert!(include_str!("../state.rs").contains("store_open::open_or_record("));
    assert!(include_str!("../../tray/mod.rs").contains("store_open_failed()"));
}
