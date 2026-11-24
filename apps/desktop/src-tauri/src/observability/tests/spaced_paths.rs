//! Home paths with a space in the user segment redact whole (#1530).

use super::super::redact_tokens;

#[test]
fn spaced_user_segment_leaves_no_surname() {
    for path in [
        r"C:\Users\Jane Doe\AppData\Local\app.exe",
        "/home/jane doe/.config/app",
        "/Users/jane doe/Library/app",
    ] {
        let out = redact_tokens(&format!("failed to open {path} now"));
        assert!(!out.to_lowercase().contains("doe"), "leaked: {out}");
        assert_eq!(out, "failed to open <path-redacted> now");
    }
}

#[test]
fn long_nbsp_and_unterminated_user_segments_leave_no_surname() {
    for text in [
        r"failed to open C:\Users\Ana Maria de Souza\AppData\app.exe now",
        "failed to open /home/jane\u{a0}doe/.config/app now",
        r#"failed to open "C:\Users\Jane Doe" now"#,
        r"failed to open C:\Users\Jane Doe",
    ] {
        let out = redact_tokens(text).to_lowercase();
        for name in ["doe", "souza", "maria"] {
            assert!(!out.contains(name), "leaked `{name}`: {out}");
        }
    }
}

#[test]
fn ordinary_prose_with_slashes_is_untouched() {
    let msg = "users and groups lookup failed, try again";
    assert_eq!(redact_tokens(msg), msg);
}
