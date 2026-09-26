use super::*;
// ── issue #1163/#1158/#1160: description, args, proofField, namespace filter ────────────────────

/// The issue's own worked example: `applications_delete`'s proof is
/// `application.title` — a multi-segment `Lookup` path, joined with `.`.
#[test]
fn commands_names_the_full_dotted_proof_field_for_a_multi_segment_lookup_path() {
    let out = commands_value(&json!({ "effect": "irreversible" }), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "applications_delete")
        .expect("applications_delete is a real Irreversible row");
    assert_eq!(row["proofField"], "application.title");
}

/// A `Count`-sourced row has no single field to name — the proof is a
/// DERIVED number, not a field on the read response — so `proofField` must
/// be absent rather than a fabricated empty string. `proofKind: "count"`
/// (CLI review round 2 — MEDIUM) tells the caller what to pass instead:
/// the array length / `total`, without dispatching the row to find out.
#[test]
fn commands_carries_no_proof_field_for_a_count_sourced_row() {
    let out = commands_value(&json!({ "effect": "irreversible" }), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "privacy_reset_app")
        .expect("privacy_reset_app is a real Count-sourced Irreversible row");
    assert!(
        row.get("proofField").is_none(),
        "a Count proof names no single field: {row}"
    );
    assert_eq!(row["proofKind"], "count");
}

/// A `MatchCount`-sourced row is the same "no single field" shape as `Count`, but the number
/// means something different (how many of the TARGETED ids exist) — still `proofKind: "count"`,
/// since the caller-facing action ("pass a count") is identical.
#[test]
fn commands_carries_count_kind_for_a_match_count_sourced_row() {
    let out = commands_value(&json!({ "effect": "irreversible" }), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "ai_generations_remove_bulk")
        .expect("ai_generations_remove_bulk is a real MatchCount-sourced Irreversible row");
    assert!(row.get("proofField").is_none());
    assert_eq!(row["proofKind"], "count");
}

/// A `Scalar`/`Lookup` with an EMPTY path names no field either — the proof IS the whole response
/// value — but that is a DIFFERENT reason than `Count`'s (CLI review round 2 — MEDIUM: both used
/// to collapse to an absent `proofField` with nothing telling them apart). `proofKind:
/// "response_value"` distinguishes it: pass the whole response, not a count.
#[test]
fn commands_carries_response_value_kind_for_an_empty_path_scalar_row() {
    let out = commands_value(&json!({ "effect": "irreversible" }), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "system_open_external")
        .expect("system_open_external is a real empty-path-Scalar Irreversible row");
    assert!(row.get("proofField").is_none());
    assert_eq!(row["proofKind"], "response_value");
}

/// A1-r2-AC-2 MEDIUM: the `commands` tool's OWN description is the one surface a model reads
/// before ever calling the tool, so it must name every key an Irreversible row can actually
/// emit — `proofKind` (present on all ~34 rows) AND `proofField` (present on only the subset
/// whose `proofKind` is "field"), never state `proofField` as if it were unconditional.
#[test]
fn the_commands_description_names_proof_kind_and_qualifies_proof_field() {
    let description = tool_description(&tools(Tier::Irreversible), TOOL_COMMANDS);
    assert!(
        description.contains("proofKind"),
        "must name proofKind, the field present on EVERY Irreversible row: {description}"
    );
    assert!(
        description.contains("proofField"),
        "must still name proofField: {description}"
    );
    assert!(
        description.contains("only when proofKind"),
        "must qualify proofField as conditional on proofKind, not state it unconditionally: \
         {description}"
    );
}

/// A1-r3-AC-2 MEDIUM: the description's "only when proofKind is field" promise (asserted above by
/// substring) is enforced nowhere on the actual payload — `proofField`/`proofKind` come from two
/// INDEPENDENT `match`es over `ProofSource` (`proof_field`/`proof_kind`), and only 4 hand-picked
/// rows were ever checked against each other. A `ProofSource` variant added to one match and not
/// its sibling would make the description a lie with every existing test green. This loops every
/// Irreversible row `commands` can emit and pins the invariant directly, so it fails the moment the
/// two matches diverge for ANY row, not just the 4 previously spot-checked.
#[test]
fn every_irreversible_row_has_proof_field_iff_its_proof_kind_is_field() {
    let out = commands_value(&json!({ "effect": "irreversible" }), Tier::Irreversible);
    let rows = out["commands"].as_array().unwrap();
    assert!(
        !rows.is_empty(),
        "must exercise at least one irreversible row"
    );
    for row in rows {
        let proof_kind = row["proofKind"]
            .as_str()
            .unwrap_or_else(|| panic!("every irreversible row must carry proofKind: {row}"));
        assert_eq!(
            row.get("proofField").is_some(),
            proof_kind == "field",
            "proofField presence must track proofKind == \"field\" for {}: {row}",
            row["command"]
        );
    }
}

/// A catalogued row carries its description and its declared args — pulled from the SAME
/// generated table `agent_call`'s dispatch-time validation reads, never a second copy.
#[test]
fn commands_carries_description_and_args_for_a_catalogued_row() {
    let out = commands_value(&json!({}), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "applications_delete")
        .expect("applications_delete is a real, catalogued row");
    let args = row["args"].as_array().expect("declared args, not null");
    let names: Vec<&str> = args.iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert!(
        names.contains(&"id") && names.contains(&"keepDocuments"),
        "{names:?}"
    );
    let keep_documents = args
        .iter()
        .find(|a| a["name"] == "keepDocuments")
        .expect("keepDocuments must be visible — the whole point of issue #1160");
    assert_eq!(
        keep_documents["required"], true,
        "keepDocuments is a required flag, not an optional one"
    );
}

/// MEDIUM — CLI review round 1: a colon-terminated description like "Factory reset:" (`summarize`
/// cutting on the FIRST `.` OR `:`, correct for a `docs/API.md` table cell but content-free when
/// the cut result is the entire text) reaches an LLM with nothing else to go on. The catalogue's
/// own `catalogueSummarize` cuts on `.` only, falling back to the full first paragraph when that
/// still leaves something too short or with a dangling backtick/paren.
#[test]
fn a_catalogued_description_never_ends_on_a_bare_colon() {
    let out = commands_value(&json!({}), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "privacy_reset_app")
        .expect("privacy_reset_app is a real, catalogued row");
    let description = row["description"].as_str().expect("a description string");
    assert!(
        !description.trim_end().ends_with(':'),
        "must not stop mid-sentence at a colon: {description}"
    );
    assert!(
        description.len() > 20,
        "too short to be useful: {description}"
    );
}

/// A `.`-cut can still land inside a backtick span or an unclosed parenthetical (an abbreviation
/// like "e.g." inside one) — `match_resume_text`'s own TSDoc has both traps in its first
/// sentence. The fallback to the full first paragraph must leave both balanced.
#[test]
fn a_catalogued_description_never_leaves_a_backtick_or_paren_unbalanced() {
    let out = commands_value(&json!({}), Tier::Irreversible);
    for row in out["commands"].as_array().unwrap() {
        let Some(description) = row["description"].as_str() else {
            continue;
        };
        let backticks = description.matches('`').count();
        assert_eq!(
            backticks % 2,
            0,
            "{}: unbalanced backticks in {description:?}",
            row["command"]
        );
        let open = description.matches('(').count();
        let close = description.matches(')').count();
        assert_eq!(
            open, close,
            "{}: unbalanced parens in {description:?}",
            row["command"]
        );
    }
}
