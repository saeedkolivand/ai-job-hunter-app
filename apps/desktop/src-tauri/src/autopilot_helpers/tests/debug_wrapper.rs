//! `redact_token` / `sanitize_reason` looking through a Rust `Debug` wrapper (`Path("…")`): a path
//! printed with `{:?}` must be redacted exactly like a bare one.

use crate::observability::{redact_token, sanitize_reason};

#[test]
fn a_path_inside_a_debug_wrapper_is_redacted_like_a_bare_one() {
    // `minidumper-child` fails startup with "Failed to create server with
    // socket name {socket_name:?}", so the path arrives as `Path("…")` — the
    // wrapper defeats the `starts_with('/')` / drive-letter checks unless the
    // redactor looks through it. Windows `Debug` doubles the backslashes.
    for (raw, leaked, expected) in [
        (
            r#"Path("/var/folders/ab/xyz123/T/temp-socket-0f")"#,
            "xyz123",
            r#"Path("<path-redacted>")"#,
        ),
        (
            r#"Path("D:\\Temp\\alice\\temp-socket-0f")"#,
            "alice",
            r#"Path("<path-redacted>")"#,
        ),
        (
            r#"Name("/tmp/alice/temp-socket-0f")"#,
            "alice",
            r#"Name("<path-redacted>")"#,
        ),
    ] {
        let out = redact_token(raw);
        assert!(!out.contains(leaked), "path leaked from {raw:?}: {out}");
        assert_eq!(out, expected, "wrapper shape must survive for {raw:?}");
    }

    // Controls: the wrapper alone is not a signal. Non-path payloads, and a
    // lone `/` or a fraction inside one, stay exactly as they were.
    for benign in [
        r#"Some("hello")"#,
        r#"Abstract("ajh-0f3a")"#,
        r#"Name("1/2")"#,
        r#"Name("/")"#,
    ] {
        assert_eq!(redact_token(benign), benign, "benign token altered");
    }
}

#[test]
fn a_debug_wrapped_socket_name_in_a_sentence_is_redacted() {
    for socket in [
        r#"Path("/var/folders/ab/xyz123/T/temp-socket-0f")"#,
        r#"Path("D:\\Temp\\alice\\temp-socket-0f")"#,
    ] {
        let out = sanitize_reason(&format!(
            "Failed to create server with socket name {socket}"
        ));
        assert_eq!(
            out,
            r#"Failed to create server with socket name Path("<path-redacted>")"#
        );
    }
}
