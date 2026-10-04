use super::*;

#[test]
fn extracts_text_and_drops_script_style() {
    let html = r#"<html><head><title>x</title></head><body>
            <style>.a{color:red}</style>
            <h1>Jane Doe</h1>
            <script>var x = 1;</script>
            <p>Senior Engineer</p>
        </body></html>"#;
    let (text, _) = parse_html(html);
    assert!(text.contains("Jane Doe"));
    assert!(text.contains("Senior Engineer"));
    assert!(!text.contains("color:red"));
    assert!(!text.contains("var x"));
}

#[test]
fn converts_anchors_to_markdown_links() {
    let html = r#"<p>Contact: <a href="https://linkedin.com/in/jane">LinkedIn</a> | <a href="mailto:jane@example.com">Email</a></p>"#;
    let (text, links) = parse_html(html);
    assert!(text.contains("[LinkedIn](https://linkedin.com/in/jane)"));
    assert!(text.contains("[Email](mailto:jane@example.com)"));
    assert_eq!(links.len(), 2);
    assert!(links
        .iter()
        .any(|l| l.url == "https://linkedin.com/in/jane"));
}

#[test]
fn block_tags_become_line_breaks() {
    let html = "<div>Experience</div><div>Acme Corp</div>";
    let (text, _) = parse_html(html);
    assert_eq!(text, "Experience\nAcme Corp");
}

#[test]
fn decodes_entities() {
    let html = "<p>R&amp;D &middot; Tools&nbsp;&amp;&nbsp;Tech</p>";
    let (text, _) = parse_html(html);
    assert!(text.contains("R&D"));
    assert!(text.contains("·"));
}

#[test]
fn full_extract_sets_html_source() {
    let html = b"<html><body><h1>Jane</h1><p>jane@example.com</p></body></html>";
    let r = extract(html).expect("html");
    assert_eq!(r.source_format, SourceFormat::Html);
    assert!(r.text.contains("Jane"));
}

#[test]
fn empty_html_errors() {
    let html = b"<html><head></head><body></body></html>";
    assert!(extract(html).is_err());
}
