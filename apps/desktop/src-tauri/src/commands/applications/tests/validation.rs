use serde_json::{json, Value};

use super::super::validation::*;
use crate::applications::MAX_JOB_DESCRIPTION_BYTES;
use crate::error::AppError;

#[test]
fn parse_next_action_at_maps_absent_null_and_valid_numbers() {
    // Absent → leave the stored reminder untouched.
    assert_eq!(parse_next_action_at(None).unwrap(), None);
    // Explicit null → clear it.
    assert_eq!(parse_next_action_at(Some(Value::Null)).unwrap(), Some(None));
    // A real timestamp → set it. Zero is a legitimate epoch value.
    assert_eq!(
        parse_next_action_at(Some(json!(1_767_225_600_000u64))).unwrap(),
        Some(Some(1_767_225_600_000))
    );
    assert_eq!(parse_next_action_at(Some(json!(0))).unwrap(), Some(Some(0)));
}

#[test]
fn parse_next_action_at_rejects_a_non_u64_instead_of_clearing() {
    // `Value::as_u64()` returns None for every one of these, which used to be
    // indistinguishable from an explicit null — so a caller trying to SET a
    // bad-typed reminder silently CLEARED it. They must be errors, not clears.
    for bad in [
        json!(-1),
        json!(1.5),
        json!(u64::MAX as f64 * 2.0),
        json!("1767225600000"),
        json!({}),
        json!([]),
    ] {
        let err = parse_next_action_at(Some(bad.clone()))
            .expect_err(&format!("{bad} must be rejected, not treated as a clear"));
        assert!(
            matches!(err, AppError::Validation(_)),
            "{bad} must fail validation, got {err:?}"
        );
    }
}

#[test]
fn none_description_is_accepted() {
    // No description supplied → always fine (the common path).
    assert!(reject_oversized_job_description(None).is_ok());
}

#[test]
fn under_and_at_cap_are_accepted() {
    // Empty, small, and exactly-at-the-cap inputs all pass — the guard rejects
    // ONLY strictly-oversized input, mirroring the store's `len() <= cap` clamp.
    let at_cap = "a".repeat(MAX_JOB_DESCRIPTION_BYTES);
    for jd in ["", "a normal job ad", at_cap.as_str()] {
        assert!(
            reject_oversized_job_description(Some(jd)).is_ok(),
            "{} bytes must be accepted (cap is {MAX_JOB_DESCRIPTION_BYTES})",
            jd.len()
        );
    }
}

#[test]
fn over_cap_is_rejected() {
    // One byte over the cap → rejected up-front (the direct-IPC abuse path the
    // renderer's Zod cap can't protect). The store still clamps as a second layer.
    let oversized = "a".repeat(MAX_JOB_DESCRIPTION_BYTES + 1);
    let err = reject_oversized_job_description(Some(&oversized))
        .expect_err("an over-cap description must be rejected");
    // It is a typed Validation error (R6), and the message names the byte limit
    // so the renderer can surface a useful reason.
    assert!(
        matches!(err, AppError::Validation(_)),
        "must be a Validation error, got {err:?}"
    );
    assert!(
        err.to_string()
            .contains(&MAX_JOB_DESCRIPTION_BYTES.to_string()),
        "rejection message must mention the byte cap, got {err:?}"
    );
}

#[test]
fn multi_byte_utf8_over_cap_is_rejected() {
    // '€' is 3 UTF-8 bytes. 66_667 repetitions → 200_001 bytes, one over the
    // 200_000-byte cap. A future switch from str.len() (bytes) to char-count
    // would keep 66_667 chars in and this test would catch the regression.
    let s = "€".repeat(66_667);
    assert_eq!(
        s.len(),
        200_001,
        "fixture sanity: expected 200_001 bytes, got {}",
        s.len()
    );
    let err = reject_oversized_job_description(Some(&s))
        .expect_err("multi-byte string over the byte cap must be rejected");
    assert!(
        matches!(err, AppError::Validation(_)),
        "must be a Validation error, got {err:?}"
    );
}

