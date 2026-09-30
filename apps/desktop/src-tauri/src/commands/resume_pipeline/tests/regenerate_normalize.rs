use crate::pipeline::resume::types::SectionKey;

const REGEN_SOURCE: &str =
    "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\nEDUCATION\n\nnothing";

/// **Scoped to a Projects regeneration.** Regenerating any OTHER section must
/// leave the spliced text untouched — normalizing there would silently
/// revert whatever the user had manually edited into the Projects section.
/// Not `key`'s to attempt at all, so there is no reason to report either.
///
/// Mutation check: drop the `key != SectionKey::Projects` arm and this fails
/// (the Skills-section spliced text gets rewritten as if it were Projects).
#[test]
fn normalize_regenerated_projects_is_scoped_to_the_projects_key() {
    let spliced = "SKILLS\n\nGo, Rust".to_string();
    let (out, reason) = super::super::normalize_regenerated_projects(
        SectionKey::Skills,
        REGEN_SOURCE,
        true,
        spliced.clone(),
    );
    assert_eq!(out, spliced, "a non-Projects regenerate is a pure no-op");
    assert_eq!(reason, None);
}

/// **Gated on real provenance.** When `source_resume_for` fell back to the
/// run's own generated text (`sourceResumeId` missing/deleted), normalizing
/// would seed the "restore" from the document about to be overwritten —
/// codifying whatever the last generation said as fact. The decline is
/// still OBSERVABLE (CodeRabbit #990 finding 1) — a silent skip on a
/// user-triggered regenerate would be unobservable while the draft-stage
/// ledger reports the same reasons for a run.
///
/// Mutation check: drop the `!source_is_provenanced` arm and this fails (the
/// altered link gets "restored" from the fallback text instead of left
/// alone).
#[test]
fn normalize_regenerated_projects_refuses_an_unprovenanced_source() {
    let spliced =
        "PROJECTS\n\n**Ledger CLI** · https://an-altered-fork.example.com/ledger\n".to_string();
    let (out, reason) = super::super::normalize_regenerated_projects(
        SectionKey::Projects,
        REGEN_SOURCE,
        false, // fallback text, not a real provenance hit
        spliced.clone(),
    );
    assert_eq!(
        out, spliced,
        "an unprovenanced source must not drive a write"
    );
    assert_eq!(reason, Some("unprovenanced_source"));
}

/// **The happy path: a Projects regenerate over a real-provenance source DOES
/// normalize** — an altered link is restored, proving the wiring (not just
/// the scoping/gating guards) actually runs, and reports no skip reason.
#[test]
fn normalize_regenerated_projects_normalizes_a_provenanced_projects_regenerate() {
    let spliced =
        "PROJECTS\n\n**Ledger CLI** · https://an-altered-fork.example.com/ledger\n".to_string();
    let (out, reason) = super::super::normalize_regenerated_projects(
        SectionKey::Projects,
        REGEN_SOURCE,
        true,
        spliced,
    );
    assert!(
        out.contains("https://github.com/janedoe/ledger"),
        "the source's own link must be restored: {out}"
    );
    assert!(!out.contains("an-altered-fork"));
    assert_eq!(reason, None);
}

/// **A declined normalization on a Projects regenerate is observable** — the
/// same `link_in_description` shape `seed_projects_for_normalize` bails on
/// for a run's draft stage, now surfaced through the regenerate command too.
#[test]
fn normalize_regenerated_projects_reports_the_seed_skip_reason() {
    let mega_source = "PROJECTS\n\n\
        Ledger CLI\n\
        https://github.com/janedoe/ledger\n\
        Beta Sync · https://github.com/janedoe/beta\n\
        Go · gRPC\n";
    let spliced = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n".to_string();
    let (out, reason) = super::super::normalize_regenerated_projects(
        SectionKey::Projects,
        mega_source,
        true,
        spliced.clone(),
    );
    assert_eq!(
        out, spliced,
        "a declined normalization leaves the text untouched"
    );
    assert_eq!(reason, Some("link_in_description"));
}
