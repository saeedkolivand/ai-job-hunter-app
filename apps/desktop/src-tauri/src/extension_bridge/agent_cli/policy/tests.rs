//! Split out of `policy.rs` (R8's hard LOC cap — the same reason
//! `agent_call/tests.rs`/`documents/sql.rs`/`applications/reminders.rs`
//! exist) — this is tests only, no logic, so it earns its own file the
//! moment the combined module crosses the cap rather than growing the
//! production file further.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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

// ── generated catalogue coverage (issues #1163, #1158, #1160) ────────────

/// A POLICY row with ZERO renderer `invoke()` references at all — this module's own doc names
/// exactly five such rows; two (`extract_resume`, `support_get_system_info`) are `NotExposed` and
/// so outside this coverage test's scope, and `dialog_open_files` (also `NotExposed`) has a real
/// call site the generator itself records in `catalogue::UNCATALOGUED`. The remaining three are the
/// ONLY Read/Reversible/Irreversible rows the generator has no signal for at all — hand-written,
/// never derived from `catalogue::UNCATALOGUED` (this repo's own standing lesson: a guard driven
/// off its own generated data cannot catch the generator silently dropping a row it used to emit).
/// `updater_status` (issue #1165's read-only counterpart to `updater_check`) has no renderer call
/// site at all — its ONLY consumer today is this agent surface's own `updater_install` confirm
/// proof, dispatched through `agent_call`'s `Webview::on_message` path, never the renderer's
/// `AppClient` — so it is uncatalogued by the same "zero renderer references" reasoning as
/// `boards_list`/`privacy_clear_data`, not an oversight.
const ALLOWLISTED_UNCATALOGUED: &[&str] = &["boards_list", "privacy_clear_data", "updater_status"];

/// Issue #1183 F4 — the ONE row of [`ALLOWLISTED_UNCATALOGUED`] whose `POLICY` `Effect` is
/// [`Effect::Irreversible`] (`privacy_clear_data`: disconnects every board and unconditionally
/// wipes the postings + interactions cache, per that row's own comment). This module's doc frames
/// every `ALLOWLISTED_UNCATALOGUED` entry purely as "zero renderer references", which is true but
/// incomplete for this one: it is also the single dispatchable row `agent_call::validate`'s
/// generated-catalogue key checking cannot reach at all (no `CatalogueArg` list to validate
/// against). Hand-written and checked by `allowlisted_uncatalogued_irreversible_entries_are_
/// exactly_the_irreversible_ones` below so a FUTURE destructive command added to
/// `ALLOWLISTED_UNCATALOGUED` — or `privacy_clear_data` being downgraded off `Irreversible` — must
/// be a deliberate, reviewed edit to this list, never a silent side effect of editing `POLICY` or
/// the allowlist alone.
const ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE: &[&str] = &["privacy_clear_data"];

/// Every dispatchable (`Read`/`Reversible`/`Irreversible`) POLICY row is reachable through
/// `agent_call::validate`'s dispatch-time key checking (issues #1163, #1158, #1160): either the
/// generated `catalogue::CATALOGUE` has an entry for it, the generator itself flagged it in
/// `catalogue::UNCATALOGUED` (a real call site it could not parse with confidence), or it is on
/// this file's own hand-written [`ALLOWLISTED_UNCATALOGUED`] (zero call site at all). A command
/// landing in none of the three is silently UNVALIDATED input with nobody accounting for why.
#[test]
fn every_dispatchable_row_is_catalogued_or_explicitly_accounted_for() {
    let mut uncovered = Vec::new();
    for entry in POLICY {
        if matches!(entry.effect, Effect::NotExposed(_)) {
            continue;
        }
        let command = entry.path.rsplit("::").next().unwrap_or(entry.path);
        let catalogued = super::super::catalogue::CATALOGUE
            .iter()
            .any(|e| e.command == command);
        let accounted_for = catalogued
            || super::super::catalogue::UNCATALOGUED.contains(&command)
            || ALLOWLISTED_UNCATALOGUED.contains(&command);
        if !accounted_for {
            uncovered.push(entry.path);
        }
    }
    assert!(
        uncovered.is_empty(),
        "these Read/Reversible/Irreversible POLICY rows are absent from CATALOGUE, \
         catalogue::UNCATALOGUED, AND this test's own ALLOWLISTED_UNCATALOGUED — a caller can \
         send them any input with none of it checked: {uncovered:?}"
    );
}

