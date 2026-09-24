use super::*;

#[test]
fn registry_includes_all_cli_agents() {
    for id in [
        ProviderId::ClaudeCode,
        ProviderId::Codex,
        ProviderId::GeminiCli,
        ProviderId::Antigravity,
    ] {
        assert!(
            backend_for(id).is_some(),
            "{} should be registered",
            id.as_str()
        );
    }
    assert!(all().iter().all(|b| b.id().is_cli_agent()));
}

#[test]
fn non_cli_provider_has_no_backend() {
    assert!(backend_for(ProviderId::Anthropic).is_none());
}

/// Codex (3 tiers) and Claude Code (5 tiers) actually read `effort`; every
/// other CLI agent accepts the parameter but ignores it, so the picker must
/// not appear for them. Claude Code's five tiers are written out by hand on
/// purpose (same reasoning as `TIER_ORDER`'s in
/// commands/ai_provider/tests.rs): a pin driven off the same
/// `claude_code::EFFORT_LEVELS` const it guards would pass no matter how
/// the allowlist is reordered or renamed — and
/// `every_providers_effort_levels_list_its_lowest_tier_first`
/// (commands/ai_provider/tests.rs) separately pins that entry ZERO of each
/// list is its lowest tier.
#[test]
fn effort_levels_only_populated_for_codex_and_claude_code() {
    assert_eq!(
        CliAgentClient::new(backend_for(ProviderId::Codex).unwrap()).effort_levels(""),
        vec!["low", "medium", "high"]
    );
    assert_eq!(
        CliAgentClient::new(backend_for(ProviderId::ClaudeCode).unwrap()).effort_levels(""),
        vec!["low", "medium", "high", "xhigh", "max"]
    );
    for id in [ProviderId::GeminiCli, ProviderId::Antigravity] {
        assert!(
            CliAgentClient::new(backend_for(id).unwrap())
                .effort_levels("")
                .is_empty(),
            "{} must not offer an effort picker",
            id.as_str()
        );
    }
}

/// Regression: an agent that emits `Done` (or a whitespace-only delta) and
/// THEN exits non-zero used to report the generic empty-answer message and
/// discard the stderr explaining the real cause. That path skips
/// `run_stream`'s earlier `!emitted_done && !success` guard entirely.
#[test]
fn a_nonzero_exit_after_done_prefers_the_stderr_diagnosis() {
    let empty = AppError::Provider(super::super::stream::EMPTY_ANSWER_MESSAGE.to_string());
    let err = terminal_error(
        empty,
        false,
        "codex",
        Some(1),
        "Error: not logged in. Run `codex login`.",
    );
    // `friendly_cli_error` recognises the auth shape and upgrades it to the
    // actionable Config error — the whole point of preferring stderr.
    assert!(
        matches!(err, AppError::Config(ref m) if m.contains("not signed in")),
        "expected the sign-in diagnosis, got {err:?}"
    );
}

/// Even with unrecognised stderr, a non-zero exit must surface the exit code
/// rather than claim the model simply returned nothing.
#[test]
fn a_nonzero_exit_with_opaque_stderr_still_beats_the_empty_answer_message() {
    let empty = AppError::Provider(super::super::stream::EMPTY_ANSWER_MESSAGE.to_string());
    let err = terminal_error(empty, false, "codex", Some(3), "segfault at 0x0");
    let msg = format!("{err}");
    assert!(
        msg.contains("segfault") || msg.contains("exit 3"),
        "got {msg}"
    );
    assert!(
        !msg.contains("no answer content"),
        "the generic empty-answer message must not win over a real failure: {msg}"
    );
}

/// The differential: a CLEAN exit that produced nothing has no stderr
/// diagnosis to prefer, so the empty-answer message must survive untouched.
#[test]
fn a_clean_exit_keeps_the_empty_answer_message() {
    let empty = AppError::Provider(super::super::stream::EMPTY_ANSWER_MESSAGE.to_string());
    let err = terminal_error(empty, true, "codex", Some(0), "some harmless warning");
    assert!(
        format!("{err}").contains("no answer content"),
        "a clean exit must keep the empty-answer message, got {err:?}"
    );
}

