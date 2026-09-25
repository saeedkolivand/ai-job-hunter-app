//! `ProofSource` invariants that hold across the WHOLE table, not one row: every
//! irreversible row's proof command must itself be a real `Read` row, and no proof may
//! point at another irreversible command.

use super::super::*;

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
