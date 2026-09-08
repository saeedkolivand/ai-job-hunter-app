//! Split out of `policy.rs` (R8's hard LOC cap — the same reason
//! `agent_call/tests.rs`/`documents/sql.rs`/`applications/reminders.rs`
//! exist) — this is tests only, no logic, so it earns its own file the
//! moment the combined module crosses the cap rather than growing the
//! production file further.

use std::collections::HashSet;

use super::*;

/// The `lib.rs` source, embedded at compile time — the SAME text
/// `cargo build` feeds to `tauri::generate_handler!`, so extraction from
/// it can never drift from what is actually wired up (mirrors
/// `commands::cli_agents::tests`' `include_str!` of the capability
/// allowlist for the identical reason).
const LIB_RS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));

/// Extract the fully-qualified command paths registered inside
/// `tauri::generate_handler![...]`, in source order. Comment-only and
/// blank lines inside the list are skipped; every other line is
/// trimmed of its trailing comma. Panics (test-only) if the markers
/// this depends on ever move — that failure itself is the signal this
/// extraction needs updating, not a silent empty result.
///
/// The closing `]` is located on the first NON-comment line that
/// carries one (LOW fix — security review): the naive `rest.find(']')`
/// this used to run against the RAW text would truncate the extraction
/// early if a `//` comment between the marker and the real terminator
/// ever contained a literal `]` — silent, since a truncated-but-still-
/// well-formed list still passes both anti-drift tests below with a
/// SMALLER `POLICY` and a smaller `registered` set, never surfacing the
/// mismatch. Comment lines are skipped when searching for `]`, not when
/// slicing — every real command line up to the true terminator is kept.
fn registered_command_paths() -> Vec<&'static str> {
    const START_MARKER: &str = "tauri::generate_handler![";
    let start = LIB_RS
        .find(START_MARKER)
        .expect("tauri::generate_handler![ marker present in lib.rs")
        + START_MARKER.len();
    let rest: &'static str = &LIB_RS[start..];

    let mut end = None;
    let mut offset = 0usize;
    for line in rest.lines() {
        let is_comment = line.trim_start().starts_with("//");
        if !is_comment {
            if let Some(pos) = line.find(']') {
                end = Some(offset + pos);
                break;
            }
        }
        // `lines()` strips the `\n` each line ended with — add it back
        // so `offset` stays a correct byte position into `rest`.
        offset += line.len() + 1;
    }
    let end = end.expect("generate_handler! list has a closing ] on a non-comment line in lib.rs");

    rest[..end]
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .map(|line| line.trim_end_matches(','))
        .collect()
}

/// Hand-written literal — deliberately NOT derived from `POLICY.len()`
/// or from `registered_command_paths()`. A test that only loops over
/// the table it guards covers additions only (this repo's own standing
/// lesson: `feedback_a_guard_driven_off_its_own_data_cannot_catch_a_deletion`)
/// — this fails independently of either source's own content.
// Named without the count itself (issue #1170, MEDIUM — a prior name
// baked in "167", and two module docs elsewhere cited that name AS the
// authoritative row count, so both silently restated a stale number the
// moment this row grew to 168): the count lives ONLY in the `assert_eq!`
// below, never in a name or a doc pointer to this test.
#[test]
fn policy_table_row_count_is_pinned() {
    // 167 + 1 (round 5, `B1-r1-ACLI-R5-1`): `updater::updater_status`, the
    // read-only counterpart added when `updater_check` was reverted from
    // `Read` back to `Reversible`.
    assert_eq!(POLICY.len(), 168);
}

/// ADR-038 §1's core invariant: the policy table and `generate_handler!`
/// agree EXACTLY — no command reachable from the registry without a
/// classified row, and no stale/typo'd row claiming a command that
/// isn't actually registered. Both directions, with the offending
/// command named in the failure message.
#[test]
fn policy_table_matches_generate_handler_exactly() {
    let registered: HashSet<&str> = registered_command_paths().into_iter().collect();
    let policy: HashSet<&str> = POLICY.iter().map(|e| e.path).collect();

    let missing: Vec<&&str> = registered.difference(&policy).collect();
    assert!(
        missing.is_empty(),
        "commands registered in generate_handler! but missing a POLICY row \
         (unclassified — must be added): {missing:?}"
    );

    let extra: Vec<&&str> = policy.difference(&registered).collect();
    assert!(
        extra.is_empty(),
        "POLICY rows with no matching generate_handler! registration \
         (stale, or the path is typo'd): {extra:?}"
    );
}

