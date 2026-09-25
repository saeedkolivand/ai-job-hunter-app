//! The verb vocabulary: the [`Verb`] this CLI parses argv into and the
//! [`VERB_TABLE`] its `--help` text and every "unknown verb" error are generated from — one
//! array, never a second hand-typed copy. Split out of `agent_cli.rs` under R8's LOC cap.

use super::*;
// ── argv → verb ─────────────────────────────────────────────────────────────

// No `Eq` (issue #1167 — `min_score: Option<f64>` can't implement it; every
// test comparison below only ever needs `PartialEq`).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Verb {
    BestMatches {
        limit: Option<u64>,
        /// Opaque to this client — passed through verbatim, never parsed
        /// here. Issue #1146 P11 introduced it as a plain numeric offset;
        /// round 2 (B3-r1-F4) folded `query`'s fingerprint into an
        /// `<issuer>:<offset>` grammar instead, once `query` (below) started
        /// changing which rows a traversal contains — the SAME per-list
        /// issuer ambiguity `found-jobs`' own cursor exists to close, this
        /// resource is no longer exempt from. See
        /// `agent_read::best_matches::best_matches_cursor_issuer`/`parse_best_matches_cursor`.
        cursor: Option<String>,
        /// Case-insensitive substring over title or company (issue #1168).
        query: Option<String>,
    },
    Job {
        url: String,
    },
    Profile,
    Automations,
    Schema,
    /// Paginated, filtered traversal of the stored found-jobs list (issue
    /// #1115), one or every autopilot at once (issue #1168) —
    /// `autopilot_get`/`autopilot_list`/`autopilot_best_matches` cannot
    /// enumerate this: the first two are unbounded (every real autopilot
    /// exceeds the MCP bridge's own result cap) and the third is a
    /// cross-autopilot top-N ranking, not a full traversal. `cursor` is
    /// opaque to this client — it is passed through verbatim in both
    /// directions and never parsed here; its shape, and the fact that a
    /// cursor is only valid for the same `autopilotId` scope (present or
    /// omitted) that issued it (issue #1130), live on
    /// `agent_read::found_jobs::parse_found_jobs_cursor`.
    FoundJobs {
        /// Optional (issue #1168) — omitted, the traversal spans every
        /// autopilot, deduped by posting identity.
        autopilot_id: Option<String>,
        limit: Option<u64>,
        cursor: Option<String>,
        /// Server-side filters (issue #1167) — the app's own predicates for
        /// each, never a fresh matcher invented on this surface (see
        /// `agent_read::found_jobs::FoundJobsFilters`'s own doc).
        min_score: Option<f64>,
        country: Option<String>,
        remote: Option<bool>,
        applied: Option<bool>,
        query: Option<String>,
        include_description: bool,
    },
    /// ADR-038 §2's generic dispatch tier (`agent call <namespace>:<command>
    /// [--input '<json>'] [--confirm '<value>']`) — a SEPARATE wire frame
    /// (`agent.call`, never `agent.query`) and reply shape (`dispatched`,
    /// never `ok`); see [`Verb::wire_type`]/[`Verb::reply_type`] and
    /// `run_verb`'s own dispatched-vs-ok branch below. `confirm` is the
    /// Phase 3 ceremony's proof value for an `Effect::Irreversible` command
    /// — `None` for every other row, and never logged/echoed anywhere on
    /// this client (see `parse_call`'s own doc).
    Call {
        namespace: String,
        command: String,
        input: Value,
        confirm: Option<String>,
    },
}

