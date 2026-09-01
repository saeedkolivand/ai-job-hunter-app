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
#[test]
fn policy_table_has_exactly_164_rows() {
    assert_eq!(POLICY.len(), 164);
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

/// HIGH fix (security review round 2): `match_resume`/`match_resume_text`
/// reach a paid embedding provider (`score_one` → `embed_charged`) with
/// `budget: None` when `semanticScoringEnabled: true` — the SAME
/// uncapped-spend shape that forced `ai_embed` `NotExposed` before ITS
/// gate landed. Hand-written (not looped, mirroring
/// `policy_table_has_exactly_164_rows`'s own discipline): a revert of
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
    assert_eq!(found.len(), 164);
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
    // `policy_table_has_exactly_164_rows`): 31 Irreversible rows
    // (`extension_bridge_regenerate_token` moved to `NotExposed` —
    // security review round 1; `ai_embed` moved NotExposed → Irreversible
    // once its `charge_provider_daily` gate landed, and
    // `match_resume`/`match_resume_text` moved Reversible → NotExposed
    // for the SAME reason `ai_embed` originally was — security review
    // round 2; security review round 3 nets +1: `ai_set_active_provider`
    // and `ai_set_provider_settings` moved Reversible → Irreversible
    // [+2], `support_export_diagnostics` moved Irreversible →
    // `NotExposed` for a vacuous proof [-1]; security review round 4 nets
    // -1: `ai_set_provider_settings` moved Irreversible → `NotExposed` —
    // its proof was bound to `activeProvider` while its own patch targets
    // a DIFFERENT, caller-chosen `provider` field entirely, so the
    // ceremony never checked the thing it was rewriting — see each row's
    // own comment).
    assert_eq!(checked, 31, "expected exactly 31 Irreversible rows");
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

/// Mutation-style guard: an `Irreversible` row whose `ProofSource`
/// pointed at ITSELF (or at another `Irreversible` row) would make the
/// ceremony circular — satisfiable without ever reading anything real.
#[test]
fn no_proof_source_points_at_an_irreversible_command() {
    for entry in POLICY {
        if let Effect::Irreversible(source) = entry.effect {
            let (_, own_command) = entry.path.rsplit_once("::").unwrap_or(("", entry.path));
            assert_ne!(
                source.read_command(),
                own_command,
                "{} names itself as its own proof source",
                entry.path
            );
        }
    }
}