/// Guards the set-equality test above against a duplicate masking a
/// missing row: two identical `path`s would satisfy both `difference`
/// checks while a third, genuinely-unclassified command silently has
/// no row at all.
#[test]
fn policy_table_has_no_duplicate_paths() {
    let mut seen = HashSet::new();
    for entry in POLICY {
        assert!(
            seen.insert(entry.path),
            "duplicate POLICY row for {}",
            entry.path
        );
    }
}

/// Every `NotExposed` row carries a real, specific reason — never a
/// bare placeholder like "unclear" or "todo" (rule 3 of the
/// classification pass this table was built under).
#[test]
fn not_exposed_rows_carry_a_real_reason() {
    for entry in POLICY {
        if let Effect::NotExposed(reason) = entry.effect {
            assert!(
                reason.trim().len() > 15,
                "{} is NotExposed with a too-short/placeholder reason: {reason:?}",
                entry.path
            );
        }
    }
}

/// LOW fix (pre-PR gate, round 3): a prior edit shortened this row's inline reason to one
/// over-broad claim — "every value extension_bridge_status could offer is one this exact
/// connection already had to possess" — which is true of `port`/`token` but NOT of `connected`,
/// a boolean that reads `true` only because THIS socket's own successful authentication is what
/// increments the counter it reports. The longer row comment above always had both clauses; the
/// short inline reason (what a caller/refusal actually sees) must too.
#[test]
fn extension_bridge_regenerate_token_reason_explains_why_connected_is_vacuous_too() {
    let entry = POLICY
        .iter()
        .find(|e| e.path.ends_with("extension_bridge_regenerate_token"))
        .expect("the row must still exist");
    let Effect::NotExposed(reason) = entry.effect else {
        panic!("must stay NotExposed");
    };
    assert!(
        reason.contains("connected"),
        "must name `connected` specifically, not just `port`/`token`: {reason}"
    );
    assert!(
        reason.contains("THIS socket") || reason.contains("this socket"),
        "must explain WHY connected is vacuous (self-authentication increments it), not just \
         assert it: {reason}"
    );
}

/// HIGH fix (security review round 2): `match_resume`/`match_resume_text`
/// reach a paid embedding provider (`score_one` → `embed_charged`) with
/// `budget: None` when `semanticScoringEnabled: true` — the SAME
/// uncapped-spend shape that forced `ai_embed` `NotExposed` before ITS
/// gate landed. Hand-written (not looped, mirroring
/// `policy_table_row_count_is_pinned`'s own discipline): a revert of
/// either row back to `Reversible` (freely dispatchable, no confirm, no
/// cap) would not be caught by any OTHER test in this file — the
/// Irreversible-row count is untouched by a `Reversible` change, and
/// `not_exposed_rows_carry_a_real_reason` only checks rows that ARE
/// `NotExposed`, never that a specific row remains one.
#[test]
fn match_resume_and_match_resume_text_stay_not_exposed_until_a_real_charge_lands() {
    for path in [
        "commands::match_resume::match_resume",
        "commands::match_resume::match_resume_text",
    ] {
        let entry = POLICY
            .iter()
            .find(|e| e.path == path)
            .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
        assert!(
            matches!(entry.effect, Effect::NotExposed(_)),
            "{path} must stay NotExposed (uncharged paid-embedding path) until a real \
             charge_provider_daily gate lands on it — got {:?}",
            entry.effect
        );
    }
}

/// `registered_command_paths` itself must find every command `lib.rs`
/// actually registers — sanity-checks the extraction against a handful
/// of paths spanning the start, middle and end of the list, so a
/// regression in the marker/parsing logic (not just a POLICY drift)
/// is caught here rather than surfacing as a confusing mismatch above.
#[test]
fn extraction_finds_known_paths_at_each_end_of_the_list() {
    let found = registered_command_paths();
    assert_eq!(
        found.first().copied(),
        Some("commands::cli_agents::cli_agents_status"),
        "extraction must find the FIRST registered command"
    );
    assert_eq!(
        found.last().copied(),
        Some("updater::updater_changelog"),
        "extraction must find the LAST registered command"
    );
    assert!(found.contains(&"commands::privacy::privacy_reset_app"));
    assert_eq!(found.len(), 168);
}

