//! Render-level checks over the actual PDF bytes: the header's own links, a
//! job-board host in the header band.

use std::collections::BTreeSet;

use crate::export::types::{DocumentType, ExportRequest};

use super::pdf_links::{page_link_annotations, topmost_n, PdfLink};
use super::ExportIssue;

/// Light URL canonicalization for the header-URL mismatch check.
///
/// Normalizes trivial differences that don't change the identity of a URL:
/// - lowercase scheme and host (the authority is case-insensitive per RFC 3986)
/// - strip a single trailing slash from the path (never from the query/fragment)
/// - decode a percent-encoded space (`%20` → space), the most common encoding
///   divergence between stored profile values and rendered PDF annotations
///
/// Intentionally conservative: the check's goal is to catch a genuinely wrong
/// URL (a company link leaking into the header), not to canonicalize semantics.
pub(super) fn canonicalize_url(url: &str) -> String {
    // Split off scheme+authority (both case-insensitive per RFC 3986).
    // Authority ends at the first '/', '?', or '#' — NOT just the first '/'.
    // Without this, `https://example.com?Token=ABC` wrongly treats `?Token=ABC`
    // as part of the authority and lowercases the query string.
    let (prefix, path_query_fragment) =
        if let Some(after_scheme_start) = url.find("://").map(|i| i + 3) {
            let after = &url[after_scheme_start..];
            // End of authority = first of '/', '?', '#' (or end of string).
            let auth_end = after.find(['/', '?', '#']).unwrap_or(after.len());
            let scheme_and_auth = &url[..after_scheme_start + auth_end];
            let rest = &url[after_scheme_start + auth_end..];
            (scheme_and_auth.to_lowercase(), rest.to_string())
        } else {
            (String::new(), url.to_string())
        };

    // Separate the path from any query/fragment so we only strip a trailing
    // slash from the path, never from inside the query or fragment.
    let path_query_fragment = {
        // Find where path ends (first '?' or '#').
        let qf_start = path_query_fragment
            .find(['?', '#'])
            .unwrap_or(path_query_fragment.len());
        let path = &path_query_fragment[..qf_start];
        let query_fragment = &path_query_fragment[qf_start..];
        // Strip at most one trailing slash from the path.
        let path = path.strip_suffix('/').unwrap_or(path);
        format!("{path}{query_fragment}")
    };

    // Decode a percent-encoded space — the most common trivial encoding mismatch.
    let path_query_fragment = path_query_fragment.replace("%20", " ");

    format!("{prefix}{path_query_fragment}")
}

/// `header_url_mismatch` (critical): a link at the header's OWN position that is
/// not one of the `allowed` URLs. `owner` names who claims them in the message.
/// An empty `allowed` owns nothing, so nothing is checked (see the invariant in
/// [`pdf_render_issues`]).
fn mismatch_issues(
    header_links: &[&PdfLink],
    allowed: &BTreeSet<String>,
    owner: &str,
) -> Vec<ExportIssue> {
    if allowed.is_empty() {
        return Vec::new();
    }
    let allowed_canonical: BTreeSet<String> = allowed.iter().map(|u| canonicalize_url(u)).collect();
    topmost_n(header_links, allowed.len())
        .into_iter()
        .filter(|link| !allowed_canonical.contains(&canonicalize_url(&link.url)))
        .map(|link| {
            ExportIssue::critical(
                "header_url_mismatch",
                format!(
                    "Header link {} is not one of the {owner} — a body/company link leaked into the header.",
                    link.url
                ),
            )
        })
        .collect()
}

/// `header_url_missing` (warning): each of `urls` that surfaced nowhere in the
/// full header band. Advisory only — see [`pdf_render_issues`].
fn missing_link_issues<'a>(
    header_links: &[&PdfLink],
    urls: impl Iterator<Item = &'a String>,
) -> Vec<ExportIssue> {
    urls.filter(|url| {
        !header_links
            .iter()
            .any(|l| canonicalize_url(&l.url) == canonicalize_url(url))
    })
    .map(|url| {
        ExportIssue::warning(
            "header_url_missing",
            format!("Contact profile link {url} is missing from the rendered header."),
        )
    })
    .collect()
}

