//! Cover-letter template preview generator for the README showcase.

use super::fixtures::canonical_template_ids;
use super::letter_fixtures::LETTER_FIXTURE_US;
use crate::export::templates::Template;
use crate::export::types::{LetterLayout, TemplateId};

/// Offline generator: one **cover-letter** style preview per résumé template.
///
/// `#[ignore]`d — an asset generator, not an assertion of behaviour. Run with:
///
/// ```text
/// cargo test --bin ajh-tauri -- --ignored generate_cover_template_previews
/// ```
///
/// This is the cover-letter analog of `generate_templates_showcase_banner`'s
/// per-template previews. For each of the same ten résumé templates it builds
/// the exact cover-letter Typst world that [`super::super::engine::render_letter_pdf`]
/// produces — `letter_style_from_template` derives the palette + fonts from the
/// résumé [`Template`], so the rendered letter *inherits that template's visual
/// style* — compiles page 1, and exports it to **SVG** (vector, no rasteriser,
/// no `image` crate, no thumbnailing). The ten `.svg` files feed the
/// AI-Generate cover-letter template picker (fetched lazily by the UI via a Vite
/// glob, mirroring the résumé `template-previews/` PNGs).
///
/// Offline hard-wall is respected: all `typst` / `typst_svg` types stay confined
/// to this test fn (same posture as the showcase test, which also imports typst
/// directly) — they never appear in production signatures. `typst-svg` is a
/// dev-dependency, never shipped in the binary.
#[test]
#[ignore]
fn generate_cover_template_previews() {
    use std::path::Path;
    use typst_layout::PagedDocument;

    use super::super::engine::{letter_scale_source, letter_source_for};
    use super::super::letter::{
        parse_cover_letter, style_from_template as letter_style_from_template,
    };
    use super::super::world::ResumeWorld;

    // Same twelve templates as the showcase generator. Slugs MUST match the
    // renderer's `TemplateId` wire ids so the preview files line up with the UI.
    let templates: &[(TemplateId, &str, &str)] = &[
        (TemplateId::Classic, "Classic", "classic"),
        (TemplateId::SwissMinimal, "SwissMinimal", "swiss-minimal"),
        (TemplateId::Academic, "Academic", "academic"),
        (TemplateId::Atelier, "Atelier", "atelier"),
        (TemplateId::Meridian, "Meridian", "meridian"),
        (TemplateId::Throughline, "Throughline", "throughline"),
        (TemplateId::Portrait, "Portrait", "portrait"),
        (TemplateId::Lebenslauf, "Lebenslauf", "lebenslauf"),
        (TemplateId::Cadence, "Cadence", "cadence"),
        (TemplateId::Regent, "Regent", "regent"),
        (TemplateId::Aria, "Aria", "aria"),
        (TemplateId::Saffron, "Saffron", "saffron"),
        (TemplateId::CologneNavy, "CologneNavy", "cologne-navy"),
        (TemplateId::Jake, "Jake", "jake"),
        (TemplateId::Awesome, "Awesome", "awesome"),
        (TemplateId::Deedy, "Deedy", "deedy"),
    ];
    assert_eq!(
        templates.len(),
        canonical_template_ids().len(),
        "cover previews must cover every canonical template"
    );

    // Embedded letter Typst sources, reused verbatim from production so the
    // preview matches `render_letter_pdf`. The gallery preview renders the
    // Classic layout — these previews answer "what does a letter styled like
    // THIS RÉSUMÉ TEMPLATE look like", which is the `style_from_template` axis;
    // the layout axis is orthogonal and is chosen separately in the picker.
    // Routed through the production picker (`letter_source_for`) rather than a
    // fixed tuple, which had gone stale at three layouts.
    let scale_typ = letter_scale_source();
    let letter_typ = letter_source_for(LetterLayout::Classic);

    let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/renderer/features/ai-generate/assets/cover-template-previews");
    std::fs::create_dir_all(&preview_dir)
        .unwrap_or_else(|e| panic!("cover previews: create_dir_all cover-template-previews: {e}"));

    let mut written = 0usize;

    for (id, label, slug) in templates {
        eprintln!("cover previews: rendering {label}...");

        // Build the letter world exactly like `render_letter_pdf`, inline.
        let t = Template::get(*id);
        let style = letter_style_from_template(&t);
        let model = parse_cover_letter(
            LETTER_FIXTURE_US,
            None,
            Some("Jane Smith"),
            "intl",
            "en",
            style,
            // Design mode: the gallery advertises what the layout looks like.
            false,
        );
        let data_json = serde_json::to_vec(&model)
            .unwrap_or_else(|e| panic!("cover previews: JSON serialise ({label}) failed: {e}"));

        let source = format!(
            "// Auto-generated cover-letter entry — do not edit.\n\
             #let data = json(\"data.json\")\n\
             {scale_typ}\n\
             {letter_typ}"
        );

        let world = ResumeWorld::with_data(&source, Some(data_json));

        // Compile to a PagedDocument (same pattern as the showcase generator).
        let warned = typst::compile::<PagedDocument>(&world);
        for w in &warned.warnings {
            eprintln!("cover previews typst warning [{w:?}]");
        }
        let document = warned.output.unwrap_or_else(|diags| {
            let msg: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
            panic!(
                "cover previews: typst compile error ({label}): {}",
                msg.join("; ")
            );
        });

        assert!(
            !document.pages().is_empty(),
            "cover previews: {label} produced zero pages"
        );

        // Export page 1 to SVG (vector — no rasterisation, no thumbnail).
        let svg: String = typst_svg::svg(&document.pages()[0], &typst_svg::SvgOptions::default());
        assert!(
            !svg.is_empty(),
            "cover previews: {label} produced an empty SVG"
        );
        assert!(
            svg.contains("<svg"),
            "cover previews: {label} SVG missing <svg root element"
        );

        let preview_path = preview_dir.join(format!("{slug}.svg"));
        std::fs::write(&preview_path, svg.as_bytes())
            .unwrap_or_else(|e| panic!("cover previews: write {}: {e}", preview_path.display()));

        written += 1;
        eprintln!("  → {} ({} bytes)", preview_path.display(), svg.len());
    }

    // Derived, not a literal: this assertion read `written == 10` while the list
    // already held twelve, so the generator could not be run at all without
    // editing it first — and being `#[ignore]`d, CI never noticed. Deriving it
    // means adding a template can never leave it stale again.
    assert_eq!(
        written,
        templates.len(),
        "cover previews: expected one SVG per template"
    );

    // Verify each exists and is non-trivial.
    for (_, label, slug) in templates {
        let p = preview_dir.join(format!("{slug}.svg"));
        let meta = std::fs::metadata(&p)
            .unwrap_or_else(|e| panic!("cover previews: {slug}.svg missing ({label}): {e}"));
        assert!(
            meta.len() > 0,
            "cover previews: {slug}.svg is empty ({label})"
        );
    }
    eprintln!(
        "cover-letter template previews written: {} → {}",
        written,
        preview_dir.display()
    );
}
