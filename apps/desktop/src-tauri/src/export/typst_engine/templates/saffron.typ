// Saffron — warm two-column premium template with a tinted LEFT sidebar.
//
// Design contract:
//   Two-column layout: a warm-peach tinted sidebar (34 % width) on the LEFT
//   carries a circular RINGED photo, contact details, and the placement-sidebar
//   sections (Skills / Education / Languages).  The main column (right) carries
//   the name/title header then Summary, Experience, Projects, Certifications, and
//   any remaining non-sidebar sections.  Certifications read in the main column
//   (per theme::placement_for's Saffron override) — the split is entirely data-
//   driven via each section's `placement` field.
//
//   The sidebar band is drawn via page(background: ...) — same technique as
//   Portrait/Atelier — so it repeats on every page without clipping.  Sidebar
//   content is placed in the background too (page 1 only), with dy = margin_v so
//   it aligns with the main-column top margin.
//
//   Photo zone (top of sidebar background):
//     - has_photo == true : circular clip of /photo.png with a 1.5pt accent ring
//       (the differentiator vs Portrait's ringless circle).
//     - has_photo == false: a warm accent monogram circle (white initials).
//
//   ATS mode (data.opts.ats == true):
//     - No sidebar background, no photo. All sections linear in document order.
//
// Design: original.  Accent: terracotta (#A85A3E).  Headings: Source Serif 4
// small-caps; body: Inter.
// ORIGINALITY: designed from generic layout conventions only.
//
// Data contract (mirrors portrait.typ):
//   data.style.c_name / c_section / c_accent / c_body / c_date / c_rule
//   data.style.font_name / font_heading / font_body
//   data.style.section_all_caps / section_small_caps / job_title_italic — bool
//   data.style.name_pt / section_pt / body_pt
//   data.opts.page_width_mm / page_height_mm / accent / ats / has_photo / lang
//   data.header.name / title / contact[]
//   data.sections[].heading / blocks[] / placement / kind
//
// Guard: every optional dict key is checked before access.
// Spacing scale constants come from _scale.typ (prepended by engine.rs).

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#3A2E28" })
#let c-section = rgb(if "c_section" in st { st.c_section } else { "#A85A3E" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#302A26" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#8A7A6E" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#E2C9B4" })

#let c-accent = {
  if "accent" in data.opts and data.opts.accent != "" {
    rgb(data.opts.accent)
  } else if "c_accent" in st {
    rgb(st.c_accent)
  } else {
    rgb("#A85A3E")
  }
}

// Sidebar background — warm peach tint.
#let c-sidebar-bg = rgb("#F5E7DA")

#let font-name    = if "font_name"    in st { st.font_name    } else { "Source Serif 4" }
#let font-heading = if "font_heading" in st { st.font_heading } else { "Source Serif 4" }
#let font-body    = if "font_body"    in st { st.font_body    } else { "Inter" }

#let all-caps     = if "section_all_caps"   in st { st.section_all_caps   } else { false }
#let small-caps   = if "section_small_caps" in st { st.section_small_caps } else { true }
#let title-italic = if "job_title_italic"   in st { st.job_title_italic   } else { true }

#let name-pt    = if "name_pt"    in st { st.name_pt    * 1pt } else { 24pt }
#let section-pt = if "section_pt" in st { st.section_pt * 1pt } else { 11pt }
#let body-pt    = if "body_pt"    in st { st.body_pt    * 1pt } else { 10.5pt }

#let emphasize-edu = if "emphasize_education" in st { st.emphasize_education } else { false }

// ── Layout constants ──────────────────────────────────────────────────────────

#let page-w = data.opts.page_width_mm  * 1mm
#let page-h = data.opts.page_height_mm * 1mm

#let sidebar-frac = 0.34
#let sidebar-w    = sidebar-frac * page-w
#let gutter       = 9mm            // gap between sidebar band edge and main text
#let margin-v     = 14mm           // top/bottom margin
#let margin-r     = 14mm           // right margin

// Main content left margin = sidebar band width + gutter.
#let main-left    = sidebar-w + gutter

