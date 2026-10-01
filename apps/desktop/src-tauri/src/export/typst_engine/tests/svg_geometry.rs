//! typst-svg glyph/shape geometry helpers shared across the typst_engine test topics.

use super::fixtures::opts_a4;
use crate::export::templates::Template;
use crate::export::typst_engine::{render_resume_svg_pages, TypstTemplate};

//
// A registry field can claim "centred name" while the layout silently ignores
// it, so the tests below assert against the RENDERED page, not `data.style`.

/// Value of attribute `name` in an SVG start tag (`name="…"`), if present.
/// Matches on `" name=\""` — the leading space is what stops `x` from matching
/// `xlink:href` and `fill` from matching `fill-rule`.
pub(super) fn svg_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let len = tag[start..].find('"')?;
    Some(&tag[start..start + len])
}

/// Parse the translation component of a typst-svg `transform` attribute.
/// typst-svg only ever emits pure translations (`translate(x)` / `translate(x
/// y)`) and the baseline y-flip (`matrix(1 0 0 -1 e f)`), and never nests two
/// flips — so a glyph's page position is the running sum of these pairs plus its
/// own `x`.
///
/// **Panics on any other transform**, rather than the `(0.0, 0.0)` this used to
/// return. A silent zero here is the worst possible failure mode for a
/// measurement helper: every geometry assertion built on it (band containment,
/// centring, section offsets) keeps passing while measuring the wrong place. A
/// `scale(…)`/`rotate(…)`/general-`matrix(…)` group would also invalidate the
/// running-sum model itself, not just shift the origin — so the only correct
/// response is to stop and say the helper needs extending.
pub(super) fn svg_translation(transform: &str) -> (f64, f64) {
    let inner = transform
        .split_once('(')
        .and_then(|(_, rest)| rest.strip_suffix(')'))
        .unwrap_or_else(|| panic!("svg_translation: malformed transform {transform:?}"));
    let nums: Vec<f64> = inner
        .split([' ', ','])
        .filter(|t| !t.is_empty())
        // Strict: an unparseable component used to be dropped by `filter_map`,
        // which silently turned `translate(3 bogus)` into the 1-arg form.
        .map(|t| {
            t.parse::<f64>()
                .unwrap_or_else(|_| panic!("svg_translation: bad number {t:?} in {transform:?}"))
        })
        .collect();

    if let Some(kind) = transform.split('(').next() {
        match (kind, nums.len()) {
            ("translate", 2) => return (nums[0], nums[1]),
            // SVG's one-argument form: `translate(x)` means ty = 0. typst-svg
            // emits it whenever a group only shifts horizontally — dropping it
            // silently under-reports x by the whole shift.
            ("translate", 1) => return (nums[0], 0.0),
            ("matrix", 6) => {
                // Only the identity and the pure y-flip keep "page position =
                // running sum of translations" true. Anything else (a scale, a
                // rotation, a skew) also transforms the CHILD coordinates, which
                // this walker does not model at all.
                let linear = (nums[0], nums[1], nums[2], nums[3]);
                assert!(
                    linear == (1.0, 0.0, 0.0, 1.0) || linear == (1.0, 0.0, 0.0, -1.0),
                    "svg_translation: {transform:?} has a non-identity, non-y-flip \
                     linear part {linear:?} — it scales/rotates its children, so the \
                     running-sum model is invalid. Extend this helper (and every \
                     caller's assumptions) rather than measuring the wrong geometry."
                );
                return (nums[4], nums[5]);
            }
            _ => {}
        }
    }
    panic!(
        "svg_translation: unsupported transform {transform:?} — this helper models \
         only `translate(x)`, `translate(x y)` and `matrix(1 0 0 ±1 e f)`. Returning \
         zero here would silently corrupt every geometry assertion built on it."
    );
}

/// Walk a typst-svg document's start tags, tracking the cumulative `<g
/// transform>` translation, and hand every non-group tag to `visit` together
/// with the offset in force at that point.
///
/// Shared by [`glyph_positions`] and [`first_filled_rect_bottom`] so the
/// group-stack rules (self-closing groups, balance checks) exist once and cannot
/// drift between the two.
pub(super) fn walk_svg_tags(svg: &str, mut visit: impl FnMut(&str, (f64, f64)) -> bool) {
    let mut stack: Vec<(f64, f64)> = vec![(0.0, 0.0)];
    let mut rest = svg;
    while let Some(lt) = rest.find('<') {
        let after = &rest[lt + 1..];
        let Some(gt) = after.find('>') else { break };
        let tag = &after[..gt];
        rest = &after[gt + 1..];

        if tag == "/g" {
            assert!(
                stack.len() > 1,
                "walk_svg_tags: `</g>` with no matching `<g>` — the transform \
                 stack underflowed, so every later offset would be wrong"
            );
            stack.pop();
        } else if tag == "g" || tag.starts_with("g ") {
            // A SELF-CLOSING group (`<g … />`, which `xmlwriter` emits for a
            // group it never wrote children into) has no `</g>` to pop it.
            // Pushing it would leave the stack permanently deep and shift every
            // following sibling by its transform.
            if tag.ends_with('/') {
                continue;
            }
            let (dx, dy) = svg_attr(tag, "transform")
                .map(svg_translation)
                .unwrap_or((0.0, 0.0));
            let top = *stack.last().expect("transform stack is never empty");
            stack.push((top.0 + dx, top.1 + dy));
        } else if visit(tag, *stack.last().expect("transform stack is never empty")) {
            return;
        }
    }
    assert_eq!(
        stack.len(),
        1,
        "walk_svg_tags: {} unclosed `<g>` element(s) — the document is truncated \
         or the tag scanner lost sync",
        stack.len() - 1
    );
}

