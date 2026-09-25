use super::*;
// ── curated_tool description join (item 8) ──────────────────────────────

#[test]
fn curated_tool_joins_base_and_extra_as_two_sentences_not_a_run_on() {
    let tool = tools(Tier::Read)
        .into_iter()
        .find(|t| t["name"] == TOOL_BEST_MATCHES)
        .unwrap();
    let description = tool["description"].as_str().unwrap().to_string();
    assert!(
        description.contains(". "),
        "base and extra must be joined as two sentences: {description}"
    );
    assert!(
        !description.contains(") title/company"),
        "must never join with a bare space (the live run-on this fix closed): {description}"
    );
}

#[test]
fn every_scraped_text_tool_carries_the_same_untrusted_fields_notice() {
    // pre-PR gate, extended in review round 2 (MEDIUM — `found-jobs` was added without
    // extending this pairwise check, the exact class of gap `#1088` warns about): every
    // curated tool that returns title/company/location/description scraped text must carry
    // the IDENTICAL notice as `best-matches`, never a fresh one-off pair test per new tool.
    let list = tools(Tier::Read);
    let notice = list
        .iter()
        .find(|t| t["name"] == TOOL_BEST_MATCHES)
        .unwrap()["description"]
        .as_str()
        .unwrap()
        .rsplit_once(". ")
        .unwrap()
        .1
        .to_string();
    for tool in [TOOL_JOB, TOOL_FOUND_JOBS] {
        let description = list.iter().find(|t| t["name"] == tool).unwrap()["description"]
            .as_str()
            .unwrap();
        assert!(
            description.contains(&notice),
            "{tool}'s description must carry the same untrusted-fields notice as best-matches: \
             {description}"
        );
    }
}

/// Issue #1170, round 5 (`B1-r1-ACLI-R5-4`): the `profile` tool must say up front it holds
/// contact fields only, and point at REAL `POLICY` reads for the résumé/document text itself (a
/// stale rename here would send a calling model at a command that no longer exists). Both
/// `documents:documents_list` (fenced/capped rows) AND `documents:documents_get_text` (the same
/// text by id, fenced and capped at the SAME limit — round 6, `B1-r2-ACLI-R6-1`: it does NOT
/// return more) are named — the earlier version of this description dropped `documents_get_text`
/// entirely rather than spelling out that its `id` param maps to `documents_list`'s `_id` value,
/// leaving an assistant with no way to reach a résumé by id at all, only by re-listing.
#[test]
fn profile_tool_description_names_a_real_document_read() {
    let list = tools(Tier::Read);
    let description = list.iter().find(|t| t["name"] == TOOL_PROFILE).unwrap()["description"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        description.contains("Contact fields only"),
        "must say the profile tool holds contact fields only: {description}"
    );
    // Every `ns:cmd`-shaped token is pulled OUT of the description text itself (same
    // discipline as `instructions_ns_cmd_pairs_are_real_policy_rows` below) — a hand-picked
    // pair list would keep passing after a rename inside the string, which is exactly how the
    // round-1 fix missed `documents_get_text` (issue #1164 round 2).
    let is_ident = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '_');
    let mut checked = 0usize;
    for word in description.split_whitespace() {
        let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != ':' && c != '_');
        let Some((ns, cmd)) = word.split_once(':') else {
            continue;
        };
        if !is_ident(ns) || !is_ident(cmd) {
            continue;
        }
        checked += 1;
        let entry = POLICY
            .iter()
            .find(|e| agent_call::split_path(e.path) == (ns, cmd))
            .unwrap_or_else(|| {
                panic!("profile's description names {word}, which is not a real POLICY row")
            });
        assert!(
            matches!(entry.effect, Effect::Read),
            "profile's description tells a caller to reach {word} via call-read, but its \
             POLICY row is not Effect::Read"
        );
    }
    assert_eq!(
        checked, 2,
        "expected exactly the 2 ns:cmd pairs the profile description names (round 5, \
         `B1-r1-ACLI-R5-4`) — documents:documents_list and documents:documents_get_text, \
         matching INSTRUCTIONS: {description}"
    );
}