/// Allowlists are debt, not absolution (`docs/architecture-rules.md`'s own framing for R2/R7's
/// exception lists): each entry here must still be a real POLICY row, and must still be genuinely
/// absent from the generated catalogue — a future `invoke()` call site added for either command
/// makes the generator catalogue it, and this test then fails until the stale entry is removed.
#[test]
fn allowlisted_uncatalogued_entries_are_still_real_and_still_uncatalogued() {
    for command in ALLOWLISTED_UNCATALOGUED {
        assert!(
            POLICY
                .iter()
                .any(|e| e.path.rsplit("::").next() == Some(*command)),
            "{command} is not a real POLICY row — remove it from ALLOWLISTED_UNCATALOGUED"
        );
        assert!(
            !super::super::catalogue::CATALOGUE
                .iter()
                .any(|e| e.command == *command),
            "{command} is now catalogued — remove it from ALLOWLISTED_UNCATALOGUED"
        );
    }
}

/// Issue #1183 F4: every [`ALLOWLISTED_UNCATALOGUED`] row whose `POLICY` `Effect` is
/// [`Effect::Irreversible`] must appear in [`ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE`], AND every
/// entry of that second list must actually be `Irreversible` — both directions, so the list can
/// neither miss a real destructive uncatalogued row nor carry a stale one. Mutating either list
/// alone, or reclassifying `privacy_clear_data`'s `POLICY` effect, fails this test.
#[test]
fn allowlisted_uncatalogued_irreversible_entries_are_exactly_the_irreversible_ones() {
    for command in ALLOWLISTED_UNCATALOGUED {
        let entry = POLICY
            .iter()
            .find(|e| e.path.rsplit("::").next() == Some(*command))
            .expect("pinned by allowlisted_uncatalogued_entries_are_still_real_and_still_uncatalogued above");
        let is_irreversible = matches!(entry.effect, Effect::Irreversible(_));
        assert_eq!(
            is_irreversible,
            ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE.contains(command),
            "{command} is {}Irreversible in POLICY but {}listed in \
             ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE — keep the two in sync",
            if is_irreversible { "" } else { "NOT " },
            if ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE.contains(command) {
                ""
            } else {
                "not "
            }
        );
    }
    for command in ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE {
        assert!(
            ALLOWLISTED_UNCATALOGUED.contains(command),
            "{command} is in ALLOWLISTED_UNCATALOGUED_IRREVERSIBLE but not in \
             ALLOWLISTED_UNCATALOGUED itself"
        );
    }
}

/// Hand-written mirror of `catalogue::UNCATALOGUED` (MEDIUM — CLI review round 1). The coverage
/// test above pairs `catalogued` with the GENERATED `UNCATALOGUED` array and nothing bounds it —
/// this repo's own standing lesson (see [`ALLOWLISTED_UNCATALOGUED`]'s doc) applies to
/// `UNCATALOGUED` itself, not only to the zero-call-site allowlist: a generator regression that
/// silently reclassified N real commands as uncatalogued would leave
/// `every_dispatchable_row_is_catalogued_or_explicitly_accounted_for` green while un-validating
/// all of them, because that test treats `UNCATALOGUED` membership as accounted-for by
/// construction. Pinned against a literal, exactly like `ALLOWLISTED_UNCATALOGUED` is.
const EXPECTED_UNCATALOGUED: &[&str] = &["dialog_open_files"];

#[test]
fn uncatalogued_matches_the_hand_written_list() {
    assert_eq!(
        super::super::catalogue::UNCATALOGUED,
        EXPECTED_UNCATALOGUED,
        "catalogue::UNCATALOGUED drifted from this test's own hand-written list — if a NEW \
         command legitimately can't be parsed by the generator (a computed key, a spread, a \
         non-literal command name, or a non-object invoke() argument), add it here \
         deliberately; if a command DROPPED OUT of UNCATALOGUED, nothing to do beyond updating \
         this list. A command that appears here without ever having been added on purpose is \
         the regression this test exists to catch."
    );
}