#[test]
fn multi_byte_utf8_at_cap_is_accepted() {
    // '€' is 3 UTF-8 bytes. 66_666 repetitions → 199_998 bytes, within the
    // 200_000-byte cap. Verifies the guard accepts multi-byte content that fits.
    let s = "€".repeat(66_666);
    assert_eq!(
        s.len(),
        199_998,
        "fixture sanity: expected 199_998 bytes, got {}",
        s.len()
    );
    assert!(
        reject_oversized_job_description(Some(&s)).is_ok(),
        "{} bytes must be accepted (cap is {MAX_JOB_DESCRIPTION_BYTES})",
        s.len()
    );
}

// ── recipient_email validation ────────────────────────────────────────────

#[test]
fn recipient_email_absent_passes_through_and_whitespace_only_clears() {
    // None = field not supplied → no update, no error.
    assert!(validate_recipient_email(None).unwrap().is_none());
    // Whitespace-only → treat as "clear the field" (not an error).
    let result = validate_recipient_email(Some("   ".into())).unwrap();
    assert_eq!(result, Some(String::new()));
}

#[test]
fn recipient_email_valid_addresses_accepted() {
    for addr in [
        "user@example.com",
        "first.last@sub.domain.org",
        "user+tag@example.co.uk",
    ] {
        let result = validate_recipient_email(Some(addr.into()));
        assert!(
            result.is_ok(),
            "valid address {addr:?} must be accepted, got {result:?}"
        );
        assert_eq!(result.unwrap(), Some(addr.to_string()));
    }
}

#[test]
fn recipient_email_malformed_addresses_are_rejected() {
    for (bad, why) in [
        ("notanemail", "missing @ must be rejected"),
        ("a@b@c.com", "multiple @ must be rejected"),
        ("@example.com", "empty local part must be rejected"),
        ("user@nodot", "domain without dot must be rejected"),
        // TLD would be empty: "user@example."
        ("user@example.", "trailing dot (empty TLD) must be rejected"),
    ] {
        let err = validate_recipient_email(Some(bad.into())).expect_err(why);
        assert!(matches!(err, AppError::Validation(_)));
    }
}

#[test]
fn recipient_email_control_characters_and_spaces_fail_validation() {
    // A stored CR/LF is a header-injection primitive for every sink built
    // from this address (the mailto: href today). Reject, never store.
    for bad in [
        "user@example.com\r\nBcc: attacker@evil.test",
        "user@example.com\nBcc: attacker@evil.test",
        "us\ter@example.com",
        "user\u{0000}@example.com",
        "user\u{000B}@example.com",
        // Interior space: never legal in the unquoted local part this
        // validator accepts, and a separator in most address parsers.
        "user name@example.com",
        "user@exa mple.com",
    ] {
        let err = validate_recipient_email(Some(bad.into()))
            .expect_err(&format!("{bad:?} must be rejected"));
        assert!(
            matches!(err, AppError::Validation(_)),
            "{bad:?} must fail validation, got {err:?}"
        );
    }
}

#[test]
fn recipient_email_surrounding_whitespace_is_trimmed_not_rejected() {
    // The control-char guard runs AFTER the trim, so a leading/trailing
    // newline is still just whitespace to strip — not a rejection.
    for padded in ["  user@example.com  ", "\r\n  user@example.com \t"] {
        let ok = validate_recipient_email(Some(padded.into()))
            .expect("surrounding whitespace is trimmed, not rejected")
            .unwrap();
        assert_eq!(ok, "user@example.com");
    }
}

// ── contact name validation ───────────────────────────────────────────────

#[test]
fn contact_name_absent_is_passthrough_and_whitespace_clears() {
    assert!(validate_contact_name(None).unwrap().is_none());
    assert_eq!(
        validate_contact_name(Some("   ".into())).unwrap(),
        Some(String::new())
    );
}

#[test]
fn contact_name_is_trimmed_and_bounded() {
    assert_eq!(
        validate_contact_name(Some("  Rita Recruiter  ".into())).unwrap(),
        Some("Rita Recruiter".to_string())
    );
    // Exactly at the cap is accepted; one byte over is rejected (never
    // silently truncated).
    let at_cap = "a".repeat(MAX_CONTACT_NAME_BYTES);
    assert_eq!(
        validate_contact_name(Some(at_cap.clone())).unwrap(),
        Some(at_cap)
    );
    let err = validate_contact_name(Some("a".repeat(MAX_CONTACT_NAME_BYTES + 1)))
        .expect_err("an over-cap name must be rejected");
    assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
}