// ATS-mode left margin — comfortable single-column.
#let ats-left     = 20mm

// Photo circle diameter: fills ~72 % of the sidebar width.
#let photo-d = 46mm
// Ring stroke thickness (the Saffron differentiator vs Portrait's ringless circle).
#let ring-w  = 1.5pt

// Sidebar internal padding.
#let sb-pad-l = 8pt
#let sb-pad-r = 6pt

// ── Flags ─────────────────────────────────────────────────────────────────────

#let is-ats    = if "ats"       in data.opts { data.opts.ats       } else { false }
#let has-photo = if "has_photo" in data.opts { data.opts.has_photo } else { false }

// ── Text / paragraph defaults ─────────────────────────────────────────────────

#set text(
  font: (font-body, "Inter", "Carlito", "Noto Sans"),
  size: body-pt,
  fill: c-body,
  lang: data.opts.lang,
)

#set par(leading: lead, spacing: sp-para)

// ── Heading display (small-caps / all-caps aware) ──────────────────────────────

#let heading-display(h) = {
  let base = if all-caps { upper(h) } else { h }
  if small-caps { smallcaps(base) } else { base }
}

// ── Rich-text helpers ─────────────────────────────────────────────────────────

#let render-runs(runs) = {
  for r in runs {
    let t = if r.bold and r.italic {
      text(weight: "bold", style: "italic", r.text)
    } else if r.bold {
      text(weight: "bold", r.text)
    } else if r.italic {
      text(style: "italic", r.text)
    } else {
      r.text
    }
    if "link" in r and r.link != none {
      link(r.link, text(fill: c-accent, t))
    } else {
      t
    }
  }
}

// ── Entry renderer (main column) ───────────────────────────────────────────────

#let render-entry(blk, bold-title) = {
  let title-content = if "title" in blk { render-runs(blk.title) } else { "" }
  let date-str = if "date" in blk and blk.date != none { blk.date } else { "" }
  let title-weight = if bold-title { "bold" } else { "regular" }

  block(breakable: false, width: 100%, {
    grid(
      columns: (1fr, auto),
      gutter: 4pt,
      text(weight: title-weight, fill: c-body, title-content),
      text(weight: title-weight, fill: c-date, size: body-pt - 1pt, date-str),
    )

    if "subtitle" in blk and blk.subtitle != none and blk.subtitle.len() > 0 {
      block(above: sp-subtitle-gap, below: sp-subtitle-below,
        text(style: if title-italic { "italic" } else { "normal" }, fill: c-body, render-runs(blk.subtitle))
      )
    }

    if "bullets" in blk and blk.bullets.len() > 0 {
      block(above: sp-bullet-above, below: 0pt, {
        set list(spacing: sp-bullet-gap)
        for bullet in blk.bullets {
          list.item(render-runs(bullet))
        }
      })
    }
  })
}

#let entry-bold-for-section(section) = {
  let kind = if "kind" in section { section.kind } else { "" }
  if kind == "education" { emphasize-edu } else { true }
}

#let render-block(b, bold-title) = {
  if b.kind == "paragraph" {
    if "runs" in b { block(below: 4pt, render-runs(b.runs)) }
  } else if b.kind == "bullet" {
    if "runs" in b { list.item(render-runs(b.runs)) }
  } else if b.kind == "entry" {
    block(below: sp-entry, render-entry(b, bold-title))
  }
}

// ── Sidebar block renderer (compact) ───────────────────────────────────────────

