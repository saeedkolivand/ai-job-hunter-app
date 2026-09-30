//! Tests asserting the OpenAI and Gemini translators degrade in lockstep on
//! schema shapes neither one's walker can safely descend into (composition
//! keywords, tuple-form `items`) — pinned together so one schema can never
//! land strict-mode'd on one provider while silently weakened on the other.

use super::super::*;

#[test]
fn both_translators_degrade_whole_on_a_composition_keyword_they_cannot_walk() {
    // The strict-mode 400 shape: `strictify` recurses through `properties`
    // and `items` only, so the object INSIDE `anyOf` came back with neither
    // `required` nor `additionalProperties` — under `strict: true`, which
    // OpenAI rejects outright, failing the whole generation. This was the
    // only schema path with no degrade (a non-object root and the depth cap
    // both already fall back to `json_object`). Mutation check: drop the
    // `has_untranslatable_keyword` filter in `openai_response_format` and
    // the first assertion fails.
    let anyof = json!({
        "type": "object",
        "properties": {
            "note": {
                "anyOf": [
                    { "type": "object", "properties": { "text": { "type": "string" } } },
                    { "type": "string" },
                ],
            },
        },
    });
    assert_eq!(
        openai_response_format(Some(&anyof)),
        json!({ "type": "json_object" })
    );
    // Gemini rejects that exact shape anyway (the `anyOf` node carries no
    // `type`). The shape it would silently WEAKEN instead is a TYPED node
    // carrying the keyword: `anyOf` is not in `GEMINI_KEPT_KEYWORDS`, so the
    // union would simply vanish and the property ship as a bare
    // `{"type": "OBJECT"}` — a constraint the caller asked for, dropped
    // without a word. Same degrade on both, so one schema can't be strict
    // on one provider and quietly loosened on the other.
    assert!(gemini_response_schema(&anyof).is_none());
    let typed_anyof = json!({
        "type": "object",
        "properties": {
            "note": {
                "type": "object",
                "anyOf": [
                    { "type": "object", "properties": { "text": { "type": "string" } } },
                    { "type": "object", "properties": { "code": { "type": "string" } } },
                ],
            },
        },
    });
    assert!(gemini_response_schema(&typed_anyof).is_none());
    assert_eq!(
        openai_response_format(Some(&typed_anyof)),
        json!({ "type": "json_object" })
    );

    // At ANY depth, and under a key neither walker even visits.
    let nested = json!({
        "type": "object",
        "properties": {
            "items": {
                "type": "array",
                "items": { "type": "object", "additionalProperties": { "$ref": "#/$defs/x" } },
            },
        },
    });
    assert_eq!(
        openai_response_format(Some(&nested)),
        json!({ "type": "json_object" })
    );
    assert!(gemini_response_schema(&nested).is_none());

    // No false positives: an ordinary in-cap schema still translates on
    // both — the degrade must cost nothing for the schemas callers write.
    let plain = json!({
        "type": "object",
        "properties": { "score": { "type": "integer" }, "notes": { "type": "string" } },
    });
    assert_eq!(
        openai_response_format(Some(&plain))["type"],
        json!("json_schema")
    );
    assert!(gemini_response_schema(&plain).is_some());
}

#[test]
fn both_translators_degrade_whole_on_tuple_form_items() {
    // TUPLE-form `items` (an ARRAY of schemas — draft-07 tuple validation,
    // renamed `prefixItems` in 2020-12) is the third way to reach a subtree
    // no walker vets, after the composition keywords and the depth cap.
    // Neither `strictify` nor `openai_strict_keywords_only` descends into
    // it — both bottom out on "not a JSON object" — so every tuple member
    // was copied VERBATIM into a `strict: true` schema. Two independent
    // 400s ride in that gap, and both are the no-degrade kind:
    //   1. an unsupported keyword (`maxLength`) reaching the vendor
    //      allowlist unfiltered, and
    //   2. an object member with neither `required` nor
    //      `additionalProperties` — exactly the composition-keyword shape.
    // OpenAI's strict subset does not document array-form `items` at all
    // (its "Supported properties" list for arrays is `minItems`/`maxItems`
    // — see `OPENAI_STRICT_KEYWORDS`), so the honest posture is the one the
    // composition keywords already take: degrade the WHOLE schema.
    // Mutation check: restore `openai_strict_keywords_only`'s non-object
    // `return true` and both OpenAI assertions below fail.
    let tuple = json!({
        "type": "object",
        "properties": {
            "pair": {
                "type": "array",
                "items": [
                    { "type": "string", "maxLength": 8 },
                    { "type": "object", "properties": { "id": { "type": "string" } } },
                ],
            },
        },
    });
    assert_eq!(
        openai_response_format(Some(&tuple)),
        json!({ "type": "json_object" })
    );
    // Same shape hung off an OBJECT-typed node, where `strictify` never
    // even looks at `items` (its object arm only rewrites `properties`) and
    // would clone the tuple through untouched — which is why the guard
    // lives on the walk that visits `items` on EVERY node, not in
    // `strictify`'s array arm.
    let object_tuple = json!({
        "type": "object",
        "properties": {
            "odd": { "type": "object", "items": [{ "type": "string", "maxLength": 8 }] },
        },
    });
    assert_eq!(
        openai_response_format(Some(&object_tuple)),
        json!({ "type": "json_object" })
    );
    // Gemini already degrades whole on both (its translator starts with
    // `as_object()?`, and it visits `items` regardless of `type`) — pinned
    // here so the two providers can never drift into "strict on one,
    // silently unconstrained on the other".
    assert!(gemini_response_schema(&tuple).is_none());
    assert!(gemini_response_schema(&object_tuple).is_none());

    // No false positives: object-form `items` — the shape every caller
    // actually writes — must still reach the decoder on both.
    let object_form = json!({
        "type": "object",
        "properties": { "tags": { "type": "array", "items": { "type": "string" } } },
    });
    assert_eq!(
        openai_response_format(Some(&object_form))["type"],
        json!("json_schema")
    );
    assert!(gemini_response_schema(&object_form).is_some());
}