/// Real clap v4 shape from an old Codex that doesn't know the Part 1d
/// isolation flags: the raw dump must become a readable "update the CLI"
/// error, not leak the argument parser text.
#[test]
fn codex_unexpected_argument_maps_to_an_update_the_cli_error() {
    let stderr = "error: unexpected argument '--ignore-user-config' found\n\n\
                  Usage: codex exec [OPTIONS] [PROMPT]...\n\n\
                  For more information, try '--help'.\n";
    let err = friendly_cli_error("codex", Some(2), stderr);
    let msg = format!("{err}");
    assert!(
        msg.contains("Update the Codex CLI"),
        "expected the update instruction, got {msg}"
    );
    assert!(
        !msg.contains("Usage: codex exec"),
        "the raw clap dump must not leak through: {msg}"
    );
}

#[test]
fn stdout_noise_filter_drops_only_operational_lines() {
    // Known operational noise is dropped…
    assert!(is_cli_stdout_noise("Loaded cached credentials."));
    assert!(is_cli_stdout_noise("  Data collection is disabled.  "));
    assert!(is_cli_stdout_noise(
        "[dotenv@17.0.0] injecting env (2) from .env"
    ));
    assert!(is_cli_stdout_noise("(node:12345) Warning: something"));
    // …while real answer text (even mentioning credentials) is kept, and blank
    // lines survive as paragraph breaks.
    assert!(!is_cli_stdout_noise("Dear Hiring Manager,"));
    assert!(!is_cli_stdout_noise(
        "I loaded cached credentials into the pipeline as described."
    ));
    assert!(!is_cli_stdout_noise(""));
}

#[test]
fn arg_token_accepts_ids_and_rejects_shell_metacharacters() {
    // Real model ids / effort levels pass unchanged (trimmed).
    assert_eq!(arg_token("gpt-5-codex"), Some("gpt-5-codex"));
    assert_eq!(arg_token("gemini-2.5-pro"), Some("gemini-2.5-pro"));
    assert_eq!(arg_token("o4-mini"), Some("o4-mini"));
    assert_eq!(arg_token("high"), Some("high"));
    assert_eq!(arg_token("  gemini-2.5-flash  "), Some("gemini-2.5-flash"));
    // opencode uses provider/model format (e.g. "openai/gpt-4o")
    assert_eq!(arg_token("openai/gpt-4o"), Some("openai/gpt-4o"));
    assert_eq!(
        arg_token("anthropic/claude-3-5-sonnet"),
        Some("anthropic/claude-3-5-sonnet")
    );
    // Leading `-` (flag-like) and `/` (Windows cmd switch) are rejected.
    assert_eq!(arg_token("-malicious"), None);
    assert_eq!(arg_token("/malicious"), None);
    assert_eq!(arg_token("-m"), None);
    assert_eq!(arg_token("/c"), None);
    // Shell metacharacters / whitespace-splitting / empties are rejected, so the
    // flag is omitted rather than smuggling text through `cmd.exe` on Windows
    // (the CVE-2024-24576 argv invariant, defended in depth).
    for bad in [
        "", "   ", "a b", "m&calc", "a|b", "a>b", "a<b", "a^b", "%PATH%", "a\"b", "a(b)", "a\r\nb",
        "$(x)", "`x`", "a;b",
    ] {
        assert_eq!(arg_token(bad), None, "{bad:?} must be rejected");
    }
}

#[tokio::test]
async fn cancel_poll_breaks_a_stalled_line_read() {
    use std::io;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // Models `run_stream`'s read loop: the line read stalls forever while the
    // cancel flag flips. The `biased` select must reach the poll branch and
    // yield `ReadOutcome::Cancelled` instead of hanging on the read.
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        flag.store(true, Ordering::SeqCst);
    });

    let outcome = async {
        loop {
            tokio::select! {
                biased;
                // A stalled stream: the next line never arrives.
                next = std::future::pending::<io::Result<Option<String>>>() => {
                    break match next {
                        Ok(Some(l)) => ReadOutcome::Line(l),
                        Ok(None) => ReadOutcome::Eof,
                        Err(e) => ReadOutcome::Err(e),
                    };
                }
                _ = tokio::time::sleep(CANCEL_POLL) => {
                    if cancelled.load(Ordering::SeqCst) {
                        break ReadOutcome::Cancelled;
                    }
                }
            }
        }
    }
    .await;

    // Cancel observed within a bounded number of polls — no hang, and it is a
    // *cancel*, distinct from a natural EOF.
    assert!(matches!(outcome, ReadOutcome::Cancelled));
}

