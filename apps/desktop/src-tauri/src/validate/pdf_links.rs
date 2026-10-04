//! Link annotations read back from a rendered PDF: where each one sits on the
//! page and where it points.

/// A link annotation read back from the rendered PDF: its `/Rect` in PDF user
/// space (points, bottom-up origin) and target URL.
#[derive(Debug)]
pub(super) struct PdfLink {
    /// `[x0, y0, x1, y1]` — bottom-left and top-right corners in points.
    pub(super) rect: [f32; 4],
    pub(super) url: String,
    /// 0-based page index the annotation lives on.
    pub(super) page: usize,
}

/// The `n` links sitting highest on the page, ordered top-down.
///
/// Selection is **geometric**, never `/Annots` emission order.
/// `page_link_annotations` yields annotations in array order, which carries no
/// guarantee about vertical position — and for a two-column template it very
/// likely doesn't, since a sidebar is emitted as its own run. Taking the first
/// `n` in emission order could therefore pick a *body* link, raise a CRITICAL
/// `header_url_mismatch`, and make `validate_and_fix` silently re-render the
/// document single-column — losing the user's chosen layout to a false
/// positive. Sorting by the same `rect` top edge the band filter already reads
/// makes "the header's own n links" a statement about the page, not about the
/// writer that produced it.
///
/// Pinned by `validate::tests::band_links::topmost_n_orders_by_geometry_not_annotation_order`.
pub(super) fn topmost_n<'a>(links: &[&'a PdfLink], n: usize) -> Vec<&'a PdfLink> {
    let top = |l: &PdfLink| l.rect[1].max(l.rect[3]);
    let mut by_position: Vec<&'a PdfLink> = links.to_vec();
    by_position.sort_by(|a, b| top(b).total_cmp(&top(a)));
    by_position.truncate(n);
    by_position
}

/// Read a page's `/Annots` entries as concrete dictionaries, handling BOTH the
/// inline-dictionary and the indirect-reference encodings — at the array level
/// and per element.
///
/// lopdf's own [`lopdf::Document::get_page_annotations`] keeps only entries that
/// are *indirect references* (`flat_map(Object::as_reference)`). Typst (our PDF
/// renderer) writes `/Annots` as an array of **inline dictionaries**, so that
/// helper returns nothing for every PDF we generate — which silently made the
/// header-link checks below see zero links and report every profile URL as
/// "missing", blocking any export that had a contact profile. Reading the array
/// ourselves keeps the validator working against Typst's output.
fn page_annot_dicts(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Vec<lopdf::Dictionary> {
    let Ok(page) = doc.get_dictionary(page_id) else {
        return Vec::new();
    };
    let array = match page.get(b"Annots") {
        Ok(lopdf::Object::Reference(id)) => doc.get_object(*id).and_then(|o| o.as_array()).ok(),
        Ok(lopdf::Object::Array(a)) => Some(a),
        _ => None,
    };
    let Some(array) = array else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|o| match o {
            lopdf::Object::Dictionary(d) => Some(d.clone()),
            lopdf::Object::Reference(id) => doc.get_dictionary(*id).ok().cloned(),
            _ => None,
        })
        .collect()
}

/// Collect `/Link` annotations (rect + `/A /URI`) for one page.
pub(super) fn page_link_annotations(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
    page_idx: usize,
) -> Vec<PdfLink> {
    let mut out = Vec::new();
    for annot in page_annot_dicts(doc, page_id) {
        let is_link = annot
            .get(b"Subtype")
            .and_then(|v| v.as_name())
            .map(|n| n == b"Link")
            .unwrap_or(false);
        if !is_link {
            continue;
        }
        let Some(rect) = annot
            .get(b"Rect")
            .ok()
            .and_then(|v| v.as_array().ok())
            .and_then(|a| {
                let v: Vec<f32> = a.iter().filter_map(|o| o.as_float().ok()).collect();
                <[f32; 4]>::try_from(v).ok()
            })
        else {
            continue;
        };
        let url = annot
            .get(b"A")
            .ok()
            .and_then(|a| match a {
                lopdf::Object::Dictionary(d) => Some(d.clone()),
                lopdf::Object::Reference(id) => doc.get_dictionary(*id).ok().cloned(),
                _ => None,
            })
            .and_then(|d| {
                d.get(b"URI")
                    .ok()
                    .and_then(|u| u.as_str().ok())
                    // Same PDF-text-string decode as the extractor: a UTF-16
                    // `/URI` read as UTF-8 is mojibake that no content check
                    // could match. Single decoder so the two can't drift.
                    .map(crate::extraction::pdf::pdf_text_string)
            });
        if let Some(url) = url {
            out.push(PdfLink {
                rect,
                url,
                page: page_idx,
            });
        }
    }
    out
}