impl Verb {
    pub(super) fn resource_name(&self) -> &'static str {
        match self {
            Verb::BestMatches { .. } => "best-matches",
            Verb::Job { .. } => "job",
            Verb::Profile => "profile",
            Verb::Automations => "automations",
            Verb::Schema => "schema",
            Verb::FoundJobs { .. } => "found-jobs",
            Verb::Call { .. } => "call",
        }
    }

    /// The outbound frame's `type` — every curated verb sends `agent.query`;
    /// [`Verb::Call`] sends the generic tier's own `agent.call` instead (two
    /// visibly different grammars, ADR-038 §2's own framing).
    pub(super) fn wire_type(&self) -> &'static str {
        match self {
            Verb::Call { .. } => msg::AGENT_CALL,
            _ => msg::AGENT_QUERY,
        }
    }

    /// The expected reply frame's `type` — the mirror of [`Self::wire_type`].
    pub(super) fn reply_type(&self) -> &'static str {
        match self {
            Verb::Call { .. } => msg::AGENT_CALL_RESULT,
            _ => msg::AGENT_RESULT,
        }
    }

    /// The outbound frame's `payload` object for this verb.
    pub(super) fn payload(&self) -> Value {
        match self {
            Verb::BestMatches {
                limit,
                cursor,
                query,
            } => {
                let mut p = json!({ "resource": self.resource_name() });
                if let Some(limit) = limit {
                    p["limit"] = json!(limit);
                }
                if let Some(cursor) = cursor {
                    p["cursor"] = json!(cursor);
                }
                if let Some(query) = query {
                    p["query"] = json!(query);
                }
                p
            }
            Verb::Job { url } => json!({ "resource": self.resource_name(), "url": url }),
            Verb::Profile | Verb::Automations | Verb::Schema => {
                json!({ "resource": self.resource_name() })
            }
            Verb::FoundJobs {
                autopilot_id,
                limit,
                cursor,
                min_score,
                country,
                remote,
                applied,
                query,
                include_description,
            } => {
                let mut p = json!({ "resource": self.resource_name() });
                if let Some(autopilot_id) = autopilot_id {
                    p["autopilotId"] = json!(autopilot_id);
                }
                if let Some(limit) = limit {
                    p["limit"] = json!(limit);
                }
                if let Some(cursor) = cursor {
                    p["cursor"] = json!(cursor);
                }
                if let Some(min_score) = min_score {
                    p["minScore"] = json!(min_score);
                }
                if let Some(country) = country {
                    p["country"] = json!(country);
                }
                if let Some(remote) = remote {
                    p["remote"] = json!(remote);
                }
                if let Some(applied) = applied {
                    p["applied"] = json!(applied);
                }
                if let Some(query) = query {
                    p["query"] = json!(query);
                }
                if *include_description {
                    p["includeDescription"] = json!(true);
                }
                p
            }
            Verb::Call {
                namespace,
                command,
                input,
                confirm,
            } => {
                let mut p = json!({ "namespace": namespace, "command": command, "input": input });
                if let Some(confirm) = confirm {
                    p["confirm"] = json!(confirm);
                }
                p
            }
        }
    }
}

/// One verb's `--help` metadata. [`VERB_TABLE`] is `parse_verb`'s own
/// canonical name list (never a second hand-typed one — see [`super::entrypoint::help_text`]
/// and `parse_verb`'s "unknown verb" branch below, both of which read from
/// this SAME array) so the CLI's usage text cannot silently drift from what
/// it actually parses (LOW fix — security review, folded into this same
/// verb table so it can't recur here either).
pub(super) struct VerbHelp {
    pub(super) name: &'static str,
    pub(super) args: &'static str,
    pub(super) returns: &'static str,
}

