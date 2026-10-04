use super::keep_alive_value;

// ── keep_alive_value pure mapping ─────────────────────────────────────────

#[test]
fn keep_alive_value_zero_returns_string_zero() {
    // 0 seconds → "0" (Ollama unload-immediately sentinel, no trailing 's').
    assert_eq!(keep_alive_value(0), "0");
}

#[test]
fn keep_alive_value_300_returns_300s() {
    // Balanced tier: 300 seconds.
    assert_eq!(keep_alive_value(300), "300s");
}

#[test]
fn keep_alive_value_1800_returns_1800s() {
    // Performance / high tier: 1800 seconds (30 min keep-alive).
    assert_eq!(keep_alive_value(1800), "1800s");
}

#[test]
fn keep_alive_value_arbitrary_non_zero() {
    // Any non-zero value gets the 's' suffix.
    assert_eq!(keep_alive_value(60), "60s");
    assert_eq!(keep_alive_value(1), "1s");
    assert_eq!(keep_alive_value(u64::MAX), format!("{}s", u64::MAX));
}
