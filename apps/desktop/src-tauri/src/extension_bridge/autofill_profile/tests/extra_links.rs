//! `extra_links` projection (`AutofillProfile::from_contact`/`clean_extra_links`) --
//! redistributed from the crate-level `test.rs` (R8 relief). Additive optional field -- PR 4 of
//! the extension roadmap: old extensions ignore the key; old desktops never send it (see the
//! omitted-when-absent test below), so neither side needs a protocol bump.

use super::super::*;

#[test]
fn from_contact_projects_valid_extra_links_verbatim_and_trims_whitespace() {
    use crate::contact_profile::{ContactLink, ContactProfile};
    let profile = ContactProfile {
        extra_links: vec![
            ContactLink {
                label: "Portfolio".to_string(),
                url: "https://saeed.dev".to_string(),
            },
            ContactLink {
                label: "  Dribbble  ".to_string(),
                url: "  http://dribbble.com/saeed  ".to_string(),
            },
        ],
        ..Default::default()
    };
    let out = AutofillProfile::from_contact(&profile);
    assert_eq!(out.extra_links.len(), 2);
    assert_eq!(out.extra_links[0].label, "Portfolio");
    assert_eq!(out.extra_links[0].url, "https://saeed.dev");
    assert_eq!(out.extra_links[1].label, "Dribbble");
    assert_eq!(
        out.extra_links[1].url, "http://dribbble.com/saeed",
        "surrounding whitespace is trimmed; the URL itself is otherwise verbatim"
    );
}

#[test]
fn from_contact_drops_empty_label_empty_url_and_non_http_scheme_entries() {
    use crate::contact_profile::{ContactLink, ContactProfile};
    let profile = ContactProfile {
        extra_links: vec![
            ContactLink {
                label: "".to_string(),
                url: "https://example.com".to_string(),
            },
            ContactLink {
                label: "   ".to_string(),
                url: "https://example.com".to_string(),
            },
            ContactLink {
                label: "Notes".to_string(),
                url: "".to_string(),
            },
            ContactLink {
                label: "Sketchy".to_string(),
                url: "javascript:alert(1)".to_string(),
            },
            ContactLink {
                label: "FTP".to_string(),
                url: "ftp://example.com/file".to_string(),
            },
            ContactLink {
                label: "Portfolio".to_string(),
                url: "https://saeed.dev".to_string(),
            },
        ],
        ..Default::default()
    };
    let out = AutofillProfile::from_contact(&profile);
    assert_eq!(
        out.extra_links.len(),
        1,
        "only the one valid http(s)-scheme, non-empty-label entry survives"
    );
    assert_eq!(out.extra_links[0].label, "Portfolio");
}

#[test]
fn from_contact_caps_extra_links_at_ten() {
    use crate::contact_profile::{ContactLink, ContactProfile};
    let profile = ContactProfile {
        extra_links: (0..15)
            .map(|i| ContactLink {
                label: format!("Link {i}"),
                url: format!("https://example.com/{i}"),
            })
            .collect(),
        ..Default::default()
    };
    let out = AutofillProfile::from_contact(&profile);
    assert_eq!(out.extra_links.len(), MAX_EXTRA_LINKS);
    assert_eq!(out.extra_links[0].label, "Link 0");
    assert_eq!(out.extra_links[9].label, "Link 9");
}

#[test]
fn profile_result_reply_omits_extra_links_key_when_absent() {
    use crate::contact_profile::ContactProfile;
    let out = resolve_profile(
        true,
        Some(&ContactProfile {
            email: Some("x@y.z".to_string()),
            ..Default::default()
        }),
    );
    let reply = profile_result_reply("req-99", out);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert!(
        v["payload"].get("extraLinks").is_none(),
        "absent extra_links must be OMITTED from the JSON (not an empty array) so an \
         old extension that has never heard of the key parses the reply unchanged"
    );
}

#[test]
fn profile_result_reply_carries_extra_links_camel_cased() {
    use crate::contact_profile::{ContactLink, ContactProfile};
    let out = resolve_profile(
        true,
        Some(&ContactProfile {
            extra_links: vec![ContactLink {
                label: "Portfolio".to_string(),
                url: "https://saeed.dev".to_string(),
            }],
            ..Default::default()
        }),
    );
    let reply = profile_result_reply("req-100", out);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["payload"]["extraLinks"][0]["label"], "Portfolio");
    assert_eq!(v["payload"]["extraLinks"][0]["url"], "https://saeed.dev");
}
