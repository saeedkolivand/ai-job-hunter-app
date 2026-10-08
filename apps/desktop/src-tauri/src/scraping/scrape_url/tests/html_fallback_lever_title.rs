//! #1416: Lever's JSON-LD `title` carries `&amp;` (real capture of the Spotify posting, trimmed
//! to the head and headline).

use super::super::*;

const LEVER: &str = include_str!("fixtures/lever_spotify_ampersand.html");
const URL: &str = "https://jobs.lever.co/spotify/d87833d0-fb78-4794-b45d-3fe5c8274bc8";

#[test]
fn json_ld_title_entities_are_decoded_once() {
    let p = parse_from_html(URL, LEVER).unwrap();
    assert_eq!(p.title, "Artist & Label Partnerships Contractor, Vietnam");
}

#[test]
fn a_literal_ampersand_entity_spelling_survives_a_single_decode() {
    let html = r#"<script type="application/ld+json">{"@type":"JobPosting","title":"R&amp;amp;D Lead","description":"x"}</script>"#;
    assert_eq!(parse_from_html(URL, html).unwrap().title, "R&amp;D Lead");
}

#[test]
fn tag_shaped_text_and_escaped_company_and_location_are_kept() {
    let html = r#"<script type="application/ld+json">{"@type":"JobPosting","title":"AT&amp;T <Senior>","description":"x","hiringOrganization":{"name":"AT&amp;T"},"jobLocation":{"address":{"addressLocality":"Q&amp;A City"}}}</script>"#;
    let p = parse_from_html(URL, html).unwrap();
    assert_eq!(p.title, "AT&T <Senior>");
    assert_eq!(p.company, "AT&T");
    assert_eq!(p.location.as_deref(), Some("Q&A City"));
}
