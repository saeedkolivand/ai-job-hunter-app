// Aria — minimalist two-column premium template with an untinted RIGHT sidebar.
//
// Design contract:
//   Two-column layout: an UNTINTED sidebar (32 % width) on the RIGHT carries a
//   rectangular top-right photo, contact details, and the placement-sidebar
//   sections (Skills / Languages / Certifications — NOT Education, which reads in
//   the main column per theme::placement_for's Aria override).  The main column
//   (left) opens with a large 30pt name then Summary, Experience, Projects,
//   Education, and any remaining non-sidebar sections.  The split is entirely
//   data-driven via each section's `placement` field.
//
//   No sidebar tint: the sidebar is separated by whitespace only (the registry
//   ships a white sidebar_bg, so no band rect is drawn).  Sidebar content is
//   placed via page(background: ...) so it can align with the main-column top
//   margin and repeat cleanly; it is rendered ONCE (page 1 only).  The main
//   column reserves the right margin (main-right) so its text clears the sidebar.
//
//   Photo zone (top of the right sidebar):
//     - has_photo == true : a rectangular /photo.png with subtle rounding.
//     - has_photo == false: NOTHING (minimalist name-only fallback — no monogram).
//       The candidate name always lives in the main-column header regardless.
//
//   ATS mode (data.opts.ats == true):
//     - No sidebar, no photo. All sections linear in document order — mirrors
//       portrait.typ's is-ats branch.
//
// Design: original.  Accent: slate (#46505C).  Name/headings: Manrope; body: Inter.
// Headings are letter-spaced (heading_tracking) all-caps with hairline rules.
// ORIGINALITY: designed from generic layout conventions only.
//
// Data contract (mirrors portrait.typ, plus data.style.heading_tracking):
//   data.style.c_name / c_section / c_accent / c_body / c_date / c_rule
//   data.style.font_name / font_heading / font_body
//   data.style.section_all_caps / job_title_italic — bool
//   data.style.heading_tracking — number: heading letter-spacing in em (0 = none)
//   data.style.name_pt / section_pt / body_pt
//   data.opts.page_width_mm / page_height_mm / accent / ats / has_photo / lang
//   data.header.name / title / contact[]
//   data.sections[].heading / blocks[] / placement / kind
//
// Guard: every optional dict key is checked before access.
// Spacing scale constants come from _scale.typ (prepended by engine.rs).

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#111111" })
#let c-section = rgb(if "c_section" in st { st.c_section } else { "#1A1A1A" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#2A2A2A" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#7A7A7A" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#D6D9DD" })

#let c-accent = {
  if "accent" in data.opts and data.opts.accent != "" {
    rgb(data.opts.accent)
  } else if "c_accent" in st {
    rgb(st.c_accent)
  } else {
    rgb("#46505C")
  }
}

#let font-name    = if "font_name"    in st { st.font_name    } else { "Manrope" }
#let font-heading = if "font_heading" in st { st.font_heading } else { "Manrope" }
#let font-body    = if "font_body"    in st { st.font_body    } else { "Inter" }

#let all-caps     = if "section_all_caps"  in st { st.section_all_caps  } else { true }
#let title-italic = if "job_title_italic"  in st { st.job_title_italic  } else { false }

// Heading letter-spacing (tracking) in em. 0 → no tracking argument emitted.
#let heading-tracking = if "heading_tracking" in st { st.heading_tracking } else { 0.0 }

#let name-pt    = if "name_pt"    in st { st.name_pt    * 1pt } else { 30pt }
#let section-pt = if "section_pt" in st { st.section_pt * 1pt } else { 10.5pt }
#let body-pt    = if "body_pt"    in st { st.body_pt    * 1pt } else { 10pt }

#let emphasize-edu = if "emphasize_education" in st { st.emphasize_education } else { false }

// ── Layout constants ──────────────────────────────────────────────────────────

#let page-w = data.opts.page_width_mm  * 1mm
#let page-h = data.opts.page_height_mm * 1mm

#let sidebar-frac = 0.32
#let sidebar-w    = sidebar-frac * page-w
#let gutter       = 10mm           // gap between main text and the sidebar
#let margin-v     = 16mm           // generous top/bottom margin
#let margin-l     = 16mm           // left margin (main column)

// Main content reserves this much on the RIGHT so it clears the sidebar zone.
#let main-right   = sidebar-w + gutter