#let render-block-sb(b, bold-title) = {
  if b.kind == "paragraph" {
    if "runs" in b {
      block(below: sp-sb-item, text(size: body-pt - 0.5pt, render-runs(b.runs)))
    }
  } else if b.kind == "bullet" {
    if "runs" in b {
      list.item(text(size: body-pt - 0.5pt, render-runs(b.runs)))
    }
  } else if b.kind == "entry" {
    let blk = b
    let title-content = if "title" in blk { render-runs(blk.title) } else { "" }
    let date-str = if "date" in blk and blk.date != none { blk.date } else { "" }
    let bold-t = if bold-title { "bold" } else { "regular" }
    block(below: sp-sb-item, {
      text(weight: bold-t, size: body-pt - 0.5pt, title-content)
      if date-str != "" {
        text(size: body-pt - 1.5pt, fill: c-date, " · " + date-str)
      }
      if "subtitle" in blk and blk.subtitle != none and blk.subtitle.len() > 0 {
        block(above: 1pt,
          text(style: "italic", size: body-pt - 1pt, render-runs(blk.subtitle))
        )
      }
    })
  }
}

// ── Section renderers ──────────────────────────────────────────────────────────

#let render-section-main(section) = {
  let bold-title = entry-bold-for-section(section)

  block(above: sp-section-above, below: sp-rule-below, {
    text(
      size: section-pt,
      weight: "bold",
      fill: c-section,
      font: (font-heading, "Source Serif 4", "Carlito"),
      heading-display(section.heading),
    )
  })
  line(length: 100%, stroke: 0.5pt + c-rule)
  block(above: sp-after-rule, {
    for b in section.blocks { render-block(b, bold-title) }
  })
}

#let render-section-sb(section) = {
  let bold-title = entry-bold-for-section(section)

  block(above: sp-sb-section-above, below: sp-sb-rule-below, {
    text(
      size: section-pt - 0.5pt,
      weight: "bold",
      fill: c-accent,
      font: (font-heading, "Source Serif 4", "Carlito"),
      heading-display(section.heading),
    )
  })
  line(length: 100%, stroke: 0.5pt + c-rule)
  block(above: sp-sb-after-rule, {
    for b in section.blocks { render-block-sb(b, bold-title) }
  })
}

// ── Section partitioning (data-driven) ─────────────────────────────────────────

#let section-placement(s) = {
  if "placement" in s { s.placement } else { "main" }
}

#let sidebar-sections = data.sections.filter(s => section-placement(s) == "sidebar")
#let main-sections    = data.sections.filter(s => section-placement(s) == "main")

// ── Candidate initials (no-photo fallback) ─────────────────────────────────────

// `.slice(0, 1)` is a BYTE offset and panics whenever the first character is
// multi-byte in UTF-8 (Ü, É, Ł, …) — plausible in DACH/EU candidate names. Use
// `.clusters()` (grapheme-cluster split) and take the first cluster instead, so
// a name like "Über Ödegaard" renders safely instead of aborting the export.
#let first-cluster(s) = {
  let cl = s.clusters()
  if cl.len() > 0 { cl.first() } else { "" }
}

#let name-str = if "name" in data.header { data.header.name } else { "" }
#let initials = {
  let parts = name-str.split(" ").filter(p => p.len() > 0)
  if parts.len() >= 2 {
    upper(first-cluster(parts.at(0))) + upper(first-cluster(parts.at(1)))
  } else if parts.len() == 1 {
    let c = first-cluster(parts.at(0))
    if c != "" { upper(c) } else { "?" }
  } else {
    "?"
  }
}

// ── Photo zone (circular, ringed) ──────────────────────────────────────────────
//
// The 1.5pt accent ring is drawn as its OWN stroked circle (fill: none),
// stacked via `place` over the clipped photo — NOT as the `stroke` of the
// clipped photo box itself. A clipped box's own stroke can be half-clipped at
// the content edge (Typst clips at the box's inner bound before the stroke is
// fully painted), which would render the ring at roughly half its 1.5pt weight.
// Stacking a separate, unclipped circle guarantees the full ring is visible —
// the Saffron differentiator vs Portrait's ringless circle.
//
// Geometry: the ring circle has radius `photo-d/2 - ring-w/2` so its stroke's
// outer edge lands exactly at `photo-d/2` (flush with the outer box, no
// overflow); the inner photo/monogram circle is sized to `inner-d` so its edge
// meets the ring's inner edge with no gap and no overlap.

#let inner-d = photo-d - 2 * ring-w

