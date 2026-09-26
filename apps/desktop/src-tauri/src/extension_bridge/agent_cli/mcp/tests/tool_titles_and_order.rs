use super::*;
// ── title + deterministic order (roadmap #1146 P1, P10) ─────────────────

/// P1 — every client's tool-approval UI shows `title` when present and the raw wire `name` when
/// not, so a tool added without one is a visible regression. Derived sweep + the hand-written
/// literal list below, per the "a guard driven off its own data can't catch a deletion" rule.
#[test]
fn every_tool_carries_a_non_empty_human_title() {
    for tool in tools(Tier::Irreversible) {
        let title = tool["title"].as_str().unwrap_or_default();
        assert!(
            !title.is_empty(),
            "{} must carry a human title",
            tool["name"]
        );
        assert_ne!(
            title,
            tool["name"].as_str().unwrap_or_default(),
            "a title that just repeats the wire name adds nothing"
        );
    }
}

#[test]
fn tool_titles_match_a_hand_written_literal_list() {
    let list = tools(Tier::Irreversible);
    let titles: Vec<&str> = list
        .iter()
        .map(|t| t["title"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        titles,
        vec![
            "Best Matches",
            "Job by URL",
            "My Profile",
            "Automations",
            "Found Jobs",
            "Commands",
            "Call (read)",
            "Call (reversible)",
            "Call (irreversible)",
        ]
    );
}

/// Round-1 review, issue #1180 (P-r1-F3): the generic `call-read
/// contact_profile_get` row and this resource share field NAMES minus
/// `photo` — never more than that. Overclaiming "identical" reads as "same
/// values, same consent gate" to an LLM client (and the next reviewer), and
/// neither is true: this resource trims/collapses/cleans its values and is
/// opt-in gated, the generic row is neither.
///
/// Round 3 (P-r3-AC-R3-F4): this is MODEL-facing text, so it must not (a)
/// carry review-process chatter ("issue #1180", "round-2 review") into the
/// tool catalogue a client reads, or (b) name the generic, ungated
/// `call-read`/`contact_profile_get` route as an available substitute at
/// exactly the moment this tool's own consent gate refuses — a disclaimer
/// naming a bypass is a hint toward it, not a warning against it. That fuller
/// comparison belongs in the Rust doc comment above the `curated_tool` call
/// (a maintainer surface), not in the wire description.
#[test]
fn profile_tool_description_never_overclaims_parity_with_the_generic_row() {
    let list = tools(Tier::Irreversible);
    let profile = list
        .iter()
        .find(|t| t["name"] == TOOL_PROFILE)
        .expect("profile tool is listed");
    let description = profile["description"].as_str().unwrap_or_default();
    assert!(
        !description.to_ascii_lowercase().contains("identical"),
        "the generic row differs in value shape, link cleaning and the consent gate: \
         {description}"
    );
    assert!(
        description.contains("consent gate") && description.contains("does not apply"),
        "must say the consent gate does NOT extend to the generic row: {description}"
    );
    assert!(
        !description.contains("call-read") && !description.contains("contact_profile_get"),
        "must not name the ungated generic route as a substitute for this consent gate: \
         {description}"
    );
    assert!(
        !description.to_ascii_lowercase().contains("issue #")
            && !description.to_ascii_lowercase().contains("round-"),
        "model-facing text must not carry review-process chatter: {description}"
    );
    // P-r3-AC-R7-F1 (round-3 review, issue #1180): `curated_tool`'s `extra` param is
    // APPENDED after the VERB_TABLE base sentence, never a replacement for it — passing a
    // full description (including a restatement of the base) doubles the opening sentence.
    assert_eq!(
        description.matches("fields for autofill").count(),
        1,
        "the base VERB_TABLE sentence and `extra` must not both describe autofill fields: \
         {description}"
    );
}

/// P1 — every tool carries the same `icons` entry (2025-11-25 tool schema), and it is a `data:`
/// URI, the only icon source a stdio MCP server (ADR-040) can use that every client — including
/// VS Code, which does not resolve a cross-origin `https://` icon for stdio servers — can render.
#[test]
fn every_tool_carries_the_same_data_uri_icon() {
    let list = tools(Tier::Irreversible);
    assert!(!list.is_empty());
    let first = list[0]["icons"].clone();
    let icons = first.as_array().expect("icons must be an array");
    assert_eq!(icons.len(), 1);
    let src = icons[0]["src"].as_str().expect("icon must carry a src");
    assert!(
        src.starts_with("data:image/png;base64,"),
        "icon src must be a data URI a stdio MCP client can render: {src}"
    );
    for tool in &list {
        assert_eq!(
            tool["icons"], first,
            "{} must carry the identical icons entry",
            tool["name"]
        );
    }
}

/// P10 — deterministic ordering is what lets a client's prompt cache survive repeated
/// `tools/list` calls in a long session. Two properties, both mutation-visible: the order is
/// STABLE call-to-call, and every lower tier is a strict PREFIX of the next, so enabling a write
/// tier appends rather than reshuffling the read tools a cached prompt already holds.
#[test]
fn tools_list_order_is_stable_across_calls_and_prefix_stable_across_tiers() {
    let ordered = |tier| -> Vec<String> {
        tools(tier)
            .iter()
            .map(|t| t["name"].as_str().unwrap_or_default().to_string())
            .collect()
    };
    let read = ordered(Tier::Read);
    assert_eq!(read, ordered(Tier::Read), "two calls must agree");
    assert_eq!(
        read,
        vec![
            "best-matches",
            "job",
            "profile",
            "automations",
            "found-jobs",
            "commands",
            "call-read",
        ],
        "the read tier's order is part of the contract, not an accident of construction"
    );
    let reversible = ordered(Tier::Reversible);
    let irreversible = ordered(Tier::Irreversible);
    assert_eq!(
        reversible[..read.len()],
        read[..],
        "read tier stays a prefix"
    );
    assert_eq!(
        irreversible[..reversible.len()],
        reversible[..],
        "reversible tier stays a prefix"
    );
    assert_eq!(reversible[read.len()..], ["call-reversible"]);
    assert_eq!(irreversible[reversible.len()..], ["call-irreversible"]);
}