// ATS-mode side margins — comfortable single-column.
#let ats-x        = 20mm

// Sidebar internal padding.
#let sb-pad-l = 6pt
#let sb-pad-r = 8pt

// Rectangular photo: fills the sidebar content width; portrait-ish aspect.
#let photo-w = sidebar-w - sb-pad-l - sb-pad-r
#let photo-h = photo-w * 1.15

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

// ── Heading helpers (all-caps + optional tracking) ─────────────────────────────

#let heading-display(h) = if all-caps { upper(h) } else { h }

#let heading-run(content, size, fill) = {
  if heading-tracking != 0.0 {
    text(
      size: size,
      weight: "bold",
      fill: fill,
      font: (font-heading, "Manrope", "Inter", "Carlito"),
      tracking: heading-tracking * 1em,
      content,
    )
  } else {
    text(
      size: size,
      weight: "bold",
      fill: fill,
      font: (font-heading, "Manrope", "Inter", "Carlito"),
      content,
    )
  }
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
    heading-run(heading-display(section.heading), section-pt, c-section)
  })
  line(length: 100%, stroke: 0.5pt + c-rule)
  block(above: sp-after-rule, {
    for b in section.blocks { render-block(b, bold-title) }
  })
}

#let render-section-sb(section) = {
  let bold-title = entry-bold-for-section(section)

  block(above: sp-sb-section-above, below: sp-sb-rule-below, {
    heading-run(heading-display(section.heading), section-pt - 0.5pt, c-accent)
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

// ── Photo zone (rectangular, subtle rounding) — omitted entirely when no photo ─

#let name-str = if "name" in data.header { data.header.name } else { "" }

// ── Sidebar inner content (rendered once, placed in page background) ───────────

#let sidebar-inner = box(
  width: sidebar-w - sb-pad-l - sb-pad-r,
  {
    // Photo zone — rectangular with subtle rounding. No monogram fallback:
    // when there is no photo the sidebar simply opens with the contact block.
    if has-photo {
      block(below: 12pt, width: 100%, {
        box(
          width: photo-w,
          height: photo-h,
          clip: true,
          radius: 3pt,
          image("photo.png", width: photo-w, height: photo-h, fit: "cover"),
        )
      })
    }

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
      font: (font-name, "Manrope", "Inter", "Carlito"),
      name-str,
    )
  })

  if "title" in data.header and data.header.title != none {
    block(below: sp-header-title-below,
      text(
        size: section-pt,
        style: if title-italic { "italic" } else { "normal" },
        fill: c-accent,
        data.header.title,
      )
    )
  }

  // Thin accent keyline anchoring the header.
  block(above: 6pt, below: 12pt,
    line(length: 100%, stroke: 1pt + c-accent)
  )
}

// ── Render ──────────────────────────────────────────────────────────────────────

#if is-ats {
  // ATS: single-column, no sidebar, no photo (mirrors portrait.typ).
  set page(
    width:  page-w,
    height: page-h,
    margin: (top: margin-v, bottom: margin-v, left: ats-x, right: ats-x),
  )

  block(below: sp-name-below, {
    text(
      size: name-pt,
      weight: "bold",
      fill: c-name,
      font: (font-name, "Manrope", "Inter", "Carlito"),
      name-str,
    )
  })

  if "title" in data.header and data.header.title != none {
    block(below: sp-header-title-below,
      text(
        size: section-pt,
        style: if title-italic { "italic" } else { "normal" },
        fill: c-accent,
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
  // Two-column layout with the sidebar on the RIGHT. The band carries NO tint
  // (Aria is separated by whitespace only); sidebar content is placed on page 1.
  set page(
    width:  page-w,
    height: page-h,
    margin: (top: margin-v, bottom: margin-v, left: margin-l, right: 0pt),
    background: {
      // No background rect — the sidebar is untinted by design.
      // Sidebar content — rendered ONCE, on page 1 only, right-aligned.
      context {
        if counter(page).get().first() == 1 {
          place(right + top, dx: -sb-pad-r, dy: margin-v, sidebar-inner)
        }
      }
    },
  )

  // Main column header (reserve the right margin so it clears the sidebar).
  pad(right: main-right, main-header-content)

  // Main column sections.
  pad(right: main-right, {
    for section in main-sections {
      render-section-main(section)
    }
  })
}