/// Render-level checks over the actual PDF bytes (renderer-agnostic, so the modern
/// résumé engine and the legacy cover-letter path are both covered):
///   * `header_url_mismatch` (critical) — a self-consistency check: every
///     link at the header's OWN position must be one of the document's own,
///     actually-authoritative header links (catches a company-link
///     displacing a personal profile / site, or a render/model
///     desynchronization). For a résumé this is whichever side (text or
///     profile) actually supplied the header — not only the profile;
///     narrowed to the header's own expected link COUNT (not the whole
///     144pt band) so a genuine, correctly-placed body link near the top of
///     page 1 is never mistaken for a header link. For a cover letter it is
///     always the profile, narrowed the SAME way (the topmost
///     `allowed.len()` band links): the letterhead (name/contact) always
///     renders before date/recipient/salutation/body, so its own links are
///     always the topmost ones — but a markdown link inside the OPENING
///     PARAGRAPH (e.g. a company-research URL the AI echoes in its first
///     sentence) can still land inside the same 144pt band as a short
///     letterhead, and a cover letter has no `can_linearize` remediation for
///     a false block (never two-column).
///   * `header_url_missing` (warning) — the reverse completeness check (a
///     profile link that did not surface anywhere in the full header band,
///     only checked when the profile is what supplied the header); advisory
///     only, as it leans on the band heuristic and a missing link never
///     corrupts the document.
///   * `header_url_job_board` (warning) — a job-board/ATS host anywhere in
///     the full header band, whoever put it there, résumé or cover letter;
///     exempts a personal Xing profile.
pub(super) fn pdf_render_issues(request: &ExportRequest, bytes: &[u8]) -> Vec<ExportIssue> {
    let doc = match lopdf::Document::load_mem(bytes) {
        Ok(d) => d,
        Err(e) => {
            // Tooling failure never blocks — debug only, this is diagnostic
            // noise a valid export will never hit.
            log::debug!("export: lopdf failed to parse rendered PDF bytes: {e}");
            return Vec::new();
        }
    };
    let page_h_pt = request.page_geometry().height_mm * 2.834_645_7;

    let pages: Vec<(usize, lopdf::ObjectId)> = doc.get_pages().into_values().enumerate().collect();
    let mut links: Vec<PdfLink> = Vec::new();

    for (idx, page_id) in &pages {
        links.extend(page_link_annotations(&doc, *page_id, *idx));
    }

    let mut issues = Vec::new();

    // The former `empty_anchor_link` geometric check (link rect vs text-baseline
    // overlap) was removed at the Typst cutover: it guarded the legacy renderer's
    // manually-placed link rects (coordinate-origin flips). Typst's
    // `link(url, body)` wraps real glyphs, so an annotation is structurally always
    // anchored to its text; the geometric approach was renderer-fragile (text-matrix
    // vs /Rect coordinate spaces) and false-flagged every valid Typst link. The
    // URL-correctness checks below (content-based, not geometric) still run.

    // A self-consistency check against the header ACTUALLY rendered — not a
    // profile-parity check, and not skipped for any document shape (see ADR
    // 0021's "Export validation" section for the full rationale/history).
    //
    // H — the editor is the source of truth: `ContactProfile::apply_to_header`
    // fills the header's contact line from the profile ONLY when the
    // text-derived header has none. `allowed` below is built from whichever
    // header runs are actually authoritative for THIS render (reconstructed
    // by running the SAME `model_from_resume_text` → `apply_to_header`
    // pipeline `prepare_resume_render` uses), not only the profile's fields —
    // so the strict block runs unconditionally for résumés, text-owned
    // header or not, profile supplied or not. Cover letters are unaffected
    // (their header override wasn't part of H) and keep the original,
    // profile-only form.
    //
    // Parses the SAME text `prepare_resume_render` actually renders from, not
    // the raw `request.text` — which can still carry the
    // "### CANDIDATE RESUME ###" / "### JOB ADVERTISEMENT ###" wrapper. Left
    // un-extracted, `parse_resume` reads that marker as an ATX heading at line
    // 0 (`strip_atx_heading` runs before the idx==0 name/contact case),
    // `seen_section` flips true immediately, and the real name/contact lines
    // never reach `header.contact` at all.
    let resume_text = (request.document_type == DocumentType::Resume).then(|| {
        let extracted = crate::export::pdf::extract_section(
            &request.text,
            "### CANDIDATE RESUME ###",
            Some("### JOB ADVERTISEMENT ###"),
        );
        if extracted.is_empty() {
            request.text.as_str()
        } else {
            extracted
        }
    });
    // `(profile_is_header_source, header)` — the boolean is recorded from the
    // text-only parse, BEFORE the profile fallback runs, so it still answers
    // "did the profile actually supply this header's contact line" for the
    // completeness/"missing" check below (only a meaningful signal relative
    // to a profile that was actually applied).
    let resume_header = resume_text.map(|t| {
        let mut model = crate::model::adapter::model_from_resume_text(t);
        let profile_is_header_source = model.header.contact.is_empty();
        if let Some(profile) = request.contact.as_ref() {
            profile.apply_to_header(&mut model.header, &request.target_lang());
        }
        (profile_is_header_source, model.header)
    });

    // Header region: the top ~2 inches (144 pt) of the first page. A
    // HEURISTIC, not a semantic boundary — a short header followed
    // immediately by a section (EXPERIENCE, say) can put a genuine, correctly
    // placed BODY link (a job's own company site) geometrically inside this
    // band. `header_links` (every band-falling annotation) still backs the
    // advisory completeness/job-board checks below, where a false positive
    // only produces a non-blocking warning; the BLOCKING mismatch check uses
    // `header_owned_links` instead (narrower — see there).
    let header_band_bottom = page_h_pt - 144.0;
    let header_links: Vec<&PdfLink> = links
        .iter()
        .filter(|l| l.page == 0 && l.rect[1].max(l.rect[3]) >= header_band_bottom)
        .collect();

    match &resume_header {
        Some((profile_is_header_source, header)) => {
            // `allowed`: the URLs the document's own, actually-authoritative
            // header claims — the reconstructed header's own link runs,
            // whichever side (text or profile) supplied them.
            let allowed: BTreeSet<String> = header
                .contact
                .iter()
                .filter_map(|r| r.link.clone())
                .collect();

            // CodeRabbit (security re-review): the mismatch check below must
            // NOT run over the full `header_links` band — the band is a
            // geometric heuristic (see above), and a genuine body link
            // rendering early on the page (a short header + an immediate
            // section) is not "the header's" just because it falls inside
            // the same 144 pt zone; flagging it there is exactly the
            // false-block this check must never produce. Narrowed to
            // `header_owned_links`: the FIRST `allowed.len()` band links, in
            // the PDF's own annotation order — which tracks top-to-bottom
            // render order for a normal single-column document, so this is
            // "the header's own N links, whichever N are actually
            // expected," not "everything that happens to be nearby." A
            // document whose header renders fewer links than expected (the
            // band clipped a wrapped line) simply checks fewer — never more
            // than are legitimately the header's.
            //
            // A SECOND reviewer (post-push, round 8) read `take(allowed.len())`
            // silently degrading to zero checks when `allowed` is empty as an
            // unintentional hole and proposed falling back to the FULL band
            // in that case. Verified empirically before writing this: that
            // fallback is wrong and would reintroduce the exact false-block
            // this round removed — a name-only header with no profile and no
            // email/phone-with-a-link produces `allowed = {}` (a phone/
            // location line never gets a `.link` run; a bare name never
            // does either), and a job's own company link rendering early on
            // the page (short header, immediate EXPERIENCE section) would be
            // flagged as `header_url_mismatch` and BLOCK a perfectly valid
            // export, on a fresh un-narrowed-band check identical in kind to
            // the one this round already fixed.
            //
            // INVARIANT, made explicit rather than left as an accident of
            // `take(0)`: an empty `allowed` means the reconstructed header —
            // the one the renderer will actually emit — owns NO links at
            // all. `header.contact`'s runs carry zero `.link` values (no
            // email, no LinkedIn/GitHub/Website, no extra link), and
            // `header.name` is a plain `String` that can never carry one
            // either. There is therefore nothing legitimately "the
            // header's" to check a band link against; every link in the
            // band in this state is body content, full stop, and must not
            // block. Pinned by `validate::tests::band_links::a_genuine_body_link_in_the_header_band_never_false_blocks_a_resume`.
            //
            // A header-owned link that is NOT one of the header's own
            // claimed links means the render itself disagrees with the
            // reconstructed model (the URL-swap regression: the document
            // shows a wrong link where its own header should be) — this
            // stays blocking, unconditionally (no profile required).
            issues.extend(mismatch_issues(
                &header_links,
                &allowed,
                "document header's own links",
            ));

            // The reverse (a profile link that did not surface in the header
            // band) is a *completeness* signal that depends on the 144 pt
            // band heuristic, so it is advisory — checked against the FULL
            // `header_links` (not narrowed: "did this expected link render
            // somewhere in the band at all" is a different question than
            // "is this band link one of the header's own") — and only
            // meaningful when the profile actually supplied this header
            // (comparing an unrelated profile's links against a text-owned
            // header would be a false, unactionable "missing" for every one
            // of them).
            if *profile_is_header_source {
                if let Some(profile) = request
                    .contact
                    .as_ref()
                    .filter(|p| !p.is_effectively_empty())
                {
                    issues.extend(missing_link_issues(
                        &header_links,
                        profile.header_urls().iter(),
                    ));
                }
            }
        }
        None => {
            // Cover letter: H doesn't apply — the header always comes
            // straight from the profile (there is no text-derived header to
            // prefer), so this keeps the original, simpler profile-parity
            // form. The strict checks genuinely need a profile to compare
            // against — no profile (or an empty one) means nothing to check.
            //
            // Narrowed the SAME way the résumé arm is (`topmost_n`) — a
            // production incident (a macOS user's PDF export blocked with
            // no remediation) traced back to this arm running unnarrowed
            // over the full band. The original reasoning here ("no body
            // SECTION can render early into a cover letter's header-band
            // the way a résumé's can") is true but incomplete: it rules out
            // a body *section* racing the header, not a body *link* inside
            // the opening paragraph itself. `parse_cover_letter` parses the
            // letterhead first and the body last (letterhead → date →
            // recipient → salutation → body, in that order), so the
            // letterhead's own links always render ABOVE the body — the
            // same "topmost N are the header's own" invariant the résumé
            // arm already relies on holds here too. A markdown link the AI
            // echoes in the opening sentence (e.g. from a company-research
            // brief) can still land inside the same 144pt band as a short
            // letterhead (no date/recipient lines to push it down), and a
            // cover letter is never two-column — `can_linearize` is always
            // false, so a false block here had NO remediation for the user.
            // Pinned by
            // `validate::tests::band_links::cover_letter_body_link_in_a_short_letterhead_band_does_not_false_block`.
            if let Some(profile) = request
                .contact
                .as_ref()
                .filter(|p| !p.is_effectively_empty())
            {
                let allowed: BTreeSet<String> = profile.header_urls().into_iter().collect();

                // Same empty-`allowed` invariant as the résumé arm: a
                // profile can be non-empty (e.g. phone/location only, which
                // `header_urls` never turns into a link) yet supply zero
                // links, in which case nothing in the band is legitimately
                // "the letterhead's" and every link there is body content.
                issues.extend(mismatch_issues(
                    &header_links,
                    &allowed,
                    "contact profile's fields",
                ));
                issues.extend(missing_link_issues(&header_links, allowed.iter()));
            }
        }
    }

    // A job-board/ATS host is never a legitimate personal contact link,
    // whoever put it there and whichever document type this is — checked
    // unconditionally: not gated on a profile being present (the people most
    // likely to export a raw imported header untouched are exactly the ones
    // who never filled one in), and now hoisted above the résumé/cover-letter
    // match (LOW, security re-review — it used to run only in the résumé
    // arm, silently never firing for a cover letter's own job-board host,
    // contradicting this warning's "unconditional" design intent). Runs
    // against the FULL band, same as the missing-completeness check: a
    // job-board URL that IS part of the header's own claimed links (the
    // user's own header literally contains one) passes the mismatch check
    // above by construction but must still warn here. Exempts a personal
    // Xing profile (`/profile/…`) — `xing.com` is also a job-board host (it
    // lists job postings too), so without the exemption a legitimate DACH
    // candidate's own profile link would warn every time.
    for link in &header_links {
        if crate::contact_profile::is_job_board(&link.url)
            && !crate::contact_profile::is_personal_xing(&link.url)
        {
            issues.push(ExportIssue::warning(
                "header_url_job_board",
                format!(
                    "Header link {} looks like a job board/ATS URL, not a personal contact link.",
                    link.url
                ),
            ));
        }
    }

    issues
}
