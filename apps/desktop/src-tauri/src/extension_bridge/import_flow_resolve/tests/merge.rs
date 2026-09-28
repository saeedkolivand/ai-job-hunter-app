use super::super::*;

use super::super::super::test_support::sample_posting;

/// `usable` is the title-gate the DOM-first import chain uses to decide whether a
/// parse degraded: a titled posting is usable; a blank/whitespace title is not
/// (→ handle_import persists a partial stub instead of erroring out).
#[test]
fn usable_requires_a_non_blank_title() {
    let url = "https://acme.example/jobs/1";
    assert!(
        usable(&sample_posting(url, "Co", "Title")),
        "a titled posting is usable"
    );
    assert!(
        !usable(&sample_posting(url, "Co", "")),
        "an empty-title posting is not usable"
    );
    assert!(
        !usable(&sample_posting(url, "Co", "   ")),
        "a whitespace-only title is not usable"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// A0b. merge_resolve_with_hint — the canonical-branch (LinkedIn search/list-view)
// hint-scoped fallback merge. `resolve()`'s non-empty title/description win; the
// `[data-ajh-job-root]` hint only fills a gap `resolve` left empty — never
// clobbers a value `resolve` already supplied. This is the fix for the LinkedIn
// search/collections import losing the JD: the canonical `resolve()` fetch
// commonly hits LinkedIn's authwall and comes back title-less or
// description-less, while the extension's captured DOM's hinted detail pane
// still has it. Deliberately scoped to the HINT only (never a whole-document
// `parse_from_html` merge) — see the wrong-job regression test below.
// ─────────────────────────────────────────────────────────────────────────────

fn posting_with(
    title: &str,
    company: &str,
    description: Option<&str>,
) -> crate::scraping::types::JobPosting {
    crate::scraping::types::JobPosting {
        description: description.map(String::from),
        ..sample_posting("https://acme.example/jobs/1", company, title)
    }
}

/// `resolve` is usable (a real title) but came back with no description
/// (the authwall degrade); the hint has one → the merged posting keeps
/// resolve's title/company but carries the hint's description. This is the
/// exact bug this fix closes.
#[test]
fn merge_resolve_with_hint_fills_a_missing_description_from_the_hint() {
    let resolved = posting_with("Staff Engineer", "Acme", None);

    let merged = merge_resolve_with_hint(
        Some(resolved),
        "Staff Engineer".to_string(),
        Some("Full job description text.".to_string()),
    )
    .expect("resolve present must merge to Some");

    assert_eq!(merged.title, "Staff Engineer", "resolve's title must win");
    assert_eq!(merged.company, "Acme", "company is untouched by the hint");
    assert_eq!(
        merged.description.as_deref(),
        Some("Full job description text."),
        "resolve's empty description must be filled in from the hint"
    );
}

/// `resolve` is unusable (blank title — the authwall degrade) but the hint
/// carries both title and description → they fill the gaps and the merged
/// posting clears `usable`; `company` stays whatever `resolve` produced (the
/// hint never supplies it).
#[test]
fn merge_resolve_with_hint_fills_title_and_description_when_resolve_is_unusable() {
    let resolved = posting_with("", "acme.example", None); // resolve's own host fallback company

    let merged = merge_resolve_with_hint(
        Some(resolved),
        "Senior Backend Engineer".to_string(),
        Some("Own the API platform.".to_string()),
    )
    .expect("resolve present must merge to Some");

    assert_eq!(merged.title, "Senior Backend Engineer");
    assert_eq!(
        merged.company, "acme.example",
        "company stays resolve's own value — the hint never supplies it"
    );
    assert_eq!(merged.description.as_deref(), Some("Own the API platform."));
    assert!(usable(&merged), "the merged posting must now be usable");
}

/// Both sides empty (no title/description anywhere) → the merge stays empty,
/// so `handle_import`'s stub-persist path still triggers exactly as before
/// this fix — the merge can never manufacture usability out of nothing.
#[test]
fn merge_resolve_with_hint_both_empty_stays_unusable() {
    let resolved = posting_with("", "", None);

    let merged = merge_resolve_with_hint(Some(resolved), String::new(), None)
        .expect("resolve present (even if unusable) still merges to Some");

    assert!(!usable(&merged));
    assert!(merged.description.is_none());
}

/// `resolve` already has a real description → the hint's (possibly different)
/// description must NOT clobber it — a fallback field can only fill a gap,
/// never override a value the primary source already supplied.
#[test]
fn merge_resolve_with_hint_never_clobbers_a_resolve_description_that_is_already_present() {
    let resolved = posting_with("Staff Engineer", "Acme", Some("Resolve's own description."));

    let merged = merge_resolve_with_hint(
        Some(resolved),
        "Staff Engineer".to_string(),
        Some("A different hint description.".to_string()),
    )
    .unwrap();

    assert_eq!(
        merged.description.as_deref(),
        Some("Resolve's own description."),
        "resolve's non-empty description must win over the hint's"
    );
}

/// No `resolve` posting at all → `None` — there is no base identity
/// (id/url/source/company) to attach the hint to, so the stub/partial path
/// covers this case instead of synthesizing a posting from the hint alone.
#[test]
fn merge_resolve_with_hint_none_resolve_is_none() {
    assert!(merge_resolve_with_hint(
        None,
        "Some Title".to_string(),
        Some("Some description.".to_string())
    )
    .is_none());
}

/// HIGH regression (wrong-job import via whole-document JSON-LD on list
/// shells): a list-shell fixture carries a `data-ajh-job-root` pane for the
/// SELECTED job PLUS unrelated SEO `JobPosting` JSON-LD (LinkedIn search pages
/// render this for the first result). `job_root_generic_html` — the extraction
/// the canonical branch actually uses — must yield the pane's own
/// title/description, not the JSON-LD's; merging that hint onto an unusable
/// `resolve` posting must therefore import the pane's job, never the
/// unrelated one. Contrast: the whole-document `parse_from_html` on this SAME
/// html WOULD adopt the JSON-LD job — pinning exactly why the canonical
/// branch must never call it on a list shell.
#[test]
fn canonical_branch_hint_scoped_fallback_ignores_unrelated_whole_document_json_ld() {
    let html = r#"
        <html>
        <head>
            <title>Jobs at LinkedIn</title>
            <script type="application/ld+json">
            {
                "@context": "https://schema.org/",
                "@type": "JobPosting",
                "title": "WRONG: Marketing Intern (first list result)",
                "description": "This is an UNRELATED job from the list shell's own SEO markup.",
                "hiringOrganization": { "name": "Wrong Corp" }
            }
            </script>
        </head>
        <body>
            <ul class="jobs-search__results-list">
                <li><a>Marketing Intern</a></li>
                <li><a>Staff Backend Engineer</a></li>
            </ul>
            <div class="jobs-details" data-ajh-job-root="true">
                <h1>Staff Backend Engineer</h1>
                <p>Own the API platform end to end at Acme, the SELECTED job.</p>
            </div>
        </body>
        </html>
    "#;

    // The whole-document parse WOULD adopt the wrong (JSON-LD) job — this is
    // exactly the risk the canonical branch's hint-scoping avoids.
    let whole_doc =
        crate::scraping::scrape_url::parse_from_html("https://www.linkedin.com/jobs/view/1", html)
            .expect("a titled JSON-LD document always yields Some");
    assert_eq!(
        whole_doc.title, "WRONG: Marketing Intern (first list result)",
        "sanity check: parse_from_html's own precedence lets JSON-LD beat the hint"
    );

    // The canonical branch's actual extraction: hint-scoped only.
    let (hint_title, hint_description) = crate::scraping::scrape_url::job_root_generic_html(html)
        .expect("a well-formed data-ajh-job-root pane must yield a hint");
    assert_eq!(
        hint_title, "Staff Backend Engineer",
        "the hint-scoped extraction must read the pane, not the JSON-LD"
    );
    assert!(
        hint_description
            .as_deref()
            .unwrap_or_default()
            .contains("SELECTED job"),
        "hint description must come from the pane, not the JSON-LD"
    );

    // An unusable `resolve()` (authwalled) merged with that hint must import
    // the pane's job — never the unrelated JSON-LD one.
    let resolved = posting_with("", "linkedin.com", None);
    let merged = merge_resolve_with_hint(Some(resolved), hint_title, hint_description)
        .expect("resolve present must merge to Some");
    assert_eq!(merged.title, "Staff Backend Engineer");
    assert!(
        merged
            .description
            .as_deref()
            .unwrap_or_default()
            .contains("SELECTED job"),
        "merged description must be the pane's, not the unrelated JSON-LD's"
    );
    assert_ne!(
        merged.title, "WRONG: Marketing Intern (first list result)",
        "the merged posting must never be the unrelated list-shell job"
    );
}
