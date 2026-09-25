//! The rest of `commands/ai`: embeddings, provider credentials,
//! and active-model configuration.
//!
//! One contiguous shard of `POLICY`, split out under R8's LOC cap and
//! concatenated back in `lib.rs`'s `generate_handler!` order by the parent.

use super::*;

pub(super) const AI_EMBEDDINGS_AND_CONFIG: &[PolicyEntry] = &[
    // ADR-038 §4 revision, reclassified AGAIN (HIGH fix — security review
    // round 2): the prior revision here reclassified this row `NotExposed`
    // because `documents::embed` → `embed_text` hit a PAID embedding
    // provider with NO `charge_provider_daily`/`limiter.acquire` gate at
    // all. That gate landed (`admit_embed`, this file's own body — see
    // `ai_embed`'s doc comment there), and its own STATUS note said this row
    // would then flip to `Read`. It does NOT: `charge_provider_daily`
    // spending against a paid provider's per-day ceiling is this table's own
    // named Irreversible trigger (module doc, "ADR-038 itself names four
    // canonical patterns" paragraph) — applied to every OTHER command that
    // charges it (`ai_generate`, `ai_research_company`, `ai_reembed_all`,
    // and `autopilot_run`'s own per-embed charge via `RerankBudget`, all
    // Irreversible below). Flipping this ONE row to `Read` instead would
    // make it the sole freely-dispatchable paid-provider-spend command in
    // the table — the exact inconsistency Finding 5 (this same review round)
    // flags for `match_resume`/`match_resume_text`. Same WEAK spend-total
    // proof as every other ungapped AI-spend row: no id-scoped record exists
    // to prove against (the request is a bare text blob), so the caller's
    // own today-so-far spend is the strongest available signal.
    PolicyEntry {
        path: "commands::ai::ai_embed",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        }),
    },
    // Writes a secret into the OS keychain — ADR-038's `credentials:*` pattern.
    // No prior key to name (a fresh set has nothing to compare against); the
    // strongest available signal is whether a key for this SAME provider
    // already exists — WEAK (boolean, low entropy), flagged.
    PolicyEntry {
        path: "commands::ai::ai_set_provider_key",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "ai_has_provider_key",
            key: "provider",
            input: LookupInput::FromCaller(&["provider"]),
            path: &["has"],
        }),
    },
    // Removes a secret from the OS keychain — `credentials:*`. Same WEAK
    // boolean proof as `ai_set_provider_key` (a real removal, `has` should
    // read `true` beforehand — but that's a coin flip's worth of entropy).
    PolicyEntry {
        path: "commands::ai::ai_remove_provider_key",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "ai_has_provider_key",
            key: "provider",
            input: LookupInput::FromCaller(&["provider"]),
            path: &["has"],
        }),
    },
    PolicyEntry { path: "commands::ai::ai_has_provider_key", effect: Effect::Read },
    // CRITICAL fix (security review round 3), reclassified from `Read`: the
    // prior "only probes the provider; no store write" comment audited the
    // PERSISTENCE axis only. Both commands take a caller-supplied
    // `base_url: Option<String>` VERBATIM and pass it straight through
    // `resolve_by_name` (`commands::ai_provider::mod.rs`), which honors it
    // for real egress on `openai-compatible`. `validate_provider_base_url`
    // (`net/ssrf.rs`) is a PROVENANCE guard against an XSS'd renderer
    // (blocks only a non-http(s) scheme and the cloud-metadata literal),
    // never a destination allowlist — its whole premise (ADR-0012) is that
    // the renderer never lets a THIRD PARTY choose this value for this
    // call. `agent call` breaks that premise outright: a caller names any
    // `https://attacker.example`, and both commands read the stored
    // provider API key out of the OS keychain and send it there —
    // `ai_test_provider_key` via `OpenAiClient::test_key`,
    // `ai_list_provider_models` via `OpenAiClient::list_models` — with no
    // confirm, and the attacker's own response body reaches
    // `friendly_api_error`'s `detail` and stdout UNFENCED on a non-2xx (an
    // injection channel needing no job board at all). Dropping `base_url`
    // server-side instead would also break the renderer's legitimate "test
    // this unsaved endpoint before you save it" flow — the reason these two
    // commands accept a caller-supplied `base_url` in the first place — a
    // change this table has no reach to make safely without touching the
    // real command; `NotExposed` is the fix that costs that flow nothing.
    PolicyEntry {
        path: "commands::ai::ai_test_provider_key",
        effect: Effect::NotExposed(
            "takes a caller-supplied base_url verbatim and, for openai-compatible, sends the \
             stored provider API key to it (OpenAiClient::test_key) — validate_provider_base_url \
             is a provenance guard against an XSS'd renderer, not a destination allowlist, and \
             agent_call has no renderer-equivalent provenance to rely on",
        ),
    },
    PolicyEntry {
        path: "commands::ai::ai_list_provider_models",
        effect: Effect::NotExposed(
            "same caller-supplied base_url egress as ai_test_provider_key (OpenAiClient::list_models \
             also sends the stored provider API key to it) — see that row's own comment",
        ),
    },
    PolicyEntry { path: "commands::ai::ai_embedding_status", effect: Effect::Read },
    // CRITICAL fix (security review round 4), reclassified from
    // `Reversible`: this row's prior comment audited only whether the
    // command PERSISTS anything undoable (it does clear only recomputable
    // derived caches on a real space change — that part is still true) and
    // missed the Round 3 addendum's own second axis entirely: `base_url:
    // Option<String>` is caller-supplied and, for `openai-compatible`, is
    // scrubbed/checked by `scrub_and_validate_embedding_base_url`
    // (`commands::ai::mod`) — the SAME `net::ssrf::validate_provider_base_url`
    // PROVENANCE guard (not a destination allowlist) `ai_test_provider_key`'s
    // row above is reclassified for. Unlike that row, the result here is
    // PERSISTED into `EmbeddingConfig` (`DocumentStore`), and
    // `documents::embedding::embed` reads it back on EVERY subsequent embed
    // call — résumé text, job-ad bodies, autopilot's re-rank pass — attaching
    // the stored provider API key via `bearer_auth` to whatever host this one
    // unconfirmed call named (`commands::ai_provider::openai::embed_impl`).
    // So one `agent call` redirects the keychain key AND every future
    // embedded document to an attacker host, silently, fired by ordinary
    // subsequent user activity (upload auto-index, match scoring,
    // autopilot) — worse than `ai_test_provider_key`'s one-shot probe: this
    // one persists. `NotExposed`, same reason shape as those two rows.
    PolicyEntry {
        path: "commands::ai::ai_set_embedding_config",
        effect: Effect::NotExposed(
            "takes a caller-supplied base_url verbatim and, for openai-compatible, persists it \
             into EmbeddingConfig after only a provenance check (scrub_and_validate_embedding_base_url \
             / validate_provider_base_url, not a destination allowlist) — every embed call afterward \
             (documents::embedding::embed) reads the stored config back and sends the provider API \
             key to that host, so one call redirects every subsequent embedded document, not just \
             this one call; see ai_test_provider_key's row for the same provenance-guard argument \
             applied to a one-shot (not persisted) probe",
        ),
    },
    // Bulk re-embeds every document; charges the daily provider ceiling per
    // document. Global (no id), WEAK spend-total fallback, flagged.
    PolicyEntry {
        path: "commands::ai::ai_reembed_all",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        }),
    },
    // Same charged re-embed path, scoped to stale documents. Same WEAK
    // spend-total fallback.
    PolicyEntry {
        path: "commands::ai::ai_index_stale_documents",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        }),
    },
    PolicyEntry { path: "commands::ai::ai_spend_summary", effect: Effect::Read },
    PolicyEntry { path: "commands::ai::ai_active_config", effect: Effect::Read },
    // HIGH fix (security review round 3), reclassified from `Reversible`:
    // the CONFIG ROW is undoable (edit it back), but that is not the effect
    // that matters — every generation the USER runs afterward through the
    // renderer ships résumé text, contact PII and full job ads to WHATEVER
    // host is stored, with no per-generation re-confirmation and nothing
    // logged beyond the command name. `validate_settings` (`ai_config/
    // mod.rs`) accepts any http(s) host for `openai-compatible`'s
    // `base_url` — the SAME provenance-only guard `ai_test_provider_key`'s
    // row above explains. No per-record proof exists — this command takes
    // no id, no `base_url` of its own — so the proof used here is the
    // CURRENT value of the very field this call overwrites (`activeProvider`,
    // read fresh via `ai_active_config` immediately before the flip): a
    // legitimate binding, unlike a value unrelated to the write. WEAK all
    // the same (it doesn't name which provider/host is about to become
    // active, only that the caller looked at what's active NOW), flagged
    // like every other no-id row in this table.
    //
    // Round 3 also moved `ai_set_provider_settings` here, reasoning "two
    // free `agent call`s install a silent egress redirect... so both rows
    // move together" — WRONG (security review round 4): `ai_set_provider_
    // settings` takes its OWN caller-chosen `provider` field, independent of
    // `activeProvider`, so binding its ceremony to `ai_active_config` proves
    // nothing about which provider its patch actually targets. That row is
    // `NotExposed` below instead — see its own comment for why no
    // `ProofSource` shape available today can bind it, and why THIS row's
    // proof (the field it directly overwrites) is a different, legitimate
    // shape that does not generalize to it.
    PolicyEntry {
        path: "commands::ai::ai_set_active_provider",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_active_config",
            path: &["activeProvider"],
        }),
    },
    // HIGH fix (security review round 4), reclassified from `Irreversible`
    // AGAIN: the round-3 revision above bound this row's proof to
    // `ai_active_config`'s `activeProvider`, but `ProviderSettingsPatch.
    // provider` (`ai_config/mod.rs`) is its OWN caller-chosen field,
    // independent of which provider is active — `set_provider_settings`
    // rewrites `base_url` on ANY named provider, while the confirmed
    // `activeProvider` field never changes. So: read `ai_active_config`,
    // confirm with the active provider's own name (a real, freely-available
    // value), then redirect a DIFFERENT provider's egress in the same call —
    // one extra free read and the ceremony is satisfied while the actual
    // target it never checked gets rewritten. This is clause 2 of the module
    // doc's NotExposed rule (zero relationship between the confirmed value
    // and the actual target) — the exact shape that demoted
    // `support_export_diagnostics`, applied here to the greater harm rather
    // than the lesser one it was applied to there. A BOUND proof is not
    // expressible with today's `ProofSource` vocabulary either:
    // `ActiveAiConfig.providers` is a `BTreeMap<String, ProviderConfig>`, and
    // `Scalar`/`Lookup` both take a `&'static [&'static str]` path, which
    // cannot embed a caller-supplied provider name at compile time to reach
    // `["providers", <that provider>, "baseUrl"]`. `ai_set_active_provider`
    // is DELIBERATELY NOT reclassified alongside this row — its proof is the
    // CURRENT value of the very field it overwrites (`activeProvider` itself),
    // a legitimate binding this row never had.
    PolicyEntry {
        path: "commands::ai::ai_set_provider_settings",
        effect: Effect::NotExposed(
            "ProviderSettingsPatch.provider is caller-chosen and independent of the confirmed \
             activeProvider — the ceremony can be satisfied by reading the active provider's name \
             while the patch rewrites a DIFFERENT provider's base_url; zero relationship between \
             the confirmed value and the actual target (the support_export_diagnostics clause), \
             and ActiveAiConfig.providers is a BTreeMap so no &'static ProofSource path can bind \
             to the caller's chosen provider — see ai_set_active_provider's row for the proof \
             shape that DOES bind and is why that row alone stays Irreversible",
        ),
    },
    // CRITICAL fix (security review round 4 — found by the same re-derive-
    // from-signature sweep that caught `ai_set_embedding_config`, not named
    // by the reviewer), reclassified from `Reversible`: the prior comment
    // ("One-time seed, row-presence gated") audited only whether the write
    // is undoable and, same trap as `ai_set_embedding_config`, read as a
    // completed audit. `config: AiConfigSnapshot` carries a caller-supplied
    // `providers: BTreeMap<String, ProviderConfig>` (each with its own
    // `base_url`) AND `active_provider` — `seed_if_empty` →
    // `apply_snapshot_conn` runs `base_url` through `scrub_settings`, the
    // SAME provenance-only `net::ssrf::validate_provider_base_url` guard
    // `ai_set_embedding_config`'s row explains, then persists it for
    // `openai-compatible` AND sets `active_provider` from the same caller
    // input in the SAME call (`ai_config/mod.rs::apply_snapshot_conn`) — one
    // call plants the redirect and flips routing onto it together, the
    // exact two-part shape round 3 flagged for `ai_set_provider_settings` +
    // `ai_set_active_provider`. The row-presence gate (`is_seeded_conn`)
    // narrows this to a WINDOW rather than removing it: on a genuinely
    // fresh install — no active provider, no provider row, no stage
    // override ever set — this call is NOT a no-op, and a CLI-paired agent
    // reaches the bridge before a human necessarily opens Settings.
    // `NotExposed`, same reason shape as `ai_set_embedding_config`; also not
    // really a CLI-facing capability in the first place — its own doc
    // describes it as the renderer's first-run Zustand migration, not
    // something an agent has any legitimate reason to drive.
    PolicyEntry {
        path: "commands::ai::ai_seed_active_config",
        effect: Effect::NotExposed(
            "config.providers carries a caller-supplied base_url per provider AND \
             config.activeProvider, both applied together on a fresh (never-configured) store — \
             scrub_settings runs the same provenance-only validate_provider_base_url guard \
             ai_set_embedding_config's row explains, then persists+activates the redirect in one \
             call; the row-presence gate only narrows this to the first-run window, it does not \
             close it, and this is a renderer migration path with no legitimate CLI use anyway",
        ),
    },
    PolicyEntry { path: "commands::ai::ai_stage_overrides", effect: Effect::Read },
    PolicyEntry { path: "commands::ai::ai_set_stage_override", effect: Effect::Reversible },
    PolicyEntry { path: "commands::ai::ai_clear_stage_override", effect: Effect::Reversible },

];