/// ADR-038 §4 (Phase 3): every `Irreversible` row's
/// `ProofSource::read_command` must itself be a REAL `Effect::Read` row
/// in this SAME table — the ceremony's whole safety property rests on
/// the proof coming from a surface this table has independently
/// classified as safe to dispatch freely. A `ProofSource` pointing at a
/// command that doesn't exist, or exists but isn't `Read`, would make
/// the ceremony either uncheckable or a second mutation smuggled in
/// under "reading the proof".
#[test]
fn every_proof_source_read_command_is_a_read_row() {
    let mut checked = 0usize;
    for entry in POLICY {
        let Effect::Irreversible(source) = entry.effect else {
            continue;
        };
        checked += 1;
        let read_command = source.read_command();
        let target = POLICY
            .iter()
            .find(|e| e.path.rsplit("::").next() == Some(read_command));
        match target {
            Some(t) if t.effect == Effect::Read => {}
            Some(t) => panic!(
                "{}'s ProofSource points at `{read_command}`, which is classified \
                 {t:?}, not Read",
                entry.path
            ),
            None => panic!(
                "{}'s ProofSource points at `{read_command}`, which has no POLICY row \
                 at all",
                entry.path
            ),
        }
    }
    // Hand-written literal (not derived from POLICY itself — the same
    // "pair a loop with a literal" discipline as
    // `policy_table_row_count_is_pinned`): 34 Irreversible rows
    // (`extension_bridge_regenerate_token` moved to `NotExposed` —
    // security review round 1; `ai_embed` moved NotExposed → Irreversible
    // once its `charge_provider_daily` gate landed, and
    // `match_resume`/`match_resume_text` moved Reversible → NotExposed
    // for the SAME reason `ai_embed` originally was — security review
    // round 2; security review round 3 nets +1: `ai_set_active_provider`
    // and `ai_set_provider_settings` moved Reversible → Irreversible
    // [+2], `support_export_diagnostics` moved Irreversible →
    // `NotExposed` for a vacuous proof [-1]; security review round 4 nets
    // 0: `ai_set_provider_settings` moved Irreversible → `NotExposed` —
    // its proof was bound to `activeProvider` while its own patch targets
    // a DIFFERENT, caller-chosen `provider` field entirely, so the
    // ceremony never checked the thing it was rewriting [-1] —
    // `ai_pull_model` moved Reversible → Irreversible: no in-app path
    // undoes a pulled multi-GB Ollama model, which is `Irreversible`'s own
    // definition regardless of nothing being destroyed [+1]; `scrape_
    // hybrid_search` adds ONE new Irreversible row for the same
    // charge_provider_daily reason as `ai_embed`/`autopilot_run` [+1];
    // `help_search` added one for that same reason, then moved Irreversible
    // → `NotExposed` (issue #1169): the corpus it would embed is the
    // caller's OWN `entries` field, not anything Rust can read, so no
    // dispatch here ever has a real corpus to search [-1];
    // `notifications_mark_read`/`notifications_mark_all_read` moved
    // Reversible → Irreversible (issue #1164): no "mark unread" exists
    // anywhere on this surface, so flipping the bit is permanent, same as
    // `notifications_remove`/`notifications_clear_all` whose ProofSource
    // shapes they now reuse [+2]; see each row's own comment).
    assert_eq!(checked, 35, "expected exactly 35 Irreversible rows");
}

/// Hand-written pin (security review round 3), mirroring
/// `match_resume_and_match_resume_text_stay_not_exposed_until_a_real_
/// charge_lands`'s own discipline: a revert of any of these four rows
/// back to `Read`/`Irreversible` (freely dispatchable, or dispatchable
/// with a proof that no longer applies) would not be caught by any
/// OTHER test in this file. `ai_test_provider_key`/
/// `ai_list_provider_models` send a caller-supplied `base_url` a
/// keychain secret; `resume::extract_resume` reads a fully
/// caller-controlled filesystem path with no validation;
/// `support_export_diagnostics` had only a vacuous compile-time-constant
/// proof. See each row's own comment for the full argument.
#[test]
fn round_3_destination_and_vacuous_proof_rows_stay_not_exposed() {
    for path in [
        "commands::ai::ai_test_provider_key",
        "commands::ai::ai_list_provider_models",
        "commands::resume::extract_resume",
        "commands::support::support_export_diagnostics",
    ] {
        let entry = POLICY
            .iter()
            .find(|e| e.path == path)
            .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
        assert!(
            matches!(entry.effect, Effect::NotExposed(_)),
            "{path} must stay NotExposed — got {:?}",
            entry.effect
        );
    }
}

