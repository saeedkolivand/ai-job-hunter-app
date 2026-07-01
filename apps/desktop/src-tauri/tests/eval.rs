//! Eval-corpus harness — Phase 1 skeleton.
//!
//! Loads the synthetic, no-PII fixtures under `tests/corpus/` and validates the
//! corpus is well-formed: every resume `.txt` has a `.tags` sidecar, every tag
//! line parses, the expected fields are present, and emails are synthetic.
//!
//! Field-level precision/recall against the real extractor is wired up in
//! Phase 6 — this stub just guards the corpus shape so later phases can rely on
//! it. (Standalone integration test: `ajh-tauri` is a binary crate, so this uses
//! only `std`, not the crate's internals.)

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// Parse a `.tags` sidecar: `key: value` lines; `#` comments and blanks ignored.
/// Repeatable keys (`section`, `link`) accumulate in order.
fn parse_tags(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            panic!("tag line {} is not `key: value`: {raw:?}", i + 1);
        };
        map.entry(k.trim().to_string())
            .or_default()
            .push(v.trim().to_string());
    }
    map
}

#[test]
fn corpus_fixtures_are_well_formed() {
    let dir = corpus_dir();
    let mut resumes = 0;

    for entry in fs::read_dir(&dir).expect("read tests/corpus directory") {
        let path = entry.expect("read dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        resumes += 1;

        let body = fs::read_to_string(&path).expect("read resume fixture");
        assert!(!body.trim().is_empty(), "{path:?} is empty");

        let tags_path = path.with_extension("tags");
        assert!(tags_path.exists(), "missing .tags sidecar for {path:?}");
        let tags = parse_tags(&fs::read_to_string(&tags_path).expect("read .tags sidecar"));

        // Every fixture declares at least a name, an email, and one section.
        for key in ["name", "email", "section"] {
            assert!(tags.contains_key(key), "{tags_path:?} tags missing `{key}`");
        }

        // No-PII guard: every declared email must be on a reserved example domain,
        // and must also literally appear in the resume body.
        for email in tags.get("email").into_iter().flatten() {
            assert!(
                email.contains("@example."),
                "fixture email must be synthetic: {email}"
            );
            assert!(
                body.contains(email.as_str()),
                "tagged email {email} not found in {path:?}"
            );
        }
    }

    assert!(
        resumes >= 2,
        "expected at least two corpus fixtures, found {resumes}"
    );
}
