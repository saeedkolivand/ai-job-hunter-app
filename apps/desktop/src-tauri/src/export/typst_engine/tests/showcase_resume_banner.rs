//! README showcase banner generator (résumé templates).

// `Template`/`TemplateId`/`TypstTemplate`/`PageGeometry`/`model_from_resume_text` are
// NOT imported here: the function body below carries its own complete inline `use` block
// (unchanged from the original file) that already shadow-imports every one of them.
use super::fixtures::canonical_template_ids;
use super::resume_fixtures::SHOWCASE_FIXTURE;
use crate::export::typst_engine::RenderOpts;

#[test]
#[ignore]
fn generate_templates_showcase_banner() {
    use image::{DynamicImage, GenericImage, ImageBuffer, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;
    use std::path::Path;
    use typst_layout::PagedDocument;
    use typst_render::{render as typst_rasterise, RenderOptions};

    use super::super::engine::TypstTemplate;
    use super::super::render::{prepare, prepare_with_photo, PreparedRender};
    use super::super::world::ResumeWorld;
    use crate::export::templates::Template;
    use crate::export::types::TemplateId;
    use crate::locale::PageGeometry;
    use crate::model::adapter::model_from_resume_text;

    // ── Layout constants ──────────────────────────────────────────────────────

    /// Pixels per Typst point at "2×" / 144 dpi.
    /// One Typst point = 1/72 inch → 144 dpi = 2.0 px/pt.
    const PIXEL_PER_PT: f32 = 2.0;

    /// Each thumbnail is scaled to exactly this width (px); height is derived
    /// from the original A4 aspect ratio.
    const CELL_W: u32 = 300;

    // Layout: a single wide row — one column per template × 1 row (banner
    // proportions). `ROWS` is 1, so every template must fit on that row; a
    // template landing at row-index >= 1 would write pixels beyond `canvas_h`
    // (an out-of-bounds `put_pixel` panic below). One row keeps the grid math
    // trivial (`col = idx % cols`, `row = idx / cols = 0`).
    //
    // Derived, not a literal: a hardcoded 12 here made the canvas one cell too
    // narrow the moment a 13th template landed, and the composition panicked
    // with an out-of-bounds pixel write rather than saying so.
    let cols: u32 = canonical_template_ids().len() as u32;
    const ROWS: u32 = 1;

    /// Outer border padding (px) and gap between cells (px).
    const PADDING: u32 = 20;
    const GAP: u32 = 14;

    /// Thin 1 px border drawn around each cell (colour: #C8C8CA mid-grey).
    const BORDER: u32 = 1;
    const BORDER_R: u8 = 200;
    const BORDER_G: u8 = 200;
    const BORDER_B: u8 = 202;

    /// Background colour: #F4F4F5 (very light warm grey).
    const BG_R: u8 = 0xF4;
    const BG_G: u8 = 0xF4;
    const BG_B: u8 = 0xF5;

    // ── A4 page geometry for rendering ────────────────────────────────────────

    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: None,
        lang: "en".to_string(),
        ats: false,
    };

    // ── Template list (must be exactly 12, matching the canonical TemplateId set) ──

    // (TemplateId, human label, kebab slug). The slug MUST match the renderer's
    // `TemplateId` wire ids so the per-template preview files line up with the UI.
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
        "showcase must cover every canonical template"
    );

    // ── Helper: compile a World to a PagedDocument ────────────────────────────

    let compile_world = |world: &ResumeWorld| -> PagedDocument {
        let warned = typst::compile::<PagedDocument>(world);
        for w in &warned.warnings {
            eprintln!("showcase typst warning [{w:?}]");
        }
        warned.output.unwrap_or_else(|diags| {
            let msg: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
            panic!("showcase: typst compile error: {}", msg.join("; "));
        })
    };

    // ── Helper: Pixmap → RgbaImage ────────────────────────────────────────────
    //
    // `typst_render::render` returns a `tiny_skia::Pixmap` whose `.data()`
    // is a flat &[u8] in premultiplied RGBA byte order.  Resume templates
    // render on a white background so virtually all pixels are fully opaque
    // (alpha = 255), meaning premultiplied == straight for those pixels.
    // For the handful of anti-aliased edge pixels the visual difference is
    // imperceptible at 420 px thumbnail width, so we copy the raw bytes
    // directly without the overhead of a per-pixel un-premultiply pass.
    // This also avoids a direct `tiny_skia` dev-dependency.

    let pixmap_to_rgba = |pxw: u32, pxh: u32, raw: Vec<u8>| -> RgbaImage {
        RgbaImage::from_raw(pxw, pxh, raw).expect("showcase: pixmap_to_rgba: buffer size mismatch")
    };

    // ── Render + rasterise each template ─────────────────────────────────────

    let model = model_from_resume_text(SHOWCASE_FIXTURE);

    // A4 at 2 px/pt → height of one cell thumbnail.
    // A4: 210 mm wide × 297 mm tall. Typst uses 1pt = 0.352778 mm,
    // so 210 mm = ~595.28 pt → 595.28 * 2 ≈ 1190 px wide before thumbnail.
    // After thumbnail to CELL_W=300: height = 300 * (297/210) ≈ 424 px.
    let a4_aspect = 297.0_f32 / 210.0_f32;
    let cell_h = (CELL_W as f32 * a4_aspect).round() as u32;

    // Per-template preview SVGs for the AI-Generate option previews. Written into
    // the renderer's feature assets (the UI imports them via a Vite glob).
    let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/renderer/features/ai-generate/assets/template-previews");
    std::fs::create_dir_all(&preview_dir)
        .unwrap_or_else(|e| panic!("showcase: create_dir_all template-previews: {e}"));

    let mut thumbnails: Vec<RgbaImage> = Vec::with_capacity(12);

    for (id, label, slug) in templates {
        eprintln!("showcase: rendering {label}...");

        let t = Template::get(*id);
        let typst_tmpl = TypstTemplate::from_template(&t);
        let source = typst_tmpl.source_with_scale();

        // Photo templates (Portrait, Lebenslauf, Aria, Saffron) take the photo-
        // capable prepare path but render their no-photo fallback so the showcase
        // generator has no binary dependency.
        let has_photo = matches!(
            id,
            TemplateId::Portrait | TemplateId::Lebenslauf | TemplateId::Aria | TemplateId::Saffron
        );

        let PreparedRender {
            source: compiled_source,
            data_json,
        } = if has_photo {
            prepare_with_photo(&model, &source, &opts, Some(&t), false)
                .unwrap_or_else(|e| panic!("showcase: prepare_with_photo({label}) failed: {e}"))
        } else {
            prepare(&model, &source, &opts, Some(&t))
                .unwrap_or_else(|e| panic!("showcase: prepare({label}) failed: {e}"))
        };

        let world = ResumeWorld::with_data(&compiled_source, Some(data_json));
        let document = compile_world(&world);

        assert!(
            !document.pages().is_empty(),
            "showcase: {label} produced zero pages"
        );

        // `render` gained an options parameter in typst 0.15; `pixel_per_pt`
        // moved onto `RenderOptions` (its default is already 2.0 = this scale).
        let render_opts = RenderOptions {
            pixel_per_pt: typst::utils::Scalar::new(f64::from(PIXEL_PER_PT)),
            render_bleed: false,
        };
        let pixmap = typst_rasterise(&document.pages()[0], &render_opts);
        let (pxw, pxh) = (pixmap.width(), pixmap.height());
        let raw = pixmap.data().to_vec();
        let rgba = pixmap_to_rgba(pxw, pxh, raw);

        // Per-template preview SVG (vector page-1 export) for the UI picker —
        // crisp at any zoom, a fraction of the old PNG's size, and self-contained
        // (Typst exports glyphs as paths, so there is no font dependency at display time).
        let svg: String = typst_svg::svg(&document.pages()[0], &typst_svg::SvgOptions::default());
        assert!(
            svg.contains("<svg"),
            "showcase: {label} preview SVG missing <svg root element"
        );
        let preview_path = preview_dir.join(format!("{slug}.svg"));
        std::fs::write(&preview_path, svg.as_bytes())
            .unwrap_or_else(|e| panic!("showcase: write preview {}: {e}", preview_path.display()));

        // Thumbnail to CELL_W × cell_h.
        let thumb = DynamicImage::ImageRgba8(rgba)
            .thumbnail(CELL_W, cell_h)
            .to_rgba8();

        let (tw_cur, th_cur) = (thumb.width(), thumb.height());
        thumbnails.push(thumb);
        eprintln!("  → thumbnail {tw_cur}×{th_cur}");
    }

    assert_eq!(
        thumbnails.len(),
        canonical_template_ids().len(),
        "must have one thumbnail per canonical template"
    );

    // ── Compose single wide row (1×10) ────────────────────────────────────────

    // Use the actual thumbnail dimensions (thumbnail() preserves aspect, so
    // width should be CELL_W and height close to cell_h).
    let tw = thumbnails[0].width();
    let th = thumbnails[0].height();

    // Canvas size:
    //   width  = PADDING + COLS*(BORDER + tw + BORDER) + (COLS-1)*GAP + PADDING
    //   height = PADDING + ROWS*(BORDER + th + BORDER) + (ROWS-1)*GAP + PADDING
    let canvas_w = PADDING + cols * (2 * BORDER + tw) + (cols - 1) * GAP + PADDING;
    let canvas_h = PADDING + ROWS * (2 * BORDER + th) + (ROWS - 1) * GAP + PADDING;

    let bg_pixel = Rgba([BG_R, BG_G, BG_B, 255u8]);
    let border_pixel = Rgba([BORDER_R, BORDER_G, BORDER_B, 255u8]);

    let mut canvas: RgbaImage = ImageBuffer::from_pixel(canvas_w, canvas_h, bg_pixel);

    for (idx, thumb) in thumbnails.iter().enumerate() {
        let col = (idx as u32) % cols;
        let row = (idx as u32) / cols;

        // Top-left of the border box for this cell.
        let bx = PADDING + col * (2 * BORDER + tw + GAP);
        let by = PADDING + row * (2 * BORDER + th + GAP);

        // Draw the 1 px border rectangle (top, bottom, left, right edges).
        for x in bx..bx + 2 * BORDER + tw {
            canvas.put_pixel(x, by, border_pixel);
            canvas.put_pixel(x, by + 2 * BORDER + th - 1, border_pixel);
        }
        for y in by..by + 2 * BORDER + th {
            canvas.put_pixel(bx, y, border_pixel);
            canvas.put_pixel(bx + 2 * BORDER + tw - 1, y, border_pixel);
        }

        // Copy thumbnail pixels into the canvas (inside the border).
        let inner_x = bx + BORDER;
        let inner_y = by + BORDER;
        canvas
            .copy_from(thumb, inner_x, inner_y)
            .unwrap_or_else(|e| panic!("showcase: copy_from cell {idx}: {e}"));
    }

    // ── Write PNG ─────────────────────────────────────────────────────────────

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let out_dir = Path::new(manifest_dir).join("../../../docs/assets");

    std::fs::create_dir_all(&out_dir)
        .unwrap_or_else(|e| panic!("showcase: create_dir_all docs/assets: {e}"));

    let out_path = out_dir.join("templates-showcase.png");

    let mut png_buf: Vec<u8> = Vec::new();
    DynamicImage::ImageRgba8(canvas.clone())
        .write_to(&mut Cursor::new(&mut png_buf), ImageFormat::Png)
        .unwrap_or_else(|e| panic!("showcase: PNG encode failed: {e}"));

    std::fs::write(&out_path, &png_buf)
        .unwrap_or_else(|e| panic!("showcase: write to {}: {e}", out_path.display()));

    // ── Verify: decode back and check dimensions ──────────────────────────────

    let verified = image::open(&out_path)
        .unwrap_or_else(|e| panic!("showcase: re-open PNG for verification failed: {e}"));

    assert_eq!(
        verified.width(),
        canvas_w,
        "showcase PNG width mismatch after write+re-open"
    );
    assert_eq!(
        verified.height(),
        canvas_h,
        "showcase PNG height mismatch after write+re-open"
    );

    let file_size = png_buf.len();
    assert!(
        file_size >= 80_000,
        "showcase PNG suspiciously small ({file_size} bytes); expected ≥80 KB"
    );
    assert!(
        file_size <= 4_000_000,
        "showcase PNG suspiciously large ({file_size} bytes); expected ≤4 MB"
    );

    eprintln!(
        "templates-showcase.png written: {}×{} px, {} bytes ({} KB)",
        canvas_w,
        canvas_h,
        file_size,
        file_size / 1024,
    );
    eprintln!("  path: {}", out_path.display());

    // ── Verify: all ten per-template previews exist and are non-trivial ───────

    for (_, label, slug) in templates {
        let p = preview_dir.join(format!("{slug}.svg"));
        let meta = std::fs::metadata(&p)
            .unwrap_or_else(|e| panic!("showcase: preview {slug}.svg missing ({label}): {e}"));
        assert!(
            meta.len() >= 1_000,
            "showcase: preview {slug}.svg suspiciously small ({} bytes)",
            meta.len()
        );
    }
    eprintln!(
        "template previews written: {} SVG → {}",
        templates.len(),
        preview_dir.display()
    );
}
