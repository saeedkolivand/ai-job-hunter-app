use super::*;
// ── Advertised schema text (issues #1129, #1130, #1132, #1144) ──────────

/// One tool's `inputSchema.properties.<field>.description`.
fn property_description(list: &[Value], tool: &str, field: &str) -> String {
    list.iter()
        .find(|t| t["name"] == tool)
        .unwrap_or_else(|| panic!("{tool} must be listed"))["inputSchema"]["properties"][field]
        ["description"]
        .as_str()
        .unwrap_or_else(|| panic!("{tool}.{field} must declare a description"))
        .to_string()
}

/// Issue #1129 — the shipped schema advertised "default 50, server cap 100" while the server
/// enforced 25/50, and an AI client has no other source for those bounds before its first call.
/// Anchored to the CONSTANTS, never to today's numbers: a test retyping 25/50 would be the same
/// hand-typed copy that drifted. The stale literal is asserted absent so the old text cannot
/// creep back in beside a correct one. Each bound is asserted as the WHOLE derived phrase rather
/// than two independent `contains` calls: separate calls still pass with the default and the cap
/// transposed, which is exactly the drift this test exists to catch.
#[test]
fn both_limit_descriptions_are_derived_from_the_constants_the_server_enforces() {
    let list = tools(Tier::Read);
    let found = property_description(&list, TOOL_FOUND_JOBS, "limit");
    assert!(
        found.contains(&format!(
            "default {DEFAULT_FOUND_JOBS_LIMIT}, server cap {MAX_FOUND_JOBS_LIMIT}"
        )),
        "found-jobs' limit must advertise the enforced default/cap: {found}"
    );
    assert!(
        !found.contains("100"),
        "the drifted cap must be gone, not merely joined by the real one: {found}"
    );
    let best = property_description(&list, TOOL_BEST_MATCHES, "limit");
    assert!(
        best.contains(&format!(
            "default {DEFAULT_BEST_MATCHES_LIMIT}, server cap {MAX_BEST_MATCHES_LIMIT}"
        )),
        "best-matches' limit must advertise the enforced default/cap: {best}"
    );
}

/// Issue #1130 — the wire cursor is `<autopilotId>:<offset>`, so calling it an "offset" invited
/// exactly the cross-autopilot reuse the resolver now rejects.
#[test]
fn the_found_jobs_cursor_is_advertised_as_an_opaque_per_autopilot_token() {
    let cursor = property_description(&tools(Tier::Read), TOOL_FOUND_JOBS, "cursor");
    assert!(
        !cursor.contains("offset"),
        "the cursor is no longer a bare offset and must not be described as one: {cursor}"
    );
    assert!(
        cursor.contains("opaque") && cursor.contains("autopilotId"),
        "it must say it is opaque and bound to the id that issued it: {cursor}"
    );
}

/// Round 2 fix (B3-r2-F5) — `best-matches`' `query` filters the already-capped, ranked
/// candidate list this tool computes (`BEST_MATCHES_CAP`, `commands::autopilot::best_matches`),
/// not the full stored corpus: a posting outside that cap reads `total: 0`/`matches: []`, a
/// confident false negative for issue #1168's headline question ("is this role already in my
/// list?"). Both the `query` property AND the tool's own base description (`VERB_TABLE`) must
/// steer a caller toward `found-jobs`' own `query`, which spans everything.
#[test]
fn best_matches_query_advertises_it_is_scoped_to_the_capped_ranked_list_not_the_full_corpus() {
    let list = tools(Tier::Read);
    let query = property_description(&list, TOOL_BEST_MATCHES, "query");
    assert!(
        query.contains("found-jobs") && query.contains("NOT"),
        "query's own description must name the scope and point at found-jobs: {query}"
    );
    let base = tool_description(&list, TOOL_BEST_MATCHES);
    assert!(
        base.contains("found-jobs"),
        "the tool's base description must point a caller at found-jobs for a full-corpus search: {base}"
    );
}

/// Issues #1167/#1168 — every new `found-jobs` server-side filter/flag must be
/// advertised on the schema a client reads BEFORE its first call, not
/// discoverable only by trial and error. `autopilotId` moved from required to
/// optional (issue #1168) at the same time, pinned here too so the two never
/// drift apart again the way the round-2 review found the limit numbers did.
#[test]
fn the_found_jobs_schema_advertises_every_filter_and_no_longer_requires_autopilot_id() {
    let list = tools(Tier::Read);
    for field in [
        "minScore",
        "country",
        "remote",
        "applied",
        "query",
        "includeDescription",
    ] {
        let description = property_description(&list, TOOL_FOUND_JOBS, field);
        assert!(
            !description.is_empty(),
            "found-jobs must advertise a `{field}` property"
        );
    }
    let tool = list
        .iter()
        .find(|t| t["name"] == TOOL_FOUND_JOBS)
        .expect("found-jobs listed");
    let required = tool["inputSchema"]["required"].as_array();
    assert!(
        required.is_none_or(|r| r.is_empty()),
        "autopilotId must no longer be required (issue #1168): {:?}",
        tool["inputSchema"]
    );
}

