use super::*;

fn body(inner: &str) -> String {
    format!("<w:document><w:body>{inner}</w:body></w:document>")
}

fn para(tag: &str, text: &str) -> String {
    format!("{tag}<w:r><w:t>{text}</w:t></w:r></w:p>")
}

/// Each paragraph spelling splits exactly once, and the structure around it never opens a spurious
/// slice. Each case asserts the EXACT text (not `contains`), which is the point.
#[test]
fn paragraph_spellings_split_exactly_once() {
    let cases: Vec<(&str, String, &str)> = vec![
        // Google Docs / LibreOffice / python-docx write a bare `<w:p>`. Asserting
        // on the EXACT text (not `contains`) is the point: the old chained-split
        // emitted every paragraph twice — once mashed with no boundaries, once
        // per paragraph — and a `contains` assertion stayed green through it.
        (
            "bare_paragraph_tags_are_not_emitted_twice",
            format!("{}{}", para("<w:p>", "Jane Doe"), para("<w:p>", "Engineer")),
            "Jane Doe\nEngineer",
        ),
        // The MS-Word spelling (`<w:p w:rsidR="…">`) keeps working unchanged.
        (
            "attributed_paragraph_tags_still_split",
            format!(
                "{}{}",
                para("<w:p w:rsidR=\"00A1\">", "Jane Doe"),
                para("<w:p w:rsidR=\"00A2\">", "Engineer")
            ),
            "Jane Doe\nEngineer",
        ),
        // A document mixing both spellings — neither branch may drop or duplicate.
        (
            "mixed_paragraph_spellings_each_appear_once",
            format!(
                "{}{}{}",
                para("<w:p w:rsidR=\"00A1\">", "Alpha"),
                para("<w:p>", "Beta"),
                para("<w:p w:rsidR=\"00A3\">", "Gamma")
            ),
            "Alpha\nBeta\nGamma",
        ),
        // A self-closing `<w:p/>` (an empty paragraph) is folded into the previous
        // slice rather than opening its own. It carries no runs, so it contributes
        // no text either way — pinning the boundary so the behaviour is on record.
        (
            "self_closing_empty_paragraph_neither_splits_nor_duplicates",
            format!("{}<w:p/>{}", para("<w:p>", "A"), para("<w:p>", "B")),
            "A\nB",
        ),
        // `<w:pPr>` (paragraph properties) starts with `<w:p` but is not a
        // paragraph — it must not open a new slice.
        (
            "paragraph_properties_tag_is_not_a_paragraph",
            "<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r><w:t>Solo</w:t></w:r></w:p>"
                .to_string(),
            "Solo",
        ),
    ];
    for (name, inner, expected) in cases {
        let (text, _) = parse_document(&body(&inner), &HashMap::new());
        assert_eq!(text, expected, "{name}");
    }
}