/// Hand-written pin (security review round 3, narrowed round 4 — see
/// `round_4_persistent_redirect_and_unbound_proof_rows_stay_not_exposed`
/// for the sibling row this test used to also cover): a revert back to
/// `Reversible` would silently restore free routing-flip dispatch with no
/// confirm and no proof.
#[test]
fn ai_set_active_provider_stays_irreversible() {
    let path = "commands::ai::ai_set_active_provider";
    let entry = POLICY
        .iter()
        .find(|e| e.path == path)
        .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
    let Effect::Irreversible(ProofSource::Scalar {
        read_command,
        path: field_path,
    }) = entry.effect
    else {
        panic!(
            "{path} must stay Irreversible with a Scalar proof — got {:?}",
            entry.effect
        );
    };
    assert_eq!(
        read_command, "ai_active_config",
        "{path}'s proof must keep reading ai_active_config"
    );
    assert_eq!(
        field_path,
        ["activeProvider"].as_slice(),
        "{path}'s proof must keep reading the activeProvider field"
    );
}

/// Hand-written pin (security review round 4): a revert of any of these
/// rows would silently restore a live primitive round 4 closed — see each
/// row's own comment. `ai_set_embedding_config` and `ai_seed_active_config`
/// both persist a caller-supplied `base_url` that every subsequent embed/
/// generate call (résumé/job text, the stored provider API key) then reads
/// back and sends to — worse than a one-shot redirect, permanent until the
/// config is changed again (`ai_seed_active_config` was found independently
/// during this round's re-sweep, not named by the original review).
/// `ai_set_provider_settings` takes a caller-CHOSEN `provider` field
/// unrelated to the confirmed `activeProvider`, so its old Scalar proof
/// never bound to the record it actually rewrites (the module doc's
/// clause-2 NotExposed rule).
#[test]
fn round_4_persistent_redirect_and_unbound_proof_rows_stay_not_exposed() {
    for path in [
        "commands::ai::ai_set_embedding_config",
        "commands::ai::ai_seed_active_config",
        "commands::ai::ai_set_provider_settings",
    ] {
        let entry = POLICY
            .iter()
            .find(|e| e.path == path)
            .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
        assert!(
            matches!(entry.effect, Effect::NotExposed(_)),
            "{path} must stay NotExposed — got {:?}",
            entry.effect
        );
    }
}

/// Hand-written pin (MCP security critique): a revert of this row back to
/// `Read` would silently let the generic tier — and every MCP `call-read`
/// client — hand back the bridge's plaintext pairing token verbatim. No
/// OTHER test in this file would catch that: the row-count tests don't
/// change (an `Effect` swap, not an add/remove), and
/// `not_exposed_rows_carry_a_real_reason` only checks rows that ARE already
/// `NotExposed`.
#[test]
fn extension_bridge_status_stays_not_exposed_so_the_pairing_token_never_reaches_a_caller() {
    let path = "commands::extension_bridge::extension_bridge_status";
    let entry = POLICY
        .iter()
        .find(|e| e.path == path)
        .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
    assert!(
        matches!(entry.effect, Effect::NotExposed(_)),
        "{path} must stay NotExposed — got {:?}",
        entry.effect
    );
}

/// Issue #1164 round 2 (`B1-r2-B2-r2-ACLI-2`): `notifications_list` returns EVERY notification,
/// read and unread, while `notifications_mark_all_read` only flips the unread subset — so the
/// `ProofSource::Count` comment above that row must call the count a superset of the blast
/// radius, never claim it is "exact". Pinned against the source text rather than behaviour
/// because the defect was the COMMENT lying about what the count proves, not the `ProofSource`
/// shape itself (round 1 sanctioned keeping `Count` here).
#[test]
fn mark_all_read_proof_comment_calls_the_count_a_superset_not_exact() {
    const POLICY_RS: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/extension_bridge/agent_cli/policy.rs"
    ));
    let row = POLICY_RS
        .find("notifications_mark_all_read")
        .expect("notifications_mark_all_read row present in policy.rs");
    let comment = &POLICY_RS[..row];
    let comment_start = comment
        .rfind("// Same no-inverse argument")
        .expect("notifications_mark_all_read's leading comment block present in policy.rs");
    let comment = &comment[comment_start..];
    assert!(
        comment.contains("superset"),
        "the comment must call the notifications_list count a superset of what actually \
         flips: {comment}"
    );
    assert!(
        !comment.contains("exact count about to be flipped"),
        "the comment must not claim the total count is the exact count about to flip — only \
         the unread subset flips: {comment}"
    );
}