/// Hand-written mirror of every catalogued arg whose `fields` is `Some(&[])` — a wrapper TYPE
/// this generator identified but could not resolve the field names of (MEDIUM — CLI review
/// round 1, issue #1158's "guess the wrapper" gap). `mcp.rs` surfaces these on the wire as
/// `"fields": null`, distinct from omitting the key, so a caller can at least tell "unknown
/// nested shape" apart from "no nested shape" — but nothing accounted for the CLASS itself. A
/// generator regression that silently reclassified a RESOLVED wrapper (e.g. a schema rename
/// dropping out of `schemas/index.ts`) as unresolved would leave every other test green while
/// quietly widening the set of commands a nested-key typo can sail past `agent_call::validate`
/// on. `(command, arg name)` pairs, pinned the same way [`EXPECTED_UNCATALOGUED`] is.
///
/// `ai_clear_stage_override`/`ai_set_stage_override`'s `stage` used to be pinned HERE
/// (A1-r1-AC-3 MEDIUM): `PipelineStage` is a scalar string-union alias
/// (`packages/shared/src/events/pipeline.ts`), not an object wrapper, so the generator's OWN
/// unresolved-named-type fallback was publishing it as `"fields": null` — the exact "this takes a
/// nested object" signal that field means. `gen-agent-catalogue.ts`'s `collectScalarTypeAliasNames`
/// now proves that shape and emits `fields: None` for both rows instead.
const EXPECTED_UNRESOLVED_WRAPPER_ARGS: &[(&str, &str)] = &[
    ("ai_set_provider_settings", "req"),
    ("autopilot_update", "req"),
    // The five below (A1-r1-AC-1 MEDIUM) used to fall through `findParamBinding` to
    // `fields: undefined` (a plain scalar) instead of this unresolved-wrapper shape: their `req`/
    // `prefs`/`filter` params are typed `unknown`, an inline object type literal, or
    // `Parameters<Fn>[0]` — none of those are a `TypeReferenceNode` the generator can look a name
    // up for, but every one IS a genuine object wrapper, not a scalar.
    ("job_preferences_set", "prefs"),
    ("resume_pipeline_run", "req"),
    ("scrape_list_interactions", "filter"),
    ("scrape_persist_job", "req"),
    ("scrape_remove_interaction", "req"),
    ("scrape_update_description", "req"),
    ("system_set_performance_mode", "config"),
];

#[test]
fn unresolved_wrapper_args_match_the_hand_written_list() {
    let mut actual: Vec<(&str, &str)> = Vec::new();
    for entry in super::super::catalogue::CATALOGUE.iter() {
        for arg in entry.args {
            if arg.fields.is_some_and(|f| f.is_empty()) {
                actual.push((entry.command, arg.name));
            }
        }
    }
    actual.sort_unstable();
    let mut expected = EXPECTED_UNRESOLVED_WRAPPER_ARGS.to_vec();
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "the set of catalogued args with an unresolved wrapper type (`fields: Some(&[])`, wire \
         `\"fields\": null`) drifted from this test's own hand-written list — if a NEW command \
         legitimately can't have its wrapper resolved, add it here deliberately; if one \
         DROPPED OUT (now resolved), remove it from this list"
    );
}

/// Hand-written pin for every catalogued arg with a RESOLVED, non-empty nested-field list
/// (`fields: Some(&["…"])`) — the sibling [`EXPECTED_UNRESOLVED_WRAPPER_ARGS`] only pinned the
/// empty class (A1-r1-AC-2 MEDIUM): nothing caught a generator regression that downgraded one of
/// THESE 32 rows to `fields: None` (a plain scalar — nested-key validation silently disabled for
/// that command) or to `fields: Some(&[])` (mis-labelled unresolved on the wire) — only
/// `applications_save_from_posting` had its own dedicated fixture assertion
/// (`validate/tests.rs`'s `resolved_fields` panic). Same shrink-only discipline as
/// [`EXPECTED_UNRESOLVED_WRAPPER_ARGS`]: a command dropping OUT of this list (its wrapper stopped
/// resolving) fails here; add a NEW resolved wrapper here deliberately, never let one through
/// silently.
const EXPECTED_RESOLVED_WRAPPER_ARGS: &[(&str, &str)] = &[
    ("ai_embed", "req"),
    ("ai_generate", "req"),
    ("ai_generations_save", "req"),
    ("ai_generations_update", "req"),
    ("ai_seed_active_config", "config"),
    ("applications_save_from_posting", "req"),
    ("applications_track", "req"),
    ("applications_update", "req"),
    ("autopilot_create", "req"),
    ("contact_profile_set", "profile"),
    ("dedup_mark_not_duplicate", "req"),
    ("discovery_search_companies", "req"),
    ("discovery_set_starred", "req"),
    ("documents_export_and_save", "request"),
    ("documents_export_document", "request"),
    ("documents_import", "req"),
    ("documents_recommend_template", "req"),
    ("documents_render_preview_images", "request"),
    ("generate_pipeline", "req"),
    ("help_search", "req"),
    ("match_resume", "req"),
    ("match_resume_text", "req"),
    ("privacy_set_crash_reporting", "settings"),
    ("referrals_upsert", "req"),
    ("resume_extract_text", "req"),
    ("resume_pipeline_regenerate_section", "req"),
    ("resume_pipeline_resolve_fabrication", "req"),
    ("resume_trim_suggestions", "req"),
    ("resume_validate_content", "req"),
    ("scrape_boards", "req"),
    ("scrape_hybrid_search", "req"),
    ("scrape_url", "req"),
];

