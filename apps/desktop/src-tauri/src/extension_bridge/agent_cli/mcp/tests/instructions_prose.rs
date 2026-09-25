use super::*;
// ── INSTRUCTIONS / build_instructions (items 7, 14, 18, 24, 27) ─────────

#[test]
fn instructions_name_both_missing_pointer_and_app_closed_and_map_cli_phrasing_onto_tools() {
    assert!(INSTRUCTIONS.contains("app_not_located"));
    assert!(INSTRUCTIONS.contains("app_not_running"));
    assert!(INSTRUCTIONS.contains("call-read"));
    assert!(
        INSTRUCTIONS.contains("--confirm"),
        "must map CLI --confirm phrasing onto this tool's own confirm argument"
    );
}

/// A3-r1-AC-4 MEDIUM: the by-NAME clause claimed `title`/`company`/`location`/`description` are
/// ALWAYS third-party scraped text, which is now false for `documents_list`'s `DocumentRecord.
/// title` (`agent_call::fence`'s own origin-aware exemption) -- fencing is by ORIGIN now, so the
/// prose must key its claim on the `<job_posting>` tag alone, never on a field name that can be
/// first-party depending on which command produced it.
#[test]
fn instructions_no_longer_claims_title_is_always_third_party_scraped_text_by_name() {
    assert!(
        !INSTRUCTIONS.contains("Fields named title"),
        "the by-name claim must be gone now that documents_list's own title is first-party: \
         {INSTRUCTIONS}"
    );
    assert!(
        INSTRUCTIONS.contains("<job_posting>...</job_posting> tags"),
        "the tag-based claim must remain -- fencing is by origin, and the tag IS the marker: \
         {INSTRUCTIONS}"
    );
}

/// A3-r3-AC-3: `prompt_fence::EXPECTED_FENCE_TAGS` pins REGISTRATION only, across every tag in
/// the crate -- most entries this dispatch surface never emits -- so it can't serve as
/// `INSTRUCTIONS`'s own coverage source. `agent_call::reshape::EMITTED_FENCE_TAGS` is that
/// source instead: the surface's own hand-audited list of every tag literal `agent_call::fence`
/// and `agent_call::reshape` actually pass to `prompt_fence::fenced`. Deleting the
/// `<user_document>`/`<app_notification>` sentence from `INSTRUCTIONS` -- nothing else in this
/// file would have caught that -- reddens this test.
#[test]
fn instructions_documents_every_fence_tag_this_surface_emits() {
    for tag in agent_call::reshape::EMITTED_FENCE_TAGS {
        let wrapper = format!("<{tag}>...</{tag}>");
        assert!(
            INSTRUCTIONS.contains(&wrapper),
            "INSTRUCTIONS never explains fence tag `{tag}` (expected `{wrapper}` somewhere): \
             {INSTRUCTIONS}"
        );
    }
}

/// SEC-1 fix (issue #1157): `Refusal::InvokeError`'s `command_error` tag is emitted directly by
/// `agent_call.rs`'s own `detail()` (a refusal builder, not the `fence.rs`/`reshape.rs` reply-
/// reshaping pipeline `EMITTED_FENCE_TAGS` scans), so it cannot ride the derived assertion above —
/// hand-written the same way `EMITTED_FENCE_TAGS` itself is, per that const's own doc.
#[test]
fn instructions_documents_the_command_error_tag() {
    assert!(
        INSTRUCTIONS.contains("<command_error>...</command_error>"),
        "INSTRUCTIONS never explains fence tag `command_error`: {INSTRUCTIONS}"
    );
}

