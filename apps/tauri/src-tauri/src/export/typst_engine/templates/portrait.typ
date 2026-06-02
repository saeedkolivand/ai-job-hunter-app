// Portrait — circular photo header, two-column premium template.
//
// Design contract:
//   Two-column layout: a sidebar (30 % width) on the left carries the photo,
//   contact details, skills, education, languages, and certifications.  The
//   main column (right 70 %) carries name/title header then summary, experience,
//   projects, and any remaining non-sidebar sections.
//
//   The sidebar band is drawn via page(background: ...) — same technique as
//   Atelier — so it repeats on every page without clipping or layout gaps.
//   Sidebar content is placed in the background too, with dy = margin_v so it
//   aligns with the main-column top margin.
//
//   Photo zone (top of sidebar background):
//     - When data.opts.has_photo == true: circular clip of /photo.png, centered
//       horizontally in the sidebar.
//     - When data.opts.has_photo == false: a monogram circle (accent fill, white
//       initials) so the sidebar header never looks broken.
//
//   ATS mode (data.opts.ats == true):
//     - No sidebar background, no photo.
//     - All sections rendered linearly in document order.
//
// Design: original.  Accent: deep slate-teal (#2A6478).
// ORIGINALITY: designed from generic layout conventions only.
//
// Data contract:
//   data.style.c_name / c_section / c_accent / c_body / c_date / c_rule
//   data.style.font_name / font_heading / font_body
//   data.style.section_all_caps — bool
//   data.style.job_title_italic — bool
//   data.style.name_pt / section_pt / body_pt
//   data.opts.page_width_mm / page_height_mm
//   data.opts.accent — optional override (#RRGGBB or "")
//   data.opts.ats — bool
//   data.opts.has_photo — bool (true → /photo.png is served in the World)
//   data.opts.lang
//   data.header.name / title / contact[]
//   data.sections[].heading / blocks[] / placement / kind
//
// Guard: every optional dict key is checked before access.
// Spacing scale constants come from _scale.typ (prepended by engine.rs).

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#122832" })
#let c-section = rgb(if "c_section" in st { st.c_section } else { "#2A6478" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#1C1E20" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#5A6E78" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#A0C3D2" })

#let c-accent = {
  if "accent" in data.opts and data.opts.accent != "" {
    rgb(data.opts.accent)
  } else if "c_accent" in st {
    rgb(st.c_accent)
  } else {
    rgb("#2A6478")
  }
}

// Sidebar background — very light teal tint.
#let c-sidebar-bg = rgb("#EBF4F8")

#let font-name    = if "font_name"    in st { st.font_name    } else { "Inter" }
#let font-heading = if "font_heading" in st { st.font_heading } else { "Inter" }
#let font-body    = if "font_body"    in st { st.font_body    } else { "Inter" }

#let all-caps     = if "section_all_caps"  in st { st.section_all_caps  } else { true }
#let title-italic = if "job_title_italic"  in st { st.job_title_italic  } else { true }

#let name-pt    = if "name_pt"    in st { st.name_pt    * 1pt } else { 22pt }
#let section-pt = if "section_pt" in st { st.section_pt * 1pt } else { 10.5pt }
#let body-pt    = if "body_pt"    in st { st.body_pt    * 1pt } else { 10.5pt }

#let emphasize-edu = if "emphasize_education" in st { st.emphasize_education } else { false }

// ── Layout constants ──────────────────────────────────────────────────────────

#let page-w = data.opts.page_width_mm  * 1mm
#let page-h = data.opts.page_height_mm * 1mm

#let sidebar-frac = 0.30
#let sidebar-w    = sidebar-frac * page-w
#let gutter       = 9mm            // gap between sidebar band edge and main text
#let margin-v     = 14mm           // top/bottom margin
#let margin-r     = 14mm           // right margin

// Main content left margin = sidebar band width + gutter.
#let main-left    = sidebar-w + gutter

// ATS-mode left margin — comfortable single-column.
#let ats-left     = 20mm

// Photo circle diameter: fills ~72 % of the sidebar width.
// Using a fixed 48mm so it looks intentional and scales well to portrait format.
#let photo-d = 48mm

// Sidebar internal padding (left/right of sidebar content).
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

// ── Entry renderer ────────────────────────────────────────────────────────────

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
        text(style: "italic", fill: c-body, render-runs(blk.subtitle))
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

// ── Block renderer (main column) ──────────────────────────────────────────────