pub(super) const VERB_TABLE: &[VerbHelp] = &[
    VerbHelp {
        name: "best-matches",
        args: "[--limit <n>] [--cursor <c>] [--query <q>]",
        returns: "the strongest jobs across every autopilot (default 20, max 100 per page); \
                  repeat with the returned `nextCursor` to reach every ranked row; `--query` \
                  filters to a title/company substring over the already-capped, ranked \
                  candidate list this call computes (NOT the full stored corpus — use \
                  `found-jobs --query` to search every stored posting), and the cursor is only \
                  valid for the SAME `--query` (present or omitted) that issued it; `total` is \
                  the size of this capped ranked list, not the number of qualifying postings in \
                  storage — use `found-jobs` for a true corpus count",
    },
    VerbHelp {
        name: "job",
        args: "<url>",
        // The url-spelling sentence is the caller-facing half of `agent_read::job_lookup_key`'s
        // doc (MEDIUM fix, security review round 4): this READ is deliberately lenient about
        // percent-escapes, the write commands are not, so the spelling a reply hands back is the
        // one that works on both.
        returns: "full detail for one posting, matched by its posting url ONLY — never by title \
                  or company (use `found-jobs --query` for that); pass back the `url` a reply \
                  gave you rather than re-encoding your own — write commands match the exact \
                  spelling. `applied` is OMITTED (never a confident false) when the applications \
                  store is unreadable; the reply then carries `appliedUnavailable: true`",
    },
    VerbHelp {
        name: "profile",
        args: "",
        returns:
            "contact-profile fields for autofill (same consent gate as the extension's profile.get)",
    },
    VerbHelp {
        name: "automations",
        args: "",
        // Issue #1132 — `totalFound` is the LAST run's kept count, while `foundJobsTotal` is the
        // whole stored list and equals `found-jobs`' own `total`. This string is what `--help`
        // prints AND what the `automations` MCP tool's description is derived from, but it is NOT
        // the only place the distinction is written (MEDIUM fix, review round 4 — the old comment
        // said "named HERE", which reads as "one surface"): `agent schema` serves
        // `agent_read::RESOURCES`' own copy. The two are pinned together by
        // `tests::both_automations_descriptions_name_both_totals`, so neither can drop a field
        // the other still explains.
        returns: "every autopilot and its status (`totalFound` is the last run's kept count; \
                  `foundJobsTotal` is the whole stored list `found-jobs` pages through)",
    },
    VerbHelp {
        name: "schema",
        args: "",
        returns: "this resource list, as machine-readable JSON",
    },
    VerbHelp {
        name: "found-jobs",
        args: "[<autopilotId>] [--limit <n>] [--cursor <c>] [--min-score <n>] [--country <s>] \
               [--remote <bool>] [--applied <bool>] [--query <q>] [--include-description]",
        returns: "one page of the stored found-jobs list; every reply carries `total` — the \
                  filtered row count this call matches, so a count never needs a full traversal. \
                  `<autopilotId>` is optional: given, scopes to one autopilot; omitted, spans \
                  every autopilot (deduped by posting identity). Rows are compact (no \
                  description) unless --include-description is set. Repeat with the returned \
                  cursor until it comes back null to traverse the whole (filtered) list — the \
                  cursor is only valid for the SAME autopilotId scope AND the same filter \
                  arguments that issued it; default/max limit are documented on \
                  `agent_read::found_jobs::resolve_found_jobs`. Each row's `applied` is OMITTED \
                  (never a confident false) when the applications store is unreadable; the reply \
                  then carries `appliedUnavailable: true` and `--applied` is refused",
    },
    VerbHelp {
        name: "call",
        args: "<namespace>:<command> [--input '<json>'] [--confirm '<value>']",
        returns: "ADR-038 §2's generic dispatch tier — Read/Reversible commands dispatch \
                  directly; an Irreversible command needs --confirm '<value>' (a proof read \
                  from ANOTHER command, named but never disclosed by a --confirm-less call — \
                  exit 4); NotExposed always refuses (see `agent schema`, the MCP `commands` \
                  tool, or policy.rs for the full table). A few unbounded list commands answer \
                  with a paged {items,total,nextCursor} envelope and take --input \
                  '{\"limit\":N,\"cursor\":\"...\"}'; the `commands` tool marks which and how. \
                  `agent call contact_profile:contact_profile_get` omits `photo`; a `contact_profile_set` \
                  write missing it restores the field (`autofill_profile::CONTACT_PROFILE_AGENT_FIELDS`)",
    },
];

pub(super) fn verb_names_joined() -> String {
    VERB_TABLE
        .iter()
        .map(|v| v.name)
        .collect::<Vec<_>>()
        .join("|")
}

#[cfg(test)]
mod tests;