/// Issue #1183 F5 (advisory): `agent_call::validate::fenced_key` re-tags an unknown-key refusal's
/// caller-SUPPLIED key text under `command_error` — the same tag `Refusal::InvokeError` uses for
/// the app's own error prose — so the "read the key/argument names in it as actionable" claim
/// must not be read as covering text the caller wrote themselves; only the app's own declared
/// names are a trustworthy fix suggestion. Deleting this scoping clause is what reddens here.
#[test]
fn instructions_scopes_command_error_actionable_names_away_from_the_callers_own_input() {
    assert!(
        INSTRUCTIONS.contains("unless the name only echoes")
            && INSTRUCTIONS.contains("never actionable on its own"),
        "INSTRUCTIONS must not claim a caller-echoed key name is actionable: {INSTRUCTIONS}"
    );
}

#[test]
fn instructions_name_connection_lost_alongside_rate_limited_in_the_no_retry_sentence() {
    // item 18 — a payload too large for the bridge frame surfaces as connection_lost, which
    // reads as transient; naming only rate_limited invited a retry loop.
    assert!(INSTRUCTIONS.contains("connection_lost"));
    assert!(INSTRUCTIONS.contains("rate_limited"));
}

/// Issue #1155's user-facing half: `rate_limited` gained a `retryAfterMs` wait hint on the wire
/// (`agent_read::throttled_reply`/`agent_call::throttled_reply`), but the model that reads THIS
/// prose — never the wire shape directly — never learns the field exists unless it is named here
/// too. Mutation-checked: reverting `instructions.rs`'s added clause turns this red.
#[test]
fn instructions_tell_the_model_rate_limited_carries_a_retry_after_ms_wait_hint() {
    assert!(
        INSTRUCTIONS.contains("retryAfterMs"),
        "the no-retry-loop sentence must also say a rate_limited result carries retryAfterMs"
    );
}

/// Issue #1170 — INSTRUCTIONS now points a caller at the résumé/document reads before it judges
/// fit. Every `ns:cmd`-shaped token the prose cites must be a REAL `POLICY` row AND an
/// `Effect::Read` row: the sentence tells a caller to reach it through `call-read`, so a row that
/// was ever anything else earns that caller a `wrong_tool` refusal (issue #1164 — the earlier
/// version of this test checked existence only, which a `git mv`-style rename would catch but a
/// reclassification would not). The one hand-written skip is `ns:cmd` itself — the earlier "a
/// detail that says `agent call ns:cmd`" sentence uses it as a PLACEHOLDER, not a real pair (same
/// "skip list, not a substring match" discipline [`EXPLAINED_IN_PROSE`] already uses above).
#[test]
fn instructions_ns_cmd_pairs_are_real_policy_rows() {
    const SKIP: &[&str] = &["ns:cmd"];
    let is_ident = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '_');
    let mut checked = 0usize;
    for word in INSTRUCTIONS.split_whitespace() {
        let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != ':' && c != '_');
        let Some((ns, cmd)) = word.split_once(':') else {
            continue;
        };
        if !is_ident(ns) || !is_ident(cmd) || SKIP.contains(&word) {
            continue;
        }
        checked += 1;
        let entry = POLICY
            .iter()
            .find(|e| agent_call::split_path(e.path) == (ns, cmd))
            .unwrap_or_else(|| {
                panic!("INSTRUCTIONS names `{word}`, which is not a real POLICY row")
            });
        assert!(
            matches!(entry.effect, Effect::Read),
            "INSTRUCTIONS tells a caller to reach `{word}` via call-read, but its POLICY row is \
             not Effect::Read"
        );
    }
    assert_eq!(
        checked, 2,
        "expected exactly the 2 résumé/document ns:cmd pairs (round 5, `B1-r1-ACLI-R5-4`): \
         documents:documents_list (fenced/capped rows) AND documents:documents_get_text (the \
         SAME text by id, fenced and capped at the SAME limit — its `id` param maps to \
         documents_list's `_id` value, spelled out in the prose rather than dropping the \
         command entirely; its reply is now fenced too, `B1-r1-ACLI-R5-7`; neither call can \
         return more of a document than the fence cap, `B1-r2-ACLI-R6-1`): {INSTRUCTIONS}"
    );
}
