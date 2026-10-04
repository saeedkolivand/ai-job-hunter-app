use super::*;

/// Convenience: collapse a RichText into (text, bold, link) tuples for asserts.
fn shape(rt: &RichText) -> Vec<(String, bool, Option<String>)> {
    rt.iter()
        .map(|r| (r.text.clone(), r.bold, r.link.clone()))
        .collect()
}

#[test]
fn tokenize_plain_text_is_one_run() {
    let rt = tokenize_rich("just plain text");
    assert_eq!(
        shape(&rt),
        vec![("just plain text".to_string(), false, None)]
    );
}

#[test]
fn tokenize_parses_bold_segments() {
    let rt = tokenize_rich("Built **React** and **Rust** apps");
    assert_eq!(
        shape(&rt),
        vec![
            ("Built ".to_string(), false, None),
            ("React".to_string(), true, None),
            (" and ".to_string(), false, None),
            ("Rust".to_string(), true, None),
            (" apps".to_string(), false, None),
        ]
    );
}

#[test]
fn tokenize_keeps_markdown_links_as_link_runs() {
    let rt = tokenize_rich("See [GitHub](https://github.com/jane) today");
    assert_eq!(
        shape(&rt),
        vec![
            ("See ".to_string(), false, None),
            (
                "GitHub".to_string(),
                false,
                Some("https://github.com/jane".to_string())
            ),
            (" today".to_string(), false, None),
        ]
    );
}

#[test]
fn tokenize_labels_bare_urls_and_emails() {
    let url = tokenize_rich("visit https://janedoe.dev now");
    assert_eq!(
        shape(&url),
        vec![
            ("visit ".to_string(), false, None),
            (
                "janedoe.dev".to_string(),
                false,
                Some("https://janedoe.dev".to_string())
            ),
            (" now".to_string(), false, None),
        ]
    );

    let email = tokenize_rich("reach me at jane@example.com");
    assert_eq!(
        shape(&email),
        vec![
            ("reach me at ".to_string(), false, None),
            (
                "jane@example.com".to_string(),
                false,
                Some("mailto:jane@example.com".to_string())
            ),
        ]
    );
}

/// Owner-reported: a contact line written as
/// `[linkedin.com/in/x](https://linkedin.com/in/x)` (the shape a pasted
/// bare link takes with no chosen label) must show the short brand label
/// ("LinkedIn"), not the full URL text, while the link itself survives.
#[test]
fn tokenize_shortens_a_markdown_link_whose_label_is_the_bare_url() {
    let rt = tokenize_rich("[linkedin.com/in/jane](https://linkedin.com/in/jane)");
    assert_eq!(
        shape(&rt),
        vec![(
            "LinkedIn".to_string(),
            false,
            Some("https://linkedin.com/in/jane".to_string())
        )]
    );
}

/// Regression: an uppercase-prefixed bare-URL label (`HTTPS://…`,
/// `WWW.…`) must still be recognized as bare and shortened to the brand
/// label — the same shape as
/// `tokenize_shortens_a_markdown_link_whose_label_is_the_bare_url` but
/// with a differently-cased scheme/`www.` prefix on the label side only,
/// which used to defeat `is_bare_url_label`'s comparison (it stripped
/// case-sensitively before comparing case-insensitively, so a stripped
/// url and an un-stripped, differently-cased label never matched).
#[test]
fn tokenize_shortens_a_markdown_link_whose_label_has_an_uppercase_prefix() {
    let rt = tokenize_rich("[HTTPS://linkedin.com/in/jane](https://linkedin.com/in/jane)");
    assert_eq!(
        shape(&rt),
        vec![(
            "LinkedIn".to_string(),
            false,
            Some("https://linkedin.com/in/jane".to_string())
        )]
    );

    let rt2 = tokenize_rich("[WWW.linkedin.com/in/jane](https://linkedin.com/in/jane)");
    assert_eq!(
        shape(&rt2),
        vec![(
            "LinkedIn".to_string(),
            false,
            Some("https://linkedin.com/in/jane".to_string())
        )]
    );
}

/// A deliberately CHOSEN label (not the bare URL) is never overwritten.
#[test]
fn tokenize_keeps_a_deliberately_chosen_markdown_link_label() {
    let rt = tokenize_rich("[My Portfolio](https://janedoe.dev/work)");
    assert_eq!(
        shape(&rt),
        vec![(
            "My Portfolio".to_string(),
            false,
            Some("https://janedoe.dev/work".to_string())
        )]
    );
}

#[test]
fn tokenize_merges_bold_and_links_in_one_line() {
    let rt = tokenize_rich("**Lead** — [LinkedIn](https://linkedin.com/in/x)");
    assert_eq!(
        shape(&rt),
        vec![
            ("Lead".to_string(), true, None),
            (" — ".to_string(), false, None),
            (
                "LinkedIn".to_string(),
                false,
                Some("https://linkedin.com/in/x".to_string())
            ),
        ]
    );
}

/// The exact shape `pipeline::resume::project_render::render_project` emits for
/// a project title line — a bold name followed by two labeled project
/// links, `·`-separated. Both labels ("Website"/"Github") must render as
/// the CLICKABLE TEXT, not the raw URL — this is what carries a source
/// résumé's own link labels through to the PDF/DOCX export rather than
/// falling back to the bare href.
#[test]
fn tokenize_renders_project_link_labels_as_the_hyperlink_text() {
    let rt = tokenize_rich(
        "**Ledger CLI** · [Website](https://ledger.example.dev) · \
         [Github](https://github.com/janedoe/ledger)",
    );
    assert_eq!(
        shape(&rt),
        vec![
            ("Ledger CLI".to_string(), true, None),
            (" · ".to_string(), false, None),
            (
                "Website".to_string(),
                false,
                Some("https://ledger.example.dev".to_string())
            ),
            (" · ".to_string(), false, None),
            (
                "Github".to_string(),
                false,
                Some("https://github.com/janedoe/ledger".to_string())
            ),
        ]
    );
}

#[test]
fn tokenize_parses_bold_inside_a_link_label() {
    let rt = tokenize_rich("[**Site**](https://janedoe.dev)");
    assert_eq!(
        shape(&rt),
        vec![(
            "Site".to_string(),
            true,
            Some("https://janedoe.dev".to_string())
        )]
    );
}