#[test]
fn resolved_wrapper_args_match_the_hand_written_list() {
    let mut actual: Vec<(&str, &str)> = Vec::new();
    for entry in super::super::catalogue::CATALOGUE.iter() {
        for arg in entry.args {
            if arg.fields.is_some_and(|f| !f.is_empty()) {
                actual.push((entry.command, arg.name));
            }
        }
    }
    actual.sort_unstable();
    let mut expected = EXPECTED_RESOLVED_WRAPPER_ARGS.to_vec();
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "the set of catalogued args with a RESOLVED nested-field list drifted from this test's \
         own hand-written list — if one DROPPED OUT, a generator regression silently disabled \
         nested-key validation for that command (or mis-labelled it `fields: Some(&[])` on the \
         wire); if a NEW one legitimately resolved, add it here deliberately"
    );
}

/// Upper bound on catalogued rows with an empty `description` (MEDIUM — CLI review round 1,
/// issue #1163's own headline ask: "every command row should carry a one-line description").
/// 58 of 162 carried none at the time this guard was added, including `applications_delete` —
/// backfilling TSDoc across ~160 IPC contract members is real, ongoing documentation debt this
/// generator cannot force by itself, so the cap is not zero. What it DOES catch: this count
/// growing — a newly added command shipping with no TSDoc on its contract member, silently
/// leaving the agent-CLI surface with less self-description than it had before. Lower this
/// constant (never raise it) as descriptions are backfilled.
const MAX_NO_DESCRIPTION_ROWS: usize = 48;

#[test]
fn catalogued_no_description_count_does_not_regress() {
    let no_description = super::super::catalogue::CATALOGUE
        .iter()
        .filter(|e| e.description.is_empty())
        .count();
    assert!(
        no_description <= MAX_NO_DESCRIPTION_ROWS,
        "{no_description} catalogued commands have no description (cap \
         {MAX_NO_DESCRIPTION_ROWS}) — a new command shipped with no TSDoc on its IPC contract \
         member; add one. If this failed after backfilling docs elsewhere and the count is now \
         LOWER, lower MAX_NO_DESCRIPTION_ROWS to match (never raise it)."
    );
}

/// Hand-written allowlist of `Effect::Irreversible` commands that STILL carry no catalogue
/// description (CLI review round 2 — MEDIUM: [`MAX_NO_DESCRIPTION_ROWS`] above is tier-blind, so
/// it could not stop a NEW destructive command shipping with no TSDoc as long as some unrelated
/// read command gained one elsewhere). Every name here is app-data-wipe/bulk-delete territory —
/// exactly where a caller needs the description most. Mirrors
/// `EXPECTED_UNRESOLVED_WRAPPER_ARGS`'s own "shrink only" discipline: remove an entry the moment
/// its TSDoc is backfilled; a PR that adds a NEW `Irreversible` row here without also adding a
/// description is the regression this guards against.
/// A1-r1-AC-6 MEDIUM backfilled TSDoc on 10 of the original 11 rows here (the smallest remaining
/// gap on #1163's stated Expected) — `privacy_clear_data` is the one deliberate holdout: it has
/// ZERO renderer call sites (see its own `POLICY` entry's doc), so there is no `invoke()` call
/// site for the generator to attach a description to, and inventing an unused TS contract member
/// just to carry TSDoc would be dead code. Reclassifying it `NotExposed` to sidestep this was
/// considered and rejected — that would gate a working destructive verb away, which this repo's
/// own rule set forbids regardless of review pressure.
const EXPECTED_IRREVERSIBLE_NO_DESCRIPTION: &[&str] = &["privacy_clear_data"];

