// Awesome — after Awesome-CV: thin accent-tinted header band + accent-bar
// section-title markers, single column body.
//
// Design contract:
//   A thin, full-width accent-tinted header band holds the candidate name
//   (white, bold) and the contact line (white) below it. A short filled
//   accent bar precedes each section-heading label instead of a
//   ruled-bottom divider. Below the band: an airy single-column body using
//   the shared _scale.typ rhythm and the render-entry pattern (bold
//   job/project titles, education non-bold).
//
//   ATS mode (data.opts.ats == true) — the toggle this design-tier template
//   surfaces: drops the colored band for a plain black-on-white header, and
//   drops the accent bar for a plain bold heading with a thin rule. The
//   reading order was always single-column top-to-bottom either way; this
//   only removes decorative color so the export is unambiguously plain when
//   requested.
//
// Design: independent design inspired by the generic "accent header band +
// marker heading" layout convention common to the Awesome-CV community
// family, not a copy of any one implementation.
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

// c-name backs the ATS-mode plain header only; the band header always uses
// hardcoded white (c-band-text below) regardless of this value — see the
// registry's `Template::awesome` doc comment for why the two must differ.
#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#1A1A1A" })
#let c-section = rgb(if "c_section" in st { st.c_section } else { "#1A1A1A" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#222222" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#6E6E6E" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#C41E3A" })

#let c-accent = {
  if "accent" in data.opts and data.opts.accent != "" {
    rgb(data.opts.accent)
  } else if "c_accent" in st {
    rgb(st.c_accent)
  } else {
    rgb("#C41E3A")
  }
}

// Band text is always white — the band supplies its own contrast, independent
// of the registry's c_name (see the note above c-name).
#let c-band-text = rgb("#FFFFFF")

#let font-name    = if "font_name"    in st { st.font_name    } else { "Inter" }
#let font-heading = if "font_heading" in st { st.font_heading } else { "Inter" }
#let font-body    = if "font_body"    in st { st.font_body    } else { "Inter" }

#let all-caps     = if "section_all_caps" in st { st.section_all_caps } else { true }
#let title-italic = if "job_title_italic" in st { st.job_title_italic } else { true }

#let name-pt    = if "name_pt"    in st { st.name_pt    * 1pt } else { 24pt }
#let section-pt = if "section_pt" in st { st.section_pt * 1pt } else { 11pt }
#let body-pt    = if "body_pt"    in st { st.body_pt    * 1pt } else { 10.5pt }

// Education is de-emphasised by default (non-academic template).
#let emphasize-edu = if "emphasize_education" in st { st.emphasize_education } else { false }

#let is-ats = if "ats" in data.opts { data.opts.ats } else { false }

// ── Layout constants ──────────────────────────────────────────────────────────

#let page-w = data.opts.page_width_mm  * 1mm
#let page-h = data.opts.page_height_mm * 1mm

// A THIN band (vs. Meridian's 38mm) — name + one contact line only.
#let band-h        = 24mm
#let keyline-pt     = 1.5pt
#let body-margin-h  = 20mm
#let body-margin-top = 7mm

#let top-margin = if is-ats { 20mm } else { band-h + keyline-pt + body-margin-top }

// ── Rich-text helpers ──────────────────────────────────────────────────────────

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

#let render-runs-white(runs) = {
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
      link(r.link, text(fill: c-band-text, t))
    } else {
      t
    }
  }
}

// ── Page setup ────────────────────────────────────────────────────────────────

#set page(
  width:  page-w,
  height: page-h,
  margin: (
    top:    top-margin,
    bottom: 18mm,
    left:   body-margin-h,
    right:  body-margin-h,
  ),
  background: if is-ats { none } else {
    place(top + left, rect(width: 100%, height: band-h, fill: c-accent))
    place(top + left, dy: band-h, line(length: 100%, stroke: keyline-pt + c-accent))
    place(top + left, dx: body-margin-h, dy: 0pt, pad(top: 6mm, {
      text(
        size: name-pt,
        weight: "bold",
        fill: c-band-text,
        font: (font-name, "Inter", "Carlito"),
        if "name" in data.header { data.header.name } else { "" },
      )
      if "title" in data.header and data.header.title != none and data.header.title != "" {
        block(above: 2pt, below: sp-header-title-below,
          text(
            size: section-pt,
            style: if title-italic { "italic" } else { "normal" },
            fill: c-band-text,
            font: (font-name, "Inter", "Carlito"),
            data.header.title,
          )
        )
      }
      if "contact" in data.header and data.header.contact.len() > 0 {
        block(above: 3pt,
          text(
            size: body-pt - 1pt,
            fill: c-band-text,
            font: (font-body, "Inter", "Carlito"),
            render-runs-white(data.header.contact),
          )
        )
      }
    }))
  },
)

#set text(
  font: (font-body, "Inter", "Carlito", "Noto Sans"),
  size: body-pt,
  fill: c-body,
  lang: data.opts.lang,
)

#set par(leading: lead, spacing: sp-para)

// ── ATS-mode plain header (normal document flow; only rendered when is-ats) ────

#if is-ats {
  block(below: sp-name-below,
    text(
      size: name-pt,
      weight: "bold",
      fill: c-name,
      font: (font-name, "Inter", "Carlito"),
      if "name" in data.header { data.header.name } else { "" },
    )
  )
  if "title" in data.header and data.header.title != none and data.header.title != "" {
    block(below: sp-header-title-below,
      text(
        size: section-pt,
        style: if title-italic { "italic" } else { "normal" },
        fill: c-body,
        data.header.title,
      )
    )
  }
  if "contact" in data.header and data.header.contact.len() > 0 {
    block(below: sp-header-contact,
      text(size: body-pt, fill: c-body, render-runs(data.header.contact))
    )
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

// ── Section renderer (accent-bar marker, or plain+rule under ATS) ──────────────

#let render-section(section) = {
  let heading-text = if all-caps { upper(section.heading) } else { section.heading }
  let bold-title = entry-bold-for-section(section)

  if is-ats {
    block(above: sp-section-above, below: sp-rule-below, {
      text(size: section-pt, weight: "bold", fill: c-section,
        font: (font-heading, "Inter", "Carlito"), heading-text)
    })
    line(length: 100%, stroke: 0.5pt + c-rule)
    block(above: sp-after-rule, {
      for b in section.blocks { render-block(b, bold-title) }
    })
  } else {
    block(above: sp-section-above, below: sp-after-rule, {
      grid(
        columns: (3.5pt, auto),
        column-gutter: 5pt,
        align: (horizon, horizon),
        box(width: 3.5pt, height: section-pt * 0.72, fill: c-accent),
        text(size: section-pt, weight: "bold", fill: c-section,
          font: (font-heading, "Inter", "Carlito"), heading-text),
      )
    })
    for b in section.blocks { render-block(b, bold-title) }
  }
}

// ── Body ──────────────────────────────────────────────────────────────────────

#for section in data.sections {
  render-section(section)
}