#let photo-zone = box(width: photo-d, height: photo-d, {
  place(center + horizon,
    circle(radius: photo-d / 2 - ring-w / 2, stroke: ring-w + c-accent, fill: none)
  )
  place(center + horizon,
    if has-photo {
      box(
        width: inner-d,
        height: inner-d,
        clip: true,
        radius: inner-d / 2,
        image("photo.png", width: inner-d, height: inner-d, fit: "cover"),
      )
    } else {
      circle(
        radius: inner-d / 2,
        fill: c-accent,
        stroke: none,
        text(
          size: name-pt * 0.75,
          weight: "bold",
          fill: white,
          font: (font-name, "Source Serif 4", "Carlito"),
          initials,
        ),
      )
    }
  )
})

// ── Sidebar inner content (rendered once, placed in page background) ───────────

#let sidebar-inner = box(
  width: sidebar-w - sb-pad-l - sb-pad-r,
  {
    // Photo zone.
    block(below: 12pt, width: 100%, align(center, photo-zone))

    // Contact line.
    if "contact" in data.header and data.header.contact.len() > 0 {
      block(below: 10pt, width: 100%, {
        text(
          size: body-pt - 1pt,
          fill: c-body,
          font: (font-body, "Inter", "Carlito"),
          render-runs(data.header.contact),
        )
      })
    }

    // Sidebar sections.
    for s in sidebar-sections {
      render-section-sb(s)
    }
  }
)

// ── Main column header (page 1 only) ───────────────────────────────────────────

#let main-header-content = {
  block(below: sp-name-below, {
    text(
      size: name-pt,
      weight: "bold",
      fill: c-name,
      font: (font-name, "Source Serif 4", "Carlito"),
      name-str,
    )
  })

  if "title" in data.header and data.header.title != none {
    block(below: sp-header-title-below,
      text(
        size: section-pt,
        style: if title-italic { "italic" } else { "normal" },
        fill: c-body,
        data.header.title,
      )
    )
  }

  // Thin accent keyline.
  block(above: 6pt, below: 10pt,
    line(length: 100%, stroke: 1.5pt + c-accent)
  )
}

// ── Render ──────────────────────────────────────────────────────────────────────

#if is-ats {
  // ATS: single-column, no sidebar, no photo.
  set page(
    width:  page-w,
    height: page-h,
    margin: (top: margin-v, bottom: margin-v, left: ats-left, right: margin-r),
  )

  block(below: sp-name-below, {
    text(
      size: name-pt,
      weight: "bold",
      fill: c-name,
      font: (font-name, "Source Serif 4", "Carlito"),
      name-str,
    )
  })

  if "title" in data.header and data.header.title != none {
    block(below: sp-header-title-below,
      text(
        size: section-pt,
        style: if title-italic { "italic" } else { "normal" },
        fill: c-body,
        data.header.title,
      )
    )
  }

  if "contact" in data.header {
    block(below: sp-header-contact,
      text(size: body-pt, fill: c-body, render-runs(data.header.contact))
    )
  }

  // All sections in document order (main + sidebar interleaved).
  for section in data.sections {
    render-section-main(section)
  }
} else {
  // Two-column layout using page(background: ...) to carry the LEFT sidebar band
  // + sidebar content — same technique as Portrait.
  set page(
    width:  page-w,
    height: page-h,
    margin: (top: margin-v, bottom: margin-v, left: 0pt, right: margin-r),
    background: {
      // Full-height tinted sidebar band — drawn on EVERY page for continuity.
      place(left + top,
        rect(width: sidebar-w, height: 100%, fill: c-sidebar-bg)
      )
      // Sidebar content — rendered ONCE, on page 1 only.
      context {
        if counter(page).get().first() == 1 {
          place(left + top, dx: sb-pad-l, dy: margin-v, sidebar-inner)
        }
      }
    },
  )

  // Main column header (inset by main-left so it clears the sidebar band).
  pad(left: main-left, main-header-content)

  // Main column sections.
  pad(left: main-left, {
    for section in main-sections {
      render-section-main(section)
    }
  })
}
