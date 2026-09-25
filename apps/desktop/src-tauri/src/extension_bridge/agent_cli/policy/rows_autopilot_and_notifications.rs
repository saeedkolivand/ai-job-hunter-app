//! Autopilot, generated artefacts, applications, notifications,
//! referrals, profile import and GitHub.
//!
//! One contiguous shard of `POLICY`, split out under R8's LOC cap and
//! concatenated back in `lib.rs`'s `generate_handler!` order by the parent.

use super::*;

pub(super) const AUTOPILOT_AND_NOTIFICATIONS: &[PolicyEntry] = &[
    // commands/autopilot.rs
    PolicyEntry {
        path: "commands::autopilot::autopilot_list",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::autopilot::autopilot_get",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::autopilot::autopilot_create",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::autopilot::autopilot_update",
        effect: Effect::Reversible,
    },
    // Deletes an autopilot record (and orphans its résumé-derived cache
    // rows). Proof is the target automation's own `name`, read via
    // `autopilot_get` by the SAME `autopilotId`.
    PolicyEntry {
        path: "commands::autopilot::autopilot_remove",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "autopilot_get",
            key: "autopilotId",
            input: LookupInput::FromCaller(&["autopilotId"]),
            path: &["name"],
        }),
    },
    // "Autopilot is a discovery agent... a run only finds, ranks and saves
    // results" (verified — no application submission), BUT the semantic
    // re-rank phase charges `charge_provider_daily` per embed when enabled
    // (`autopilot/rerank.rs::charge_one_embed`) — real, if capped, spend.
    // Same id-scoped proof as `autopilot_remove`.
    PolicyEntry {
        path: "commands::autopilot::autopilot_run",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "autopilot_get",
            key: "autopilotId",
            input: LookupInput::FromCaller(&["autopilotId"]),
            path: &["name"],
        }),
    },
    PolicyEntry {
        path: "commands::autopilot::autopilot_pause",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::autopilot::autopilot_resume",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::autopilot::autopilot_take_pending_focus",
        effect: Effect::NotExposed(
            "atomically consumes a buffered native window-focus intent meant for the renderer's \
             own window — same reasoning as menu_take_pending",
        ),
    },
    PolicyEntry {
        path: "commands::autopilot::autopilot_best_matches",
        effect: Effect::Read,
    },
    // commands/ai_generations.rs
    PolicyEntry {
        path: "commands::ai_generations::ai_generations_list",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::ai_generations::ai_generations_save",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::ai_generations::ai_generations_update",
        effect: Effect::Reversible,
    },
    // Deletes a generation AND cascades to delete its pipeline run trail
    // (`purge_run_trails`). Proof is the target generation's own `jobTitle`,
    // read via `ai_generations_list`, matched by id.
    PolicyEntry {
        path: "commands::ai_generations::ai_generations_remove",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "ai_generations_list",
            id_field: &["id"],
            match_field: "id",
            value_field: "jobTitle",
        }),
    },
    // Bulk delete by a caller-supplied `ids` array — no single record to
    // name. Proof is the COUNT of those ids that actually exist right now,
    // read via `ai_generations_list` — genuinely computed and scoped to the
    // targeted set, but a count, not a name; weaker than the single-id row
    // above, flagged.
    PolicyEntry {
        path: "commands::ai_generations::ai_generations_remove_bulk",
        effect: Effect::Irreversible(ProofSource::MatchCount {
            read_command: "ai_generations_list",
            ids_field: &["ids"],
            match_field: "id",
        }),
    },
    // commands/applications.rs
    PolicyEntry {
        path: "commands::applications::applications_list",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::applications::applications_get",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::applications::applications_set_status",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::applications::applications_accept_status_event",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::applications::applications_reject_status_event",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::applications::applications_update",
        effect: Effect::Reversible,
    },
    // Deletes an Application and (unless keep_documents) cascades to its
    // child generations. Proof is the target application's own `title`,
    // read via `applications_get` by the SAME `id`.
    PolicyEntry {
        path: "commands::applications::applications_delete",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "applications_get",
            key: "id",
            input: LookupInput::FromCaller(&["id"]),
            path: &["application", "title"],
        }),
    },
    PolicyEntry {
        path: "commands::applications::applications_track",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::applications::applications_save_from_posting",
        effect: Effect::Reversible,
    },
    // commands/notifications.rs
    PolicyEntry {
        path: "commands::notifications::notifications_list",
        effect: Effect::Read,
    },
    // No inverse exists anywhere on this surface: `notifications::mod.rs`
    // has no "mark unread", `commands::notifications` exposes no such
    // command, and the renderer only ever filters on `!n.read` — it never
    // sets `read` back to `false`. Flipping this bit is therefore
    // permanent from the app's own perspective, same as a delete — the
    // module doc's rule for `Effect::Irreversible`. Same shape and same
    // `ProofSource` as `notifications_remove` just below: id-scoped, so the
    // proof is the target notification's own `title`, read via
    // `notifications_list`, matched by the SAME id (issue #1164).
    PolicyEntry {
        path: "commands::notifications::notifications_mark_read",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "notifications_list",
            id_field: &["id"],
            match_field: "id",
            value_field: "title",
        }),
    },
    // Same no-inverse argument as `notifications_mark_read` above, applied
    // to every notification at once — no selector, so this is the
    // selector-less shape `notifications_clear_all` already uses: the proof
    // is the TOTAL notification count, read via `notifications_list`
    // itself — a superset of the blast radius, since `mark_all_read` only
    // flips the unread subset and no Read row exposes that narrower count
    // (issue #1164).
    PolicyEntry {
        path: "commands::notifications::notifications_mark_all_read",
        effect: Effect::Irreversible(ProofSource::Count {
            read_command: "notifications_list",
        }),
    },
    // Proof is the target notification's own `title`, read via
    // `notifications_list`, matched by id.
    PolicyEntry {
        path: "commands::notifications::notifications_remove",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "notifications_list",
            id_field: &["id"],
            match_field: "id",
            value_field: "title",
        }),
    },
    // Unconditional wipe of every notification — no selector. Proof is the
    // exact count about to be lost, read via `notifications_list` itself —
    // precisely scoped, the strongest shape a global wipe can have.
    PolicyEntry {
        path: "commands::notifications::notifications_clear_all",
        effect: Effect::Irreversible(ProofSource::Count {
            read_command: "notifications_list",
        }),
    },
    PolicyEntry {
        path: "commands::notifications::notifications_clicked",
        effect: Effect::NotExposed(
            "focuses the desktop window and emits an event to open the inbox; meaningless for a \
             non-interactive caller with no window to focus",
        ),
    },
    // commands/referrals.rs
    PolicyEntry {
        path: "commands::referrals::referrals_list",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::referrals::referrals_upsert",
        effect: Effect::Reversible,
    },
    // Proof is the target referral's own `companyName`, read via
    // `referrals_list`, matched by id.
    PolicyEntry {
        path: "commands::referrals::referrals_remove",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "referrals_list",
            id_field: &["id"],
            match_field: "id",
            value_field: "companyName",
        }),
    },
    // commands/profile_import.rs
    // Network (fetches the given profile url), but no PAID provider in the
    // chain — `Read` on both axes (no persisted-state change, no un-metered
    // billable spend), unlike `ai_embed` above. VERIFIED (recorded so this
    // doesn't get re-raised every review pass): takes a CALLER-SUPPLIED url
    // as its own argument, so `agent call` makes this an egress primitive
    // reachable from the CLI — the SAME reach the UI already has (a user
    // pastes a profile url there too), not a new one this table opens.
    // `import_from_url`'s own `detect_platform` host-allowlists the fetch to
    // linkedin.com (exact/suffix match on the URL's HOST, never a substring
    // scan — guards the `attacker.example/linkedin.com/...` lookalike and a
    // loopback-egress url), so the caller controls WHICH linkedin.com path
    // is fetched, never an arbitrary destination host.
    PolicyEntry {
        path: "commands::profile_import::profile_import_from_url",
        effect: Effect::Read,
    },
    // commands/github.rs
    // Network, no paid provider in the chain — same `Read` reasoning as
    // `profile_import_from_url` above. VERIFIED, and UNLIKE that row: this
    // DOES take a caller-supplied `input` (a username or github.com url),
    // but `parse_username` extracts+validates it into a bare username (SSRF
    // guard — rejects a metadata-service url, e.g. `169.254.169.254`, and
    // anything not github.com-shaped) before `api_url` builds a request
    // against the FIXED host `api.github.com` — the caller controls a path
    // segment, never the destination host, so this is not an arbitrary-url
    // egress primitive the way `profile_import_from_url` is.
    PolicyEntry {
        path: "commands::github::github_import_repos",
        effect: Effect::Read,
    },
];