/// Issue #1146 P11 — `best-matches` gained the same `cursor`/`query` args
/// `found-jobs` already advertised; a client has no other source for either
/// before its first call.
#[test]
fn the_best_matches_schema_advertises_cursor_and_query() {
    let list = tools(Tier::Read);
    for field in ["cursor", "query"] {
        let description = property_description(&list, TOOL_BEST_MATCHES, field);
        assert!(
            !description.is_empty(),
            "best-matches must advertise a `{field}` property"
        );
    }
}

/// Issue #1168 — `job` matches by posting `url` ONLY; a caller trying to look
/// a posting up by title/company needs to be pointed at `found-jobs`' own
/// `query` filter instead of guessing.
#[test]
fn the_job_tool_description_says_url_only_and_points_at_found_jobs_query() {
    let description = tool_description(&tools(Tier::Read), TOOL_JOB);
    assert!(
        description.to_lowercase().contains("url only"),
        "job's description must say it matches by url only: {description}"
    );
    assert!(
        description.contains("found-jobs"),
        "job's description must point a title/company lookup at found-jobs: {description}"
    );
}

/// Round-4 fix T3-cont (PR #1182 round-5) — the whole safety of "absent
/// `applied` ≠ `false`" rests on a caller knowing to check for
/// `appliedUnavailable` instead of reading a missing key as falsy; that must
/// be readable from the tool descriptions themselves, not only from Rust doc
/// comments no MCP client ever sees.
#[test]
fn the_job_and_found_jobs_descriptions_document_applied_unavailable() {
    let list = tools(Tier::Read);
    for tool in [TOOL_JOB, TOOL_FOUND_JOBS] {
        let description = tool_description(&list, tool);
        assert!(
            description.contains("appliedUnavailable"),
            "{tool}'s description must document appliedUnavailable: {description}"
        );
    }
}

/// Issue #1132 — `totalFound` is the LAST run's kept count and diverged from the traversable
/// total by up to ~24x on real data, with nothing on the surface saying so.
#[test]
fn the_automations_description_distinguishes_both_totals() {
    let description = tool_description(&tools(Tier::Read), TOOL_AUTOMATIONS);
    assert!(
        description.contains("totalFound") && description.contains("foundJobsTotal"),
        "both totals must be named where a client reads them: {description}"
    );
    assert!(
        description.contains("`totalFound` is the last run's"),
        "`totalFound` must be qualified as last-run-only: {description}"
    );
}

/// Issue #1144 — a well-formed payload sent as the bare `input` fails with an opaque
/// `invoke_error` on ~24 write commands; the wrapper key isn't derivable, so both channels a
/// client reads must say it.
#[test]
fn the_generic_input_schema_and_the_instructions_both_document_the_wrapper_key() {
    let input = property_description(&tools(Tier::Read), TOOL_CALL_READ, "input");
    assert!(
        input.contains("parameter") && input.contains("invoke_error"),
        "the input schema must name the wrapper key rule and the recovery signal: {input}"
    );
    assert!(
        INSTRUCTIONS.contains("invoke_error"),
        "a client that reads only `instructions` must learn the same recovery: {INSTRUCTIONS}"
    );
}

/// A1-r3-AC-1 HIGH — the invalid_input promise in `INSTRUCTIONS` used to be unconditional
/// ("a call whose `input` carries an unrecognised … key refuses"), qualified only for `args:
/// null`, never for a wrapper arg whose `fields` is `null` (the generator could not resolve its
/// shape). For those rows (e.g. `autopilot_update`'s `req`) a wholly-unrecognised NESTED key is
/// never validated and reaches the app — this pins BOTH surfaces a client reads so the caveat
/// cannot silently drop again.
#[test]
fn instructions_and_commands_description_qualify_the_null_fields_nested_gap() {
    assert!(
        INSTRUCTIONS.contains("fields") && INSTRUCTIONS.contains("nothing inside it is validated"),
        "must disclose that a null-fields wrapper's nested keys are never checked: {INSTRUCTIONS}"
    );
    let description = tool_description(&tools(Tier::Irreversible), TOOL_COMMANDS);
    assert!(
        description.contains("nothing inside it is validated"),
        "the commands tool's own description must carry the same caveat: {description}"
    );
}

/// Roadmap #1146 P6 — `autopilot_run` does its whole scrape inside the app and can outlast this
/// server's per-call budget, so a timeout must not read as "the run stopped".
#[test]
fn call_irreversible_says_long_work_outlives_the_call_and_names_what_to_poll() {
    let description = tool_description(&tools(Tier::Irreversible), TOOL_CALL_IRREVERSIBLE);
    assert!(
        description.contains("autopilot_run") && description.contains("timeout"),
        "the async shape must be stated on the tool itself: {description}"
    );
    assert!(
        description.contains(TOOL_AUTOMATIONS),
        "and it must name what to poll instead: {description}"
    );
}
