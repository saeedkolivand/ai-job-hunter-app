// Lebenslauf — DACH DIN-style tabular CV.
//
// Design contract:
//   Formal A4 single-column layout following German Lebenslauf conventions
//   (generic DIN 5008 / DACH style — not a copy of any product).
//
//   Header zone:
//     - Photo: rectangular, top-right corner, when data.opts.has_photo is true.
//     - Candidate name (large, bold) and professional title left-aligned.
//     - Contact details stacked below name as short key: value lines.
//     - Thin accent rule beneath the header block.
//
//   Body:
//     - Section headings: normal-case, accent color, ruled-bottom divider.
//     - Entry blocks: left date-range column (fixed width) | right content column.
//       This tabular layout mimics the classic Lebenslauf left-label / right-value
//       row structure, providing a strong date-first scan path.
//     - Bullet points are indented within the right content column.
//     - Non-entry blocks (paragraphs, standalone bullets): rendered as normal text.
//
//   ATS mode (data.opts.ats == true):
//     - Photo omitted.
//     - Normal single-column linear output (same entry content, no date-column).
//
//   No-photo fallback:
//     - When data.opts.has_photo is false the header is text-only (name + title +
//       contact).  The header retains its proportions; no empty photo box appears.
//
// Design: original — independent of any product.
// Accent: warm slate (#3D4F6B).
// ORIGINALITY: designed from generic DACH CV conventions only.
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
//   data.opts.has_photo — bool
//   data.opts.lang
//   data.header.name / title / contact[]
//   data.sections[].heading / blocks[] / kind
//
// Guard: every optional dict key is checked before access.
// Spacing scale constants come from _scale.typ (prepended by engine.rs).

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#141923" })
#let c-section = rgb(if "c_section" in st { st.c_section } else { "#3D4F6B" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#1E1E23" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#64707D" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#B4BED2" })

#let c-accent = {
  if "accent" in data.opts and data.opts.accent != "" {
    rgb(data.opts.accent)
  } else if "c_accent" in st {
    rgb(st.c_accent)
  } else {
    rgb("#3D4F6B")
  }
}

#let font-name    = if "font_name"    in st { st.font_name    } else { "Carlito" }
#let font-heading = if "font_heading" in st { st.font_heading } else { "Carlito" }
#let font-body    = if "font_body"    in st { st.font_body    } else { "Carlito" }

// DIN style: normal-case section headings, no italic job titles.
#let all-caps     = if "section_all_caps" in st { st.section_all_caps } else { false }
#let title-italic = if "job_title_italic" in st { st.job_title_italic } else { false }

#let name-pt    = if "name_pt"    in st { st.name_pt    * 1pt } else { 20pt }
#let section-pt = if "section_pt" in st { st.section_pt * 1pt } else { 11pt }
#let body-pt    = if "body_pt"    in st { st.body_pt    * 1pt } else { 10.5pt }

// Education is de-emphasised by default (non-academic).
#let emphasize-edu = if "emphasize_education" in st { st.emphasize_education } else { false }

// ── Layout constants ──────────────────────────────────────────────────────────

#let page-w = data.opts.page_width_mm  * 1mm
#let page-h = data.opts.page_height_mm * 1mm

// Body margins (DIN 5008 approximate: top 27mm, left/right 25mm, bottom 20mm).
#let margin-top    = 20mm
#let margin-bottom = 20mm
#let margin-lr     = 22mm

// Photo: standard Lebenslauf photo width ~35 mm, height ~45 mm (passport ratio).
#let photo-w = 35mm
#let photo-h = 45mm

// Date column width in the entry table.
#let date-col-w = 32mm
// Gap between date column and content column.
#let date-gap   = 8pt

// ── ATS / photo flags ─────────────────────────────────────────────────────────

#let is-ats    = if "ats"       in data.opts { data.opts.ats       } else { false }
#let has-photo = if "has_photo" in data.opts { data.opts.has_photo } else { false }

// ── Page setup ────────────────────────────────────────────────────────────────

#set page(
  width:  page-w,
  height: page-h,
  margin: (top: margin-top, bottom: margin-bottom, left: margin-lr, right: margin-lr),
)

#set text(
  font: (font-body, "Carlito", "Inter", "Noto Sans"),
  size: body-pt,
  fill: c-body,
  lang: data.opts.lang,
)

