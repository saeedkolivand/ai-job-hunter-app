use std::cell::RefCell;
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

#[test]
fn failures_abort_and_never_start_services() {
    let log = RefCell::new(Vec::new());
    let result: Result<(), ()> = finish_setup(
        Some(vec![("documents", "STORAGE")]),
        |failures| log.borrow_mut().push(format!("abort {}", failures.len())),
        || {
            log.borrow_mut().push("services".into());
            Ok(())
        },
    );
    assert_eq!(result, Ok(()));
    assert_eq!(*log.borrow(), ["abort 1"]);
}

#[test]
fn no_failures_start_services_and_never_abort() {
    let log = RefCell::new(Vec::new());
    let result: Result<(), &str> = finish_setup(
        None,
        |_| log.borrow_mut().push("abort".to_string()),
        || {
            log.borrow_mut().push("services".into());
            Err("a setup error still propagates")
        },
    );
    assert_eq!(result, Err("a setup error still propagates"));
    assert_eq!(*log.borrow(), ["services"]);
}

#[test]
fn the_abort_runs_lock_then_report_then_hide_then_dialog_once_each() {
    let log = RefCell::new(Vec::new());
    abort_sequence(
        &[("documents", "STORAGE")],
        Path::new("data-dir-for-test"),
        || log.borrow_mut().push("lock".to_string()),
        |failures| log.borrow_mut().push(format!("report {}", failures.len())),
        || log.borrow_mut().push("hide".into()),
        |text| {
            assert!(text.contains("documents (STORAGE)"));
            log.borrow_mut().push("dialog".into());
        },
    );
    assert_eq!(*log.borrow(), ["lock", "report 1", "hide", "dialog"]);
}

/// Source text with comment lines and all whitespace removed (LF/CRLF-safe).
fn squashed(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .flat_map(|line| line.split_whitespace())
        .collect()
}

/// Pins two wiring points into Tauri types that a unit test cannot construct (no
/// `Invoke`, no `App`): the handler feeding the lock flag into the gate, and
/// `show_focus` returning early while locked. The branch logic behind both is covered
/// behaviourally by the `ipc_gate` and `abort_sequence` tests; this only guards that
/// the real call sites still use it.
#[test]
fn the_handler_and_show_focus_are_wired_to_the_lock() {
    assert!(squashed(include_str!("../handler.rs"))
        .contains("super::ipc_gate::gate(super::store_open::ipc_locked(),invoke,"));
    assert!(squashed(include_str!("../../tray/mod.rs"))
        .contains("pubfnshow_focus(app:&AppHandle){ifcrate::shell::store_open_failed(){return;}"));
}
