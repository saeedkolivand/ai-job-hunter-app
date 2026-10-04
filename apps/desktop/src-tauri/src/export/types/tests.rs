use super::*;

/// A stale frontend that sends a removed template id (e.g. "two-column",
/// "refined-executive") or a completely unknown string must NEVER cause a
/// deserialisation error — it must silently fall back to `Classic`.
#[test]
fn unknown_template_id_falls_back_to_classic() {
    for bad in &[
        "modern",
        "two-column",
        "refined-executive",
        "executive",
        "editorial-serif",
        "mono-technical",
        "bogus",
        "BOGUS",
        "",
    ] {
        let json = format!("\"{}\"", bad);
        let id: TemplateId = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("deserialise {:?} failed: {e}", bad));
        assert_eq!(
            id,
            TemplateId::Classic,
            "unknown id {:?} should fall back to Classic, got {id:?}",
            bad
        );
    }
}

/// Live ids must still round-trip correctly (no regression).
#[test]
fn live_template_ids_round_trip() {
    let cases = [
        (TemplateId::Classic, "\"classic\""),
        (TemplateId::SwissMinimal, "\"swiss-minimal\""),
        (TemplateId::Academic, "\"academic\""),
        (TemplateId::Atelier, "\"atelier\""),
        (TemplateId::Meridian, "\"meridian\""),
        (TemplateId::Throughline, "\"throughline\""),
        (TemplateId::Portrait, "\"portrait\""),
        (TemplateId::Lebenslauf, "\"lebenslauf\""),
        (TemplateId::Cadence, "\"cadence\""),
        (TemplateId::Regent, "\"regent\""),
        (TemplateId::CologneNavy, "\"cologne-navy\""),
        (TemplateId::Aria, "\"aria\""),
        (TemplateId::Saffron, "\"saffron\""),
        (TemplateId::Jake, "\"jake\""),
        (TemplateId::Awesome, "\"awesome\""),
        (TemplateId::Deedy, "\"deedy\""),
    ];
    for (id, expected_json) in cases {
        let serialized = serde_json::to_string(&id).expect("serialize");
        assert_eq!(serialized, expected_json, "{id:?} serialized wrong");
        let deserialized: TemplateId = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized, id, "{id:?} did not round-trip");
    }
}

/// Every letter layout round-trips through kebab-case serde.
#[test]
fn letter_layout_round_trips() {
    let cases = [
        (LetterLayout::Classic, "\"classic\""),
        (LetterLayout::Refined, "\"refined\""),
        (LetterLayout::Banded, "\"banded\""),
        (LetterLayout::Navy, "\"navy\""),
        (LetterLayout::Sidebar, "\"sidebar\""),
        (LetterLayout::Monogram, "\"monogram\""),
    ];
    for (layout, expected_json) in cases {
        let serialized = serde_json::to_string(&layout).expect("serialize");
        assert_eq!(serialized, expected_json, "{layout:?} serialized wrong");
        let deserialized: LetterLayout = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized, layout, "{layout:?} did not round-trip");
    }
}

/// An unknown / removed letter-layout id must never error — it falls back to
/// `Classic`, mirroring `TemplateId`'s graceful degradation.
#[test]
fn unknown_letter_layout_falls_back_to_classic() {
    for bad in &["olivia", "belinda", "two-column", "bogus", "BANDED", ""] {
        let json = format!("\"{}\"", bad);
        let layout: LetterLayout = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("deserialise {:?} failed: {e}", bad));
        assert_eq!(
            layout,
            LetterLayout::Classic,
            "unknown layout {:?} should fall back to Classic, got {layout:?}",
            bad
        );
    }
}

/// The default (used by `#[serde(default)]` when the field is absent) is
/// `Classic` — the pre-layout-picker output.
#[test]
fn letter_layout_default_is_classic() {
    assert_eq!(LetterLayout::default(), LetterLayout::Classic);
}

/// The wire field is `letterLayoutId` (shared TS contract), and an absent
/// field defaults to `Classic` so existing cover-letter requests are
/// unaffected.
#[test]
fn export_request_reads_letter_layout_id_and_defaults() {
    let with_layout: ExportRequest = serde_json::from_str(
        r#"{"text":"x","format":"pdf","documentType":"cover-letter",
            "templateId":"classic","meta":null,"letterLayoutId":"banded"}"#,
    )
    .expect("deserialize request with letterLayoutId");
    assert_eq!(with_layout.letter_layout, LetterLayout::Banded);

    let without: ExportRequest = serde_json::from_str(
        r#"{"text":"x","format":"pdf","documentType":"cover-letter",
            "templateId":"classic","meta":null}"#,
    )
    .expect("deserialize request without letterLayoutId");
    assert_eq!(without.letter_layout, LetterLayout::Classic);
}