#let render-block(b, bold-title) = {
  if b.kind == "paragraph" {
    if "runs" in b { block(below: 4pt, render-runs(b.runs)) }
  } else if b.kind == "bullet" {
    if "runs" in b { list.item(render-runs(b.runs)) }
  } else if b.kind == "entry" {
    block(below: sp-entry, render-entry(b, bold-title))
  }
}

// ── Sidebar block renderer (compact) ─────────────────────────────────────────

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

// ── Section renderers ─────────────────────────────────────────────────────────

#let render-section-main(section) = {
  let heading-text = if all-caps { upper(section.heading) } else { section.heading }
  let bold-title = entry-bold-for-section(section)

  block(above: sp-section-above, below: sp-rule-below, {
    text(
      size: section-pt,
      weight: "bold",
      fill: c-section,
      font: (font-heading, "Inter", "Carlito"),
      heading-text,
    )
  })
  line(length: 100%, stroke: 0.5pt + c-rule)
  block(above: sp-after-rule, {
    for b in section.blocks { render-block(b, bold-title) }
  })
}

#let render-section-sb(section) = {
  let heading-text = if all-caps { upper(section.heading) } else { section.heading }
  let bold-title = entry-bold-for-section(section)

  block(above: sp-sb-section-above, below: sp-sb-rule-below, {
    text(
      size: section-pt - 0.5pt,
      weight: "bold",
      fill: c-accent,
      font: (font-heading, "Inter", "Carlito"),
      heading-text,
    )
  })
  line(length: 100%, stroke: 0.5pt + c-rule)
  block(above: sp-sb-after-rule, {
    for b in section.blocks { render-block-sb(b, bold-title) }
  })
}

// ── Section partitioning ──────────────────────────────────────────────────────

#let section-placement(s) = {
  if "placement" in s { s.placement } else { "main" }
}

#let sidebar-sections = data.sections.filter(s => section-placement(s) == "sidebar")
#let main-sections    = data.sections.filter(s => section-placement(s) == "main")

// ── Candidate initials (no-photo fallback) ────────────────────────────────────

#let name-str = if "name" in data.header { data.header.name } else { "" }
#let initials = {
  let parts = name-str.split(" ").filter(p => p.len() > 0)
  if parts.len() >= 2 {
    upper(parts.at(0).slice(0, 1)) + upper(parts.at(1).slice(0, 1))
  } else if parts.len() == 1 {
    upper(parts.at(0).slice(0, 1))
  } else {
    "?"
  }
}

// ── Sidebar inner content (rendered once, placed in page background) ───────────
// Built as a fixed-width box so it repeats cleanly on every page AND so that
// `line(length: 100%)` resolves to the sidebar box width rather than the full
// page width.  This is the same technique used in the Atelier template.

#let sidebar-inner = box(
  width: sidebar-w - sb-pad-l - sb-pad-r,
  {
    // Photo zone.
    block(below: 10pt, width: 100%, {
      align(center, {
        if has-photo {
          box(
            width: photo-d,
            height: photo-d,
            clip: true,
            radius: photo-d / 2,
            image("photo.png", width: photo-d, height: photo-d, fit: "cover"),
          )
        } else {
          circle(
            radius: photo-d / 2,
            fill: c-accent,
            stroke: none,
            text(
              size: name-pt * 0.8,
              weight: "bold",
              fill: white,
              font: (font-name, "Inter", "Carlito"),
              initials,
            ),
          )
        }
      })
    })

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

// ── Main column header (rendered in normal content flow, page 1 only) ─────────

#let main-header-content = {
  block(below: sp-name-below, {
    text(
      size: name-pt,
      weight: "bold",
      fill: c-name,
      font: (font-name, "Inter", "Carlito"),
      name-str,
    )
  })

  if "title" in data.header and data.header.title != none {
    block(below: 4pt,
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
    line(length: 100%, stroke: 2pt + c-accent)
  )
}

// ── Render ────────────────────────────────────────────────────────────────────

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
      font: (font-name, "Inter", "Carlito"),
      name-str,
    )
  })

  if "title" in data.header and data.header.title != none {
    block(below: 4pt,
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
  // Two-column layout using page(background: ...) to carry the sidebar band
  // + sidebar content on every page — same technique as the Atelier template.
  set page(
    width:  page-w,
    height: page-h,
    margin: (top: margin-v, bottom: margin-v, left: 0pt, right: margin-r),
    background: {
      // Full-height tinted sidebar band.
      place(left + top,
        rect(width: sidebar-w, height: 100%, fill: c-sidebar-bg)
      )
      // Sidebar content placed over the band, starting at the top margin.
      place(left + top, dx: sb-pad-l, dy: margin-v, sidebar-inner)
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