#[test]
fn every_irreversible_row_without_a_description_is_on_the_hand_written_list() {
    let mut actual: Vec<&str> = POLICY
        .iter()
        .filter_map(|entry| {
            if !matches!(entry.effect, Effect::Irreversible(_)) {
                return None;
            }
            let (_, command) = crate::extension_bridge::agent_call::split_path(entry.path);
            let has_description = super::super::catalogue::CATALOGUE
                .iter()
                .any(|e| e.command == command && !e.description.is_empty());
            (!has_description).then_some(command)
        })
        .collect();
    actual.sort_unstable();
    let mut expected = EXPECTED_IRREVERSIBLE_NO_DESCRIPTION.to_vec();
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "the set of Irreversible commands with no catalogue description drifted from this \
         test's own hand-written list — if a NEW Irreversible command legitimately has no \
         description yet, add TSDoc to its IPC contract member instead of adding it here; if \
         one DROPPED OUT (now described), remove it from EXPECTED_IRREVERSIBLE_NO_DESCRIPTION"
    );
}

// --- TR-03 MEDIUM (test-author round) -------------------------------------------------------
//
// Nothing before this pinned the generated `CATALOGUE` against the Rust `#[tauri::command]`
// handler signatures `agent_call::validate::check_input` gates dispatch on. `CATALOGUE` is
// generated from `apps/desktop/src/tauri-client/namespaces/**/*.ts` — a DIFFERENT source of
// truth from the handler being gated. Every other test in this file pins MEMBERSHIP
// (`policy_table_matches_generate_handler_exactly`) or wrapper CLASS
// (`EXPECTED_RESOLVED_WRAPPER_ARGS`/`EXPECTED_UNRESOLVED_WRAPPER_ARGS`); none compares an
// argument's NAME or `required` flag against the handler that actually reads it. Drift there
// silently converts a valid agent call into `invalid_input`, or drops a required-key refusal
// (an `Option<T>` field the catalogue wrongly marks `required: true` would refuse a perfectly
// valid omitted key; the reverse would let a truly-required key through as `None`, panicking or
// misbehaving deeper in the handler).

