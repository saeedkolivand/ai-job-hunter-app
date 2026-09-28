//! The AUTO-flag consent gate, table-driven ACROSS `status.update`'s `auto_write_refused` and
//! `answers.save`'s `auto_save_refused`/`is_auto_answers_save` (R8 shrink pass — those two
//! test bodies differed only by the function under test). `answers_save`'s malformed-flag guard
//! has no `status.update` counterpart and stays in `answers_save/tests/auto_flag.rs`.
#![cfg(test)]

use serde_json::{json, Value};

use super::{answers_save, status_update};

/// A.4's decisive server-side gate (PR4): an AUTO write is refused ONLY while the write's own
/// opt-in is off; a deliberate (manual/popup) write is never refused here, opt-in or not.
#[test]
fn auto_write_is_refused_only_when_flagged_auto_and_optin_off() {
    struct Case {
        refused: fn(&Value, bool) -> bool,
        auto: Value,
        manual: Value,
        manual_message: &'static str,
    }
    let cases = [
        Case {
            refused: status_update::auto_write_refused,
            auto: json!({ "url": "https://x.co/j", "to": "applied", "auto": true }),
            manual: json!({ "url": "https://x.co/j", "to": "applied" }),
            manual_message: "manual click stays ungated even with the opt-in OFF",
        },
        Case {
            refused: answers_save::auto_save_refused,
            auto: json!({ "url": "https://x.co/j", "answers": [], "auto": true }),
            manual: json!({ "url": "https://x.co/j", "answers": [] }),
            manual_message:
                "a manual (popup) save stays ungated by this flag even with the opt-in OFF",
        },
    ];
    for case in cases {
        assert!(
            (case.refused)(&case.auto, false),
            "auto + opt-in OFF → refuse"
        );
        assert!(
            !(case.refused)(&case.auto, true),
            "auto + opt-in ON → allowed"
        );
        assert!(
            !(case.refused)(&case.manual, false),
            "{}",
            case.manual_message
        );
        assert!(!(case.refused)(&case.manual, true));
    }
}

/// The `auto` flag defaults to `false` (a manual click/save) whenever it is absent from the
/// payload — never `unwrap_or`-panics, never treats an absent flag as auto.
#[test]
fn is_auto_flag_defaults_false_when_absent() {
    struct Case {
        is_auto: fn(&Value) -> bool,
        absent_message: &'static str,
    }
    let cases = [
        Case {
            is_auto: status_update::is_auto_status_update,
            absent_message: "absent `auto` → treated as a manual click",
        },
        Case {
            is_auto: answers_save::is_auto_answers_save,
            absent_message: "absent `auto` → treated as a manual (popup) save",
        },
    ];
    for case in cases {
        assert!((case.is_auto)(&json!({ "auto": true })));
        assert!(!(case.is_auto)(&json!({ "auto": false })));
        assert!(
            !(case.is_auto)(&json!({ "url": "x" })),
            "{}",
            case.absent_message
        );
    }
}
