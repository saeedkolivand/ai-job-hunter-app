//! Shared PDF-introspection helpers + fixtures reused across the pdf test topics.

/// Recursively collect every `/URI` action target in a parsed PDF (link
/// annotations store the URL nested under the annotation's `/A` action dict).
pub(super) fn collect_uris(doc: &lopdf::Document) -> Vec<String> {
    fn from_dict(d: &lopdf::Dictionary, out: &mut Vec<String>) {
        if let Ok(u) = d.get(b"URI") {
            if let Ok(bytes) = u.as_str() {
                out.push(String::from_utf8_lossy(bytes).into_owned());
            }
        }
        for (_, v) in d.iter() {
            from_obj(v, out);
        }
    }
    fn from_obj(o: &lopdf::Object, out: &mut Vec<String>) {
        match o {
            lopdf::Object::Dictionary(d) => from_dict(d, out),
            lopdf::Object::Stream(s) => from_dict(&s.dict, out),
            lopdf::Object::Array(a) => a.iter().for_each(|v| from_obj(v, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for obj in doc.objects.values() {
        from_obj(obj, &mut out);
    }
    out
}

/// Collect every link annotation's `[x0,y0,x1,y1]` rect (points) + target URL.
/// Typst writes `/Annots` as **inline** dictionaries nested in the page object,
/// so we recurse through arrays/dicts (not just top-level objects).
pub(super) fn collect_link_rects(doc: &lopdf::Document) -> Vec<([f32; 4], String)> {
    fn from_dict(d: &lopdf::Dictionary, out: &mut Vec<([f32; 4], String)>) {
        let is_link = d
            .get(b"Subtype")
            .ok()
            .and_then(|v| v.as_name().ok())
            .map(|n| n == b"Link")
            .unwrap_or(false);
        if is_link {
            let rect = d
                .get(b"Rect")
                .ok()
                .and_then(|v| v.as_array().ok())
                .and_then(|a| {
                    let v: Vec<f32> = a.iter().filter_map(|o| o.as_float().ok()).collect();
                    <[f32; 4]>::try_from(v).ok()
                });
            let uri = match d.get(b"A") {
                Ok(lopdf::Object::Dictionary(ad)) => ad
                    .get(b"URI")
                    .ok()
                    .and_then(|u| u.as_str().ok())
                    .map(|b| String::from_utf8_lossy(b).into_owned()),
                _ => None,
            };
            if let (Some(rect), Some(uri)) = (rect, uri) {
                out.push((rect, uri));
            }
        }
        for (_, v) in d.iter() {
            from_obj(v, out);
        }
    }
    fn from_obj(o: &lopdf::Object, out: &mut Vec<([f32; 4], String)>) {
        match o {
            lopdf::Object::Dictionary(d) => from_dict(d, out),
            lopdf::Object::Stream(s) => from_dict(&s.dict, out),
            lopdf::Object::Array(a) => a.iter().for_each(|v| from_obj(v, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for obj in doc.objects.values() {
        from_obj(obj, &mut out);
    }
    out
}

/// A long contact profile (many links) used to exercise header wrapping in both
/// the résumé layout engine and the legacy cover-letter letterhead.
pub(super) fn long_contact_profile() -> crate::contact_profile::ContactProfile {
    use crate::contact_profile::{ContactLink, ContactProfile, LocalizedText};
    let extra = |label: &str, url: &str| ContactLink {
        label: label.to_string(),
        url: url.to_string(),
    };
    ContactProfile {
        location: Some(LocalizedText {
            default: "Amsterdam, Netherlands".to_string(),
            ..Default::default()
        }),
        email: Some("lena.vos@example.com".to_string()),
        phone: Some("+31 6 12345678".to_string()),
        linkedin: Some("https://www.linkedin.com/in/lena-vos/".to_string()),
        website: Some("https://drive.google.com/file/d/abc/view".to_string()),
        extra_links: vec![
            extra("Dribbble", "https://dribbble.com/lenavos"),
            extra("Behance", "https://behance.net/lenavos"),
            extra("Portfolio", "https://lena.example/portfolio"),
            extra("YouTube", "https://youtube.com/@lenavos"),
            extra("Instagram", "https://instagram.com/lenavos"),
            extra("Medium", "https://medium.com/@lenavos"),
        ],
        ..Default::default()
    }
}