/// One handler's own non-injected parameter, hand-parsed from its `#[tauri::command]` signature
/// — name already converted to Tauri's wire convention (snake_case Rust ident -> camelCase JSON
/// key) so it compares directly against a [`CatalogueArg::name`].
struct HandlerParam {
    name: String,
    required: bool,
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// `snake_case` -> `camelCase`, matching Tauri's own default wire-key convention (the same
/// conversion `gen-agent-catalogue.ts` relies on when it reads the TS side of the same contract).
fn snake_to_camel(s: &str) -> String {
    let mut out = String::new();
    let mut upper_next = false;
    for c in s.chars() {
        if c == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(c.to_uppercase());
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Strip `//` line comments from a parameter list — a handler that documents one param inline
/// (e.g. `ai_lookup_salary`'s `country`/`currency`/`effort`) would otherwise have its comment
/// TEXT treated as literal parameter source, corrupting the top-level comma split below (a
/// comment's own prose commas would be read as param separators).
fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|line| line.find("//").map_or(line, |i| &line[..i]))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Split a parameter list on top-level commas only — a comma nested inside `<...>` (a generic
/// like `Option<ScrapeListFilter>` or `tauri::State<'_, T>`) does not end a param.
fn split_top_level_params(src: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, c) in src.char_indices() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&src[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&src[start..]);
    parts
}

/// A Tauri-injected parameter (`AppHandle`/`tauri::State<..>`) carries no wire key at all — not
/// a [`HandlerParam`], the same exemption `entry_for`'s own catalogue never lists one for.
fn is_injected_param_type(ty: &str) -> bool {
    let ty = ty.trim();
    ty.contains("AppHandle") || ty.starts_with("State<") || ty.starts_with("tauri::State<")
}

fn parse_param(raw: &str) -> Option<HandlerParam> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let colon = raw.find(':')?;
    let name = raw[..colon].trim();
    let name = name.strip_prefix("mut ").unwrap_or(name).trim();
    let ty = raw[colon + 1..].trim();
    if is_injected_param_type(ty) {
        return None;
    }
    Some(HandlerParam {
        name: snake_to_camel(name.trim_start_matches('_')),
        required: !ty.starts_with("Option<"),
    })
}

/// Byte-scan `text` for every `#[tauri::command]`/`#[command]`-annotated fn and return its
/// `(name, non-injected params)`. No `syn`/regex dependency (neither is a `[dependencies]` of
/// this crate in this shape) — this mirrors `registered_command_paths`'s own hand-rolled
/// `include_str!` extraction above rather than adding one for a test-only need.
fn command_handler_params(text: &str) -> Vec<(String, Vec<HandlerParam>)> {
    const MARKERS: [&str; 2] = ["#[tauri::command]", "#[command]"];
    let mut results = Vec::new();
    let mut pos = 0usize;
    while pos < text.len() {
        let Some((idx, marker)) = MARKERS
            .iter()
            .filter_map(|m| text[pos..].find(m).map(|i| (pos + i, *m)))
            .min_by_key(|(i, _)| *i)
        else {
            break;
        };
        let after = idx + marker.len();
        let Some(fn_rel) = text[after..].find("fn ") else {
            pos = after;
            continue;
        };
        let between = &text[after..after + fn_rel];
        // Only whitespace/`pub`/`pub(...)`/`async` may sit between the marker and `fn ` — guards
        // against this exact literal marker string ever appearing somewhere unrelated to the fn
        // it is meant to annotate.
        if !between
            .split_whitespace()
            .all(|tok| tok == "pub" || tok == "async" || tok.starts_with("pub("))
        {
            pos = after;
            continue;
        }
        let fn_start = after + fn_rel + "fn ".len();
        let name_end = text[fn_start..]
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .map_or(text.len(), |i| fn_start + i);
        let name = text[fn_start..name_end].to_string();
        let Some(paren_rel) = text[name_end..].find('(') else {
            pos = name_end;
            continue;
        };
        let paren_start = name_end + paren_rel;
        let bytes = text.as_bytes();
        let mut depth = 0i32;
        let mut close = paren_start;
        for (i, &b) in bytes[paren_start..].iter().enumerate() {
            match b {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = paren_start + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let params_src = strip_line_comments(&text[paren_start + 1..close]);
        let params = split_top_level_params(&params_src)
            .into_iter()
            .filter_map(parse_param)
            .collect();
        results.push((name, params));
        pos = close + 1;
    }
    results
}

#[test]
fn catalogue_arg_names_and_required_flags_match_every_command_handler_signature() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs_files(&src_dir, &mut files);

    let mut handlers: HashMap<String, Vec<HandlerParam>> = HashMap::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for (name, params) in command_handler_params(&text) {
            handlers.insert(name, params);
        }
    }
    assert!(
        handlers.len() >= 167,
        "expected to find at least the 167 #[tauri::command]/#[command] handlers this crate \
         registers, found {} — the byte-scan above likely drifted from the real signature shape \
         (fix the scan, don't loosen this bound)",
        handlers.len()
    );

    let mut mismatches = Vec::new();
    for entry in super::super::catalogue::CATALOGUE.iter() {
        let Some(params) = handlers.get(entry.command) else {
            mismatches.push(format!(
                "{}: catalogued but no #[tauri::command] handler found by this scan",
                entry.command
            ));
            continue;
        };
        let catalogue_names: HashSet<&str> = entry.args.iter().map(|a| a.name).collect();
        let handler_names: HashSet<&str> = params.iter().map(|p| p.name.as_str()).collect();
        if catalogue_names != handler_names {
            mismatches.push(format!(
                "{}: catalogue args {catalogue_names:?} != handler params {handler_names:?}",
                entry.command
            ));
            continue;
        }
        for arg in entry.args {
            let handler = params
                .iter()
                .find(|p| p.name == arg.name)
                .expect("checked above");
            if handler.required != arg.required {
                mismatches.push(format!(
                    "{}.{}: catalogue required={} but the handler param is {}",
                    entry.command,
                    arg.name,
                    arg.required,
                    if handler.required {
                        "non-Option"
                    } else {
                        "Option<..>"
                    }
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "CATALOGUE drifted from its Rust #[tauri::command] handler signature:\n{}",
        mismatches.join("\n")
    );
}