#[tokio::test]
async fn eof_without_cancel_is_clean_completion_not_a_cancel() {
    use std::io;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // A natural EOF that coincides with *no* cancellation must resolve to
    // `Eof` (clean break), never `Cancelled`. The biased line read wins over
    // the poll, so even if a cancel were racing the EOF still takes priority —
    // here cancel never fires, so the only correct outcome is `Eof`.
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();

    let outcome = async {
        loop {
            tokio::select! {
                biased;
                // Stream end: the read is immediately ready with `Ok(None)`.
                next = async { io::Result::Ok(None::<String>) } => {
                    break match next {
                        Ok(Some(l)) => ReadOutcome::Line(l),
                        Ok(None) => ReadOutcome::Eof,
                        Err(e) => ReadOutcome::Err(e),
                    };
                }
                _ = tokio::time::sleep(CANCEL_POLL) => {
                    if flag.load(Ordering::SeqCst) {
                        break ReadOutcome::Cancelled;
                    }
                }
            }
        }
    }
    .await;

    // A real EOF is never misreported as a cancellation.
    assert!(matches!(outcome, ReadOutcome::Eof));
}

#[test]
fn resolve_models_prefers_live_discovery_over_the_curated_fallback() {
    let live = vec![json!({ "name": "gpt-6-astra", "displayName": "GPT-6 Astra" })];
    let out = resolve_models(Some(live.clone()), &["gpt-5-codex", "o4-mini"]);
    assert_eq!(out, live);
}

#[test]
fn resolve_models_falls_back_and_labels_the_source_when_discovery_is_none() {
    let out = resolve_models(None, &["gpt-5-codex", "o4-mini"]);
    assert_eq!(
        out,
        vec![
            json!({ "name": "gpt-5-codex", "source": "fallback" }),
            json!({ "name": "o4-mini", "source": "fallback" }),
        ]
    );
}

/// Discovery running and finding nothing usable is not "the CLI has zero
/// models" — it's the same "no live source available" case as `None`.
#[test]
fn resolve_models_treats_an_empty_discovery_result_as_no_discovery() {
    let out = resolve_models(Some(Vec::new()), &["gpt-5-codex"]);
    assert_eq!(
        out,
        vec![json!({ "name": "gpt-5-codex", "source": "fallback" })]
    );
}

/// PR #1187 review: `ProviderModelInfo.source` in the TS contract
/// (`packages/shared/src/ipc/contracts/ai.ts`) is `?: 'fallback'` — present
/// with that exact value on a curated entry, ABSENT (not `null`) on a live
/// one. `.get("source")` pins that field-presence contract directly, rather
/// than relying on whole-value equality alone.
#[test]
fn fallback_entries_carry_source_and_live_entries_omit_the_key_entirely() {
    let fallback_out = resolve_models(None, &["gpt-5-codex"]);
    assert_eq!(
        fallback_out[0].get("source").and_then(Value::as_str),
        Some("fallback")
    );

    let live = vec![json!({ "name": "gpt-6-astra" })];
    let live_out = resolve_models(Some(live), &["gpt-5-codex"]);
    assert!(live_out[0].get("source").is_none());
}

#[tokio::test]
async fn detect_missing_binary_is_false() {
    let (ok, version) = detect("ajh-definitely-not-a-real-binary-x9z").await;
    assert!(!ok);
    assert!(version.is_none());
}

#[tokio::test]
async fn detect_cached_serves_cached_result_within_ttl() {
    let bin = "ajh-cache-probe-binary-not-real-q7w";
    // First call probes (binary missing) and caches the negative result.
    assert_eq!(detect_cached(bin).await, (false, None));
    // Poison the cache with a value a real probe could never produce, then
    // confirm the next call returns it — proving it read the cache, not the
    // binary (i.e. no re-spawn within the TTL).
    detect_cache().lock().insert(
        bin.to_string(),
        Detected {
            ok: true,
            version: Some("9.9.9".into()),
            at: Instant::now(),
        },
    );
    assert_eq!(detect_cached(bin).await, (true, Some("9.9.9".to_string())));
}