/// Round 5 (`B1-r1-ACLI-R5-3`): direct pin for both rows issue #1164 reclassified — the aggregate
/// row count (`policy_table_row_count_is_pinned`) and the Irreversible tally
/// (`every_proof_source_read_command_is_a_read_row`'s trailing `checked == 35`) are BOTH blind to
/// a revert of one row paired with an unrelated +1/-1 elsewhere in the table: `checked` stays 35
/// either way. `notifications_mark_read` had no direct pin anywhere (it appeared in this file only
/// inside a comment); `notifications_mark_all_read` was pinned only for its comment's WORDING
/// (`mark_all_read_proof_comment_calls_the_count_a_superset_not_exact` above), never its `Effect`
/// or `ProofSource` shape. This asserts both rows' actual classification and proof shape directly.
#[test]
fn notifications_mark_rows_stay_irreversible_with_their_proof_shapes() {
    let mark_read = POLICY
        .iter()
        .find(|e| e.path == "commands::notifications::notifications_mark_read")
        .expect("commands::notifications::notifications_mark_read is a real POLICY row");
    let Effect::Irreversible(ProofSource::ListMatch {
        read_command,
        id_field,
        match_field,
        value_field,
    }) = mark_read.effect
    else {
        panic!(
            "notifications_mark_read must be Irreversible(ProofSource::ListMatch), got {:?}",
            mark_read.effect
        );
    };
    assert_eq!(read_command, "notifications_list");
    assert_eq!(id_field, &["id"]);
    assert_eq!(match_field, "id");
    assert_eq!(value_field, "title");

    let mark_all_read = POLICY
        .iter()
        .find(|e| e.path == "commands::notifications::notifications_mark_all_read")
        .expect("commands::notifications::notifications_mark_all_read is a real POLICY row");
    let Effect::Irreversible(ProofSource::Count { read_command }) = mark_all_read.effect else {
        panic!(
            "notifications_mark_all_read must be Irreversible(ProofSource::Count), got {:?}",
            mark_all_read.effect
        );
    };
    assert_eq!(read_command, "notifications_list");
}

/// Round 5 (`B1-r1-ACLI-R5-1`): `updater_install`'s proof must read the PENDING version off
/// `updater_status` — the read-only counterpart added when `updater_check` was reverted from
/// `Read` back to `Reversible` (it writes `UpdaterState`/emits an event, so it cannot be the proof
/// source for another `Irreversible` row: `every_proof_source_read_command_is_a_read_row` requires
/// the target to be `Effect::Read`) — rather than `system_get_version` (the CURRENTLY RUNNING
/// version — a vacuous proof, since it never changes as a result of confirming).
#[test]
fn updater_install_proof_reads_the_pending_version_off_updater_status() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "updater::updater_install")
        .expect("updater::updater_install is a real POLICY row");
    let Effect::Irreversible(ProofSource::Scalar { read_command, path }) = entry.effect else {
        panic!(
            "updater_install must stay Irreversible(ProofSource::Scalar), got {:?}",
            entry.effect
        );
    };
    assert_eq!(
        read_command, "updater_status",
        "updater_install's proof must read updater_status, not system_get_version's vacuous \
         running-version echo, and not updater_check (Reversible, not a valid Read proof source)"
    );
    assert_eq!(
        path,
        &["version"],
        "updater_install's proof must walk to updater_status's `version` field"
    );
}

/// Round 5 (`B1-r1-ACLI-R5-1`): `updater_check` must stay `Effect::Reversible` — it writes
/// `UpdaterState` and emits `updater:status`, and reclassifying it `Read` (issue #1165) forced
/// `call-read`'s `readOnlyHint` to `false` for every one of this table's 63 other `Read` rows,
/// since the hint is a per-TOOL promise, not per-row (see `mcp::tests::
/// call_read_annotations_claim_read_only`). `updater::updater_status` is the read-only
/// alternative this row's proof now points at.
#[test]
fn updater_check_stays_reversible_not_read() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "updater::updater_check")
        .expect("updater::updater_check is a real POLICY row");
    assert_eq!(
        entry.effect,
        Effect::Reversible,
        "updater_check must stay Reversible, not Read — got {:?}",
        entry.effect
    );
    let status = POLICY
        .iter()
        .find(|e| e.path == "updater::updater_status")
        .expect("updater::updater_status is a real POLICY row");
    assert_eq!(
        status.effect,
        Effect::Read,
        "updater_status must be the genuinely read-only alternative — got {:?}",
        status.effect
    );
}

