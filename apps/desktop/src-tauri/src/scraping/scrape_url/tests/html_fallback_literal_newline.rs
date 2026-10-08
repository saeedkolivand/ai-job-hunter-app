//! A standalone literal backslash-n in a decoded JSON-LD description (a real Lever/Spotify
//! posting) is restored to a newline at the decode site, never in the shared converter.

use super::super::html_fallback::unescape_literal_newlines;
use super::super::*;

#[test]
fn json_ld_description_with_a_literal_backslash_n_between_tags_loses_it() {
    // The JSON text `\\n` decodes to the two characters backslash + n.
    let html = r#"<html><head><script type="application/ld+json">
        {"@type":"JobPosting","title":"Android Engineer",
         "description":"<p>Great team.</p>\\n<p>What You'll Do</p>\\n"}
        </script></head></html>"#;
    let desc = parse_from_html("https://jobs.lever.co/spotify/1", html)
        .unwrap()
        .description
        .unwrap();
    assert!(
        !desc.contains('\\'),
        "literal backslash-n survived: {desc:?}"
    );
    assert!(desc.contains("What You'll Do"));
}

#[test]
fn text_that_is_not_a_standalone_literal_backslash_n_is_left_alone() {
    for keep in [
        r"<code>\n</code>",
        r"<pre>a\n b</pre>",
        r"<p>C:\new</p>",
        r"<p>a \\n b</p>",
    ] {
        assert_eq!(unescape_literal_newlines(keep), keep);
    }
}