#[test]
fn contact_name_control_characters_are_rejected_but_spaces_are_kept() {
    // The name is interpolated verbatim into the migration's
    // "Apply-by-email: <name> <<email>>" note and into the display-name half
    // of any future message header, so a bare CR/LF is the same injection
    // primitive the address guard rejects.
    for bad in [
        "Rita\r\nBcc: attacker@evil.test",
        "Rita\nRecruiter",
        "Rita\tRecruiter",
        "Rita\u{0000}",
    ] {
        let err = validate_contact_name(Some(bad.into()))
            .expect_err(&format!("{bad:?} must be rejected"));
        assert!(
            matches!(err, AppError::Validation(_)),
            "{bad:?} must fail validation, got {err:?}"
        );
    }
    // Interior SPACES are legal in a name (unlike in an address) — real
    // names have them, so the guard must not borrow the email rule wholesale.
    assert_eq!(
        validate_contact_name(Some("Rita von der Recruiter".into())).unwrap(),
        Some("Rita von der Recruiter".to_string())
    );
}

#[test]
fn contact_name_cap_counts_bytes_not_chars() {
    // 'ü' is 2 UTF-8 bytes: 101 chars = 202 bytes > the 200-byte cap. A
    // char-count cap would let this through and the store would keep a
    // longer value than the contract promises.
    let s = "ü".repeat(101);
    assert!(s.chars().count() < MAX_CONTACT_NAME_BYTES);
    assert!(s.len() > MAX_CONTACT_NAME_BYTES);
    assert!(validate_contact_name(Some(s)).is_err());
}

// ── status-note cap ───────────────────────────────────────────────────────

#[test]
fn status_note_absent_empty_and_at_cap_are_accepted() {
    // Most transitions carry no note at all.
    assert_eq!(validate_status_note(None).unwrap(), "");
    assert_eq!(validate_status_note(Some(String::new())).unwrap(), "");
    // Surrounding whitespace is trimmed, not rejected.
    assert_eq!(
        validate_status_note(Some("  called the recruiter  ".into())).unwrap(),
        "called the recruiter"
    );
    // Exactly at the cap passes — the guard rejects only what is over it.
    let at_cap = "a".repeat(MAX_STATUS_NOTE_BYTES);
    assert_eq!(validate_status_note(Some(at_cap.clone())).unwrap(), at_cap);
}

#[test]
fn status_note_over_the_cap_is_rejected_not_truncated() {
    // One byte over → a typed Validation error naming the limit. Rejecting
    // (not truncating) matches `validate_contact_name`: a silently halved
    // interaction-log entry is worse than a visible, recoverable error.
    let over = "a".repeat(MAX_STATUS_NOTE_BYTES + 1);
    let err = validate_status_note(Some(over)).expect_err("over-cap note must be rejected");
    assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    assert!(
        err.to_string().contains(&MAX_STATUS_NOTE_BYTES.to_string()),
        "the message must name the byte cap, got {err:?}"
    );
}

#[test]
fn status_note_cap_counts_bytes_not_chars() {
    // 'ü' is 2 UTF-8 bytes: 1_001 chars = 2_002 bytes, over the 2_000-byte
    // cap even though a char-count guard would wave it through.
    let s = "ü".repeat(MAX_STATUS_NOTE_BYTES / 2 + 1);
    assert!(s.chars().count() < MAX_STATUS_NOTE_BYTES);
    assert!(s.len() > MAX_STATUS_NOTE_BYTES);
    assert!(validate_status_note(Some(s)).is_err());
}

#[test]
fn recipient_email_multibyte_over_254_bytes_rejected() {
    // 'ü' is 2 UTF-8 bytes; 128 repetitions = 256 bytes > 254-byte cap.
    let long = format!("{}@example.com", "ü".repeat(128));
    assert!(long.len() > 254, "fixture must exceed 254 bytes");
    let err =
        validate_recipient_email(Some(long)).expect_err("over-254-byte address must be rejected");
    assert!(matches!(err, AppError::Validation(_)));
}