#set par(leading: lead, spacing: sp-para)

// ── Rich-text helper ──────────────────────────────────────────────────────────

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

// ── Header ────────────────────────────────────────────────────────────────────
// When has-photo: name/title/contact on the left, photo on the right.
// When no photo: full-width name/title/contact.

#let name-str = if "name" in data.header { data.header.name } else { "" }

#let header-text-block() = {
  block(below: sp-name-below, {
    text(
      size: name-pt,
      weight: "bold",
      fill: c-name,
      font: (font-name, "Carlito", "Inter"),
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

  if "contact" in data.header and data.header.contact.len() > 0 {
    block(above: 4pt, below: 0pt,
      text(size: body-pt, fill: c-body, render-runs(data.header.contact))
    )
  }
}

// Render header zone.
#if not is-ats and has-photo {
  // Two-column header: name/title/contact left, photo right.
  block(below: 10pt, {
    grid(
      columns: (1fr, photo-w),
      gutter: 10pt,
      header-text-block(),
      // Photo: rectangular (standard Lebenslauf format), top-right.
      align(top + right,
        box(
          width: photo-w,
          height: photo-h,
          clip: true,
          image("photo.png", width: photo-w, height: photo-h, fit: "cover"),
        )
      ),
    )
  })
} else {
  // No-photo header: full-width text only.
  block(below: 10pt, header-text-block())
}

// Accent keyline beneath the header.
#line(length: 100%, stroke: 0.75pt + c-accent)
#block(above: 8pt, below: 0pt, [])

// ── Entry renderer (tabular date | content) ───────────────────────────────────
// Each entry is laid out as a two-column grid:
//   left  (date-col-w): date range, right-aligned, date color.
//   right (1fr):        title / subtitle / bullets.
// In ATS mode the date column is omitted for linear output.

#let render-entry-tabular(blk, bold-title) = {
  let title-content = if "title" in blk { render-runs(blk.title) } else { "" }
  let date-str = if "date" in blk and blk.date != none { blk.date } else { "" }
  let title-weight = if bold-title { "bold" } else { "regular" }

  if is-ats {
    // ATS: single-column, date inline.
    block(breakable: false, width: 100%, below: sp-entry, {
      text(weight: title-weight, fill: c-body, title-content)
      if date-str != "" {
        text(fill: c-date, size: body-pt - 1pt, "  " + date-str)
      }
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
  } else {
    // Tabular: date left, content right.
    block(breakable: false, width: 100%, below: sp-entry, {
      grid(
        columns: (date-col-w, date-gap, 1fr),
        rows: (auto,),
        gutter: 0pt,
        // Date cell — right-aligned.
        align(right + top,
          text(fill: c-date, size: body-pt - 1pt, date-str)
        ),
        // Gap.
        [],
        // Content cell.
        {
          text(weight: title-weight, fill: c-body, title-content)
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
        },
      )
    })
  }
}

#let entry-bold-for-section(section) = {
  let kind = if "kind" in section { section.kind } else { "" }
  if kind == "education" { emphasize-edu } else { true }
}

// ── Block renderer ────────────────────────────────────────────────────────────

#let render-block(b, bold-title) = {
  if b.kind == "paragraph" {
    if "runs" in b { block(below: 4pt, render-runs(b.runs)) }
  } else if b.kind == "bullet" {
    if "runs" in b { list.item(render-runs(b.runs)) }
  } else if b.kind == "entry" {
    render-entry-tabular(b, bold-title)
  }
}

// ── Section renderer ──────────────────────────────────────────────────────────

#let render-section(section) = {
  let heading-text = if all-caps { upper(section.heading) } else { section.heading }
  let bold-title = entry-bold-for-section(section)

  block(above: sp-section-above, below: sp-rule-below, {
    text(
      size: section-pt,
      weight: "bold",
      fill: c-accent,
      font: (font-heading, "Carlito", "Inter"),
      heading-text,
    )
  })
  line(length: 100%, stroke: 0.5pt + c-rule)
  block(above: sp-after-rule, {
    for b in section.blocks { render-block(b, bold-title) }
  })
}

// ── Body ──────────────────────────────────────────────────────────────────────

#for section in data.sections {
  render-section(section)
}
