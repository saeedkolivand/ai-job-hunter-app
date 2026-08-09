// Deedy — after the modern single-column Deedy revision: bold name block
// with an accent-colored surname, generous section spacing, subtle grey
// meta-line under entries.
//
// Design contract:
//   A large bold name block where the LAST word of the candidate's name (the
//   surname) renders in the accent color while the rest of the name stays in
//   the base ink — the reference's signature name treatment, reproduced here
//   as a runtime string split rather than a hardcoded field (the generic IR
//   carries `header.name` as one string). Section headings get extra
//   breathing room above them (`sp-section-above` plus a fixed original-design
//   supplement) versus the shared house rhythm. Entry subtitles render as a
//   subtle grey meta-line (not italic, unlike every other template here) —
//   the "meta" read the reference's sub-line conventionally carries.
//
//   ATS mode (data.opts.ats == true) — the toggle this design-tier template
//   surfaces: renders the whole name in one color (the accent split is purely
//   cosmetic; color never affects PDF text extraction, so this is an honesty
//   signal, not a parser-safety fix — the layout was already single-column).
//
// Design: independent design inspired by the generic "large name block +
// meta sub-line" layout convention common to the Deedy community family, not
// a copy of any one implementation.
// ORIGINALITY: independent design based on generic layout conventions only.
//
// Data contract (same as single_column.typ):
//   data.style.c_name / c_section / c_accent / c_body / c_date / c_rule
//   data.style.font_name / font_heading / font_body
//   data.style.section_all_caps — bool
//   data.style.job_title_italic — bool
//   data.style.name_pt / section_pt / body_pt
//   data.opts.page_width_mm / page_height_mm
//   data.opts.accent — optional override (#RRGGBB or "")
//   data.opts.ats — bool
//   data.opts.lang
//   data.header.name / title / contact[]
//   data.sections[].heading / blocks[] / kind
//
// Guard: every optional dict key is checked before access.
// Spacing scale constants come from _scale.typ (prepended by engine.rs).

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#1A1A1A" })
#let c-section = rgb(if "c_section" in st { st.c_section } else { "#1A1A1A" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#222222" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#787878" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#C8C8C8" })
// Subtle grey meta-line color — deliberately its own tone (lighter than
// c-date), independent so a template with a warm c-date doesn't drag the
// meta-line off "subtle grey".
#let c-meta    = rgb("#7A7A7A")

#let c-accent = {
  if "accent" in data.opts and data.opts.accent != "" {
    rgb(data.opts.accent)
  } else if "c_accent" in st {
    rgb(st.c_accent)
  } else {
    rgb("#1E4FB3")
  }
}

#let font-name    = if "font_name"    in st { st.font_name    } else { "Manrope" }
#let font-heading = if "font_heading" in st { st.font_heading } else { "Manrope" }
#let font-body    = if "font_body"    in st { st.font_body    } else { "Inter" }

#let all-caps     = if "section_all_caps" in st { st.section_all_caps } else { true }
#let title-italic = if "job_title_italic" in st { st.job_title_italic } else { false }

#let name-pt    = if "name_pt"    in st { st.name_pt    * 1pt } else { 27pt }
#let section-pt = if "section_pt" in st { st.section_pt * 1pt } else { 11.5pt }
#let body-pt    = if "body_pt"    in st { st.body_pt    * 1pt } else { 10.5pt }

#let emphasize-edu = if "emphasize_education" in st { st.emphasize_education } else { false }

#let is-ats = if "ats" in data.opts { data.opts.ats } else { false }

// Extra breathing room above every section heading, on top of the shared
// house rhythm — the reference's "generous section spacing" trait. A literal
// local supplement, not a new registry knob (`_scale.typ` stays the single
// locked source for the base rhythm every template shares).
#let sp-section-extra = 8pt

// ── Page setup ────────────────────────────────────────────────────────────────

#set page(
  width:  data.opts.page_width_mm  * 1mm,
  height: data.opts.page_height_mm * 1mm,
  margin: (x: 22mm, y: 20mm),
)

#set text(
  font: (font-body, "Inter", "Carlito", "Noto Sans"),
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

// ── Header: bold name block, surname in accent (dropped under ATS mode) ────────

#let name-str = if "name" in data.header { data.header.name } else { "" }
#let name-tokens = name-str.split(" ").filter((w) => w != "")

#let name-block() = {
  if is-ats or name-tokens.len() <= 1 {
    // ATS mode, or a single-token name with nothing to split: one plain run.
    text(size: name-pt, weight: "bold", fill: c-name,
      font: (font-name, "Manrope", "Carlito"), name-str)
  } else {
    let surname = name-tokens.last()
    let rest = name-tokens.slice(0, name-tokens.len() - 1).join(" ")
    text(size: name-pt, weight: "bold", fill: c-name,
      font: (font-name, "Manrope", "Carlito"), rest + " ")
    text(size: name-pt, weight: "bold", fill: c-accent,
      font: (font-name, "Manrope", "Carlito"), surname)
  }
}

#block(below: sp-name-below, name-block())

#if "title" in data.header and data.header.title != none and data.header.title != "" {
  block(below: sp-header-title-below,
    text(
      size: section-pt,
      style: if title-italic { "italic" } else { "normal" },
      fill: c-body,
      data.header.title,
    )
  )
}

#if "contact" in data.header and data.header.contact.len() > 0 {
  block(below: sp-header-contact,
    text(size: body-pt, fill: c-body, render-runs(data.header.contact))
  )
}

// ── Entry renderer (subtitle = subtle grey meta-line, not italic) ──────────────

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
        text(size: body-pt - 0.5pt, fill: c-meta, render-runs(blk.subtitle))
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

// ── Block renderer ────────────────────────────────────────────────────────────

#let render-block(b, bold-title) = {
  if b.kind == "paragraph" {
    if "runs" in b { block(below: 4pt, render-runs(b.runs)) }
  } else if b.kind == "bullet" {
    if "runs" in b { list.item(render-runs(b.runs)) }
  } else if b.kind == "entry" {
    block(below: sp-entry, render-entry(b, bold-title))
  }
}

// ── Section renderer (generous spacing above, ruled-bottom heading) ────────────

#let render-section(section) = {
  let heading-text = if all-caps { upper(section.heading) } else { section.heading }
  let bold-title = entry-bold-for-section(section)

  block(above: sp-section-above + sp-section-extra, below: sp-rule-below, {
    text(
      size: section-pt,
      weight: "bold",
      fill: c-section,
      font: (font-heading, "Manrope", "Carlito"),
      heading-text,
    )
  })
  line(length: 100%, stroke: 0.6pt + c-rule)
  block(above: sp-after-rule, {
    for b in section.blocks { render-block(b, bold-title) }
  })
}

// ── Body ──────────────────────────────────────────────────────────────────────

#for section in data.sections {
  render-section(section)
}