/// Issue #1169: `commands::help::help_search` must actually BE the `NotExposed` row the doc
/// guard below and `every_proof_source_read_command_is_a_read_row`'s comment both describe — the
/// prior version of this file asserted the PROSE said so without ever reading `POLICY` itself
/// (round-3 finding `B1-r3-ACLI-2`), so reverting the row back to `Irreversible`/`Read` left every
/// other test in this file green.
#[test]
fn help_search_stays_not_exposed() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "commands::help::help_search")
        .expect("commands::help::help_search is a real POLICY row");
    assert!(
        matches!(entry.effect, Effect::NotExposed(_)),
        "commands::help::help_search must stay NotExposed (issue #1169) — got {:?}",
        entry.effect
    );
}

/// Issue #1164 round 2 (`B1-r2-B2-r2-ACLI-6`): `commands::help::help_search` is `NotExposed`
/// (issue #1169), but its own module doc claimed unqualified reachability from the agent CLI /
/// extension bridge in four spans — falsifying the RATIONALE those spans give for re-checking
/// every Zod cap in Rust. Every span that mentions the agent CLI or extension bridge reaching
/// `help_search` must also name its current `NotExposed` status (issue #1169), so a future
/// reclassification of the POLICY row is the only thing that can make the doc true again without
/// a human re-reading it — enforced together with `help_search_stays_not_exposed` above, which
/// pins the ROW itself (round-3 finding `B1-r3-ACLI-2`: this test alone never read `POLICY`).
#[test]
fn help_module_doc_reachability_claims_stay_paired_with_its_not_exposed_status() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "commands::help::help_search")
        .expect("commands::help::help_search is a real POLICY row");
    assert!(
        matches!(entry.effect, Effect::NotExposed(_)),
        "this doc-reachability guard only makes sense while the row is NotExposed — got {:?}",
        entry.effect
    );
    const HELP_RS: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands/help.rs"));
    let lines: Vec<&str> = HELP_RS.lines().collect();
    let mut checked = 0usize;
    for (n, line) in lines.iter().enumerate() {
        if !(line.contains("agent CLI") || line.contains("agent-CLI")) {
            continue; // not a span naming the agent CLI at all
        }
        // A window of comment lines around the mention, so "reach"/"NotExposed"/"#1169" can
        // be on a neighbouring line within the same prose block, not literally the same line.
        let start = n.saturating_sub(6);
        let end = (n + 6).min(lines.len());
        let window = lines[start..end].join("\n");
        if !window.contains("reach") {
            continue; // an "agent CLI" mention unrelated to reachability
        }
        checked += 1;
        assert!(
            window.contains("NotExposed") && window.contains("#1169"),
            "a reachability claim at help.rs line {} must name help_search's current \
             NotExposed status (issue #1169): {window}",
            n + 1
        );
    }
    assert!(
        checked >= 4,
        "expected at least the 4 reachability spans issue #1164 round 2 flagged, found \
         {checked}"
    );
}

/// Mutation-style guard: an `Irreversible` row whose `ProofSource`
/// pointed at ITSELF, or at ANY OTHER `Irreversible` row, would make the
/// ceremony circular — satisfiable only by first satisfying another
/// ceremony, never by reading anything real. The self-only shape (comparing
/// `read_command()` against `entry`'s own bare command name) covers the
/// first clause but not the second — a proof source naming a *different*
/// Irreversible row's command would pass that narrower check. Resolving
/// `read_command()` to its own POLICY row and asserting that row isn't
/// itself `Irreversible` covers both in one comparison: a row that names
/// itself resolves back to `entry`, which is Irreversible by the `if let`
/// above, so self-reference still fails here too — there is no longer a
/// separate self-only branch to keep in sync with this one.
#[test]
fn no_proof_source_points_at_an_irreversible_command() {
    for entry in POLICY {
        if let Effect::Irreversible(source) = entry.effect {
            let read_command = source.read_command();
            let Some(target) = POLICY
                .iter()
                .find(|e| e.path.rsplit("::").next() == Some(read_command))
            else {
                continue; // absent-row case is `every_proof_source_read_command_is_a_read_row`'s job
            };
            assert!(
                !matches!(target.effect, Effect::Irreversible(_)),
                "{}'s ProofSource points at `{read_command}` ({}), which is itself \
                 Irreversible — the ceremony would need another ceremony to satisfy",
                entry.path,
                target.path
            );
        }
    }
}