/// Every glyph in a typst-svg page as `(page_x, baseline_y, fill)`, in Typst
/// points with the page's top-left as the origin. Glyphs are `<use>` elements;
/// decorative `<path>` shapes (page background, rules, bars) are ignored.
pub(super) fn glyph_positions(svg: &str) -> Vec<(f64, f64, String)> {
    let mut out = Vec::new();
    walk_svg_tags(svg, |tag, (ox, oy)| {
        if tag.starts_with("use ") {
            let gx: f64 = svg_attr(tag, "x")
                .map(|v| {
                    v.parse()
                        .unwrap_or_else(|_| panic!("glyph_positions: bad `use` x={v:?}"))
                })
                .unwrap_or(0.0);
            // A `<use y=…>` would need the enclosing y-flip applied to it; the
            // walker only sums translations, so a non-zero one it silently
            // ignored would misplace the glyph vertically.
            let gy: f64 = svg_attr(tag, "y")
                .map(|v| {
                    v.parse()
                        .unwrap_or_else(|_| panic!("glyph_positions: bad `use` y={v:?}"))
                })
                .unwrap_or(0.0);
            assert_eq!(
                gy, 0.0,
                "glyph_positions: `<use y=\"{gy}\">` — glyph-local vertical offsets \
                 are not modelled (the enclosing y-flip would have to be applied); \
                 extend the helper instead of dropping it"
            );
            let fill = svg_attr(tag, "fill").unwrap_or("").to_string();
            out.push((ox + gx, oy, fill));
        }
        false
    });
    out
}

/// Width, in points, of the topmost baseline among the glyphs left of `max_x`
/// — first glyph origin to last glyph origin.
///
/// Reads the RENDERED TYPE SIZE, which glyph positions alone cannot: typst-svg
/// gives every glyph an explicit `x` advance, so the same string set in the same
/// font scales this figure linearly with its point size. Comparing the same
/// string in two renders therefore yields their size ratio directly, with no
/// hardcoded point values and no dependency on which glyphs typst-svg chose to
/// group.
///
/// `max_x` selects a column: pass the body's left edge to measure the sidebar
/// rail, or `f64::INFINITY` for the whole page.
pub(super) fn top_line_extent(glyphs: &[(f64, f64, String)], max_x: f64) -> f64 {
    let zone: Vec<&(f64, f64, String)> = glyphs.iter().filter(|(x, _, _)| *x < max_x).collect();
    if zone.is_empty() {
        return 0.0;
    }
    let top = zone
        .iter()
        .map(|(_, y, _)| *y)
        .fold(f64::INFINITY, f64::min);
    // 0.5pt tolerance: one baseline, not "roughly the top of the page".
    let xs: Vec<f64> = zone
        .iter()
        .filter(|(_, y, _)| (*y - top).abs() < 0.5)
        .map(|(x, _, _)| *x)
        .collect();
    let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    hi - lo
}

/// Bottom edge (page y, in points) of the first FULL-WIDTH rectangle painted in
/// `fill` — for a header-band template that is the band itself, drawn first as
/// the page background. Narrow accent shapes (section-marker bars) share the
/// fill, so anything under 100pt wide is skipped.
///
/// Reading the band out of the render instead of restating its constant is what
/// makes the containment assertions real: a test that compares glyphs against a
/// hardcoded band height silently keeps passing when the band shrinks.
pub(super) fn first_filled_rect_bottom(svg: &str, fill: &str) -> Option<f64> {
    let mut found = None;
    walk_svg_tags(svg, |tag, (_, oy)| {
        if !tag.starts_with("path ") {
            return false;
        }
        if !svg_attr(tag, "fill").is_some_and(|f| f.eq_ignore_ascii_case(fill)) {
            return false;
        }
        // `d="M x yv Hh WvΩ-HZ"` — the axis-aligned rect typst-svg emits for
        // a `rect(...)`: origin, height (v), width (h).
        let Some(d) = svg_attr(tag, "d") else {
            return false;
        };
        let nums: Vec<f64> = d
            .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
            .filter_map(|t| t.parse::<f64>().ok())
            .collect();
        if nums.len() >= 4 && nums[3].abs() >= 100.0 {
            found = Some(oy + nums[1] + nums[2]);
            return true; // stop the walk
        }
        false
    });
    found
}

/// Collapse [`glyph_positions`] to one entry per text line: `(baseline_y,
/// leftmost_x, rightmost_x)`, ordered top-to-bottom. Glyphs sharing a baseline
/// are one line.
pub(super) fn text_lines(svg: &str) -> Vec<(f64, f64, f64)> {
    let mut lines: Vec<(f64, f64, f64)> = Vec::new();
    for (x, y, _) in glyph_positions(svg) {
        match lines.iter_mut().find(|l| (l.0 - y).abs() < 0.01) {
            Some(line) => {
                line.1 = line.1.min(x);
                line.2 = line.2.max(x);
            }
            None => lines.push((y, x, x)),
        }
    }
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    lines
}

/// Render page 1 of `template` for `model` to SVG.
pub(super) fn svg_page1(
    model: &crate::model::document::DocumentModel,
    template: &Template,
    ats: bool,
) -> String {
    let mut opts = opts_a4();
    opts.ats = ats;
    render_resume_svg_pages(
        model,
        TypstTemplate::from_template(template),
        &opts,
        Some(template),
    )
    .unwrap_or_else(|e| {
        panic!(
            "render_resume_svg_pages({:?}) should succeed: {e:?}",
            template.id
        )
    })
    .into_iter()
    .next()
    .unwrap_or_else(|| panic!("{:?}: at least one SVG page", template.id))
}
