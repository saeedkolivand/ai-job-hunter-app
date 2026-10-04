use super::*;

#[test]
fn every_supported_extension_resolves() {
    for ext in [
        "pdf", "docx", "txt", "md", "markdown", "html", "htm", "rtf", "png", "jpg", "jpeg", "webp",
        "doc",
    ] {
        assert!(extractor_for(ext).is_some(), "no extractor for .{ext}");
    }
}

#[test]
fn unknown_extension_is_unregistered() {
    assert!(extractor_for("pages").is_none());
    assert!(extractor_for("").is_none());
}

#[test]
fn no_extension_is_claimed_twice() {
    let mut seen = std::collections::HashSet::new();
    for e in REGISTRY.iter() {
        for ext in e.extensions() {
            assert!(seen.insert(*ext), "extension .{ext} is registered twice");
        }
    }
}
