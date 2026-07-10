// Parametric single-column template for the Typst rendering engine.
//
// Design contract:
//   All colors, fonts, and section presentation are driven by `data.style`
//   (populated from the Template registry in render.rs). The house spacing
//   scale comes from _scale.typ (prepended by engine.rs).
//
// Data contract:
//   data.style.c_name / c_section / c_accent / c_body / c_date / c_rule
//             — color hex strings (#RRGGBB)
//   data.style.font_name / font_heading / font_body — Typst font family names
//   data.style.section_style — "ruled-bottom" | "underline" | "bold-only"
//   data.style.name_centered — bool: centre the name block
//   data.style.section_all_caps — bool: uppercase section headings
//   data.style.section_small_caps — bool: wrap heading text in Typst smallcaps(…)
//             (small-caps glyph variant; the text layer keeps its original case)
//             at 0.85 × section_pt
//   data.style.job_title_italic — bool: italic job title under entry
//   data.style.heading_tracking — number: extra heading letter-spacing in em (0 = none)
//   data.style.link_underline — bool: underline hyperlinked runs when true
//   data.style.rule_thickness — number: ruled-bottom stroke thickness in pt
//             (0 or absent falls back to the house 0.5pt)
//   data.style.name_pt / section_pt / body_pt — font sizes in pt
//   data.opts.page_width_mm / page_height_mm — page geometry
//   data.opts.accent — optional override (#RRGGBB or "")
//   data.opts.lang — BCP-47 language tag
//   data.opts.ats — ATS flag (no-op for single column)
//   data.header.name — candidate name
//   data.header.title — optional professional title string
//   data.header.contact[] — rich-text runs for contact line
//   data.sections[].heading — section heading string
//   data.sections[].blocks[] — typed blocks (paragraph/bullet/entry)
//   (data.sections[].placement is ignored — all sections are main-column)
//
// Guard: every optional dict key is checked with `"key" in dict` before access.
// Spacing scale constants come from _scale.typ (prepended by engine.rs).

// ── Style resolution ──────────────────────────────────────────────────────────

// Resolve style from data.style; fall back to safe defaults if absent.
#let st = if "style" in data { data.style } else { (:) }

#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#111111" })
#let c-section = rgb(if "c_section" in st { st.c_section } else { "#111111" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#222222" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#555555" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#aaaaaa" })

// Accept a per-render accent override; else use data.style.c_accent; else grey.
#let c-accent = {
  if "accent" in data.opts and data.opts.accent != "" {
    rgb(data.opts.accent)
  } else if "c_accent" in st {
    rgb(st.c_accent)
  } else {
    rgb("#444444")
  }
}

#let font-name    = if "font_name"    in st { st.font_name    } else { "Carlito" }
#let font-heading = if "font_heading" in st { st.font_heading } else { "Carlito" }
#let font-body    = if "font_body"    in st { st.font_body    } else { "Carlito" }

#let section-style   = if "section_style"   in st { st.section_style   } else { "ruled-bottom" }
#let name-centered   = if "name_centered"   in st { st.name_centered   } else { false }
#let all-caps        = if "section_all_caps" in st { st.section_all_caps } else { false }
#let small-caps-flag = if "section_small_caps" in st { st.section_small_caps } else { false }
#let title-italic    = if "job_title_italic" in st { st.job_title_italic } else { true }

// PR3 knobs. Defaults are neutral: heading-tracking 0 emits no `tracking:` arg
// and link-underline false emits no `underline(…)` wrapper, so pre-PR3 templates
// render byte-identically.
#let heading-tracking = if "heading_tracking" in st { st.heading_tracking } else { 0.0 }
#let link-underline   = if "link_underline"   in st { st.link_underline   } else { false }

// Rule-stroke thickness: fall back to the house 0.5pt when absent OR 0.0 (every
// pre-PR3 ruled template already ships 0.5, so this is byte-identical for them;
// only Cadence sets a real 0.75 override).
#let rule-thickness = if "rule_thickness" in st and st.rule_thickness != 0.0 { st.rule_thickness } else { 0.5 }

#let name-pt    = if "name_pt"    in st { st.name_pt    * 1pt } else { 20pt }
#let section-pt = if "section_pt" in st { st.section_pt * 1pt } else { 11pt }
#let body-pt    = if "body_pt"    in st { st.body_pt    * 1pt } else { 10.5pt }

// When true, education entry titles are rendered bold (same as other entries).
// Only the Academic template sets this to true; all others default to false
// so that education entries are de-emphasized (normal weight).
#let emphasize-edu = if "emphasize_education" in st { st.emphasize_education } else { false }

// ── Page & typography setup ───────────────────────────────────────────────────

#set page(
  width:  (data.opts.page_width_mm  * 1mm),
  height: (data.opts.page_height_mm * 1mm),
  margin: (x: 25.4mm, y: 25.4mm),
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
      let styled = text(fill: c-accent, t)
      link(r.link, if link-underline { underline(styled) } else { styled })
    } else {
      t
    }
  }
}

// ── Header ────────────────────────────────────────────────────────────────────

#block(below: sp-name-below, {
  let name-text = text(
    size: name-pt,
    weight: "bold",
    fill: c-name,
    font: (font-name, "Carlito", "Inter"),
    data.header.name,
  )
  if name-centered {
    align(center, name-text)
  } else {
    name-text
  }
})

#if "title" in data.header and data.header.title != none {
  let t = text(
    size: section-pt,
    style: if title-italic { "italic" } else { "normal" },
    fill: c-body,
    data.header.title,
  )
  block(below: sp-header-title-below, if name-centered { align(center, t) } else { t })
}

#if "contact" in data.header {
  block(below: sp-header-contact, {
    text(size: body-pt, fill: c-body, render-runs(data.header.contact))
  })
}

// ── Entry renderer ────────────────────────────────────────────────────────────
//
// bold-title: when true the title + date are bold; when false normal weight.
// The caller (render-section) decides this based on section.kind and
// the emphasize-edu flag from data.style.

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

// Decide whether entry titles should be bold for a given section.
// Education sections are de-emphasized (normal weight) unless emphasize-edu is true.
#let entry-bold-for-section(section) = {
  let kind = if "kind" in section { section.kind } else { "" }
  if kind == "education" { emphasize-edu } else { true }
}

// ── Block renderer ────────────────────────────────────────────────────────────

#let render-block(b, bold-title) = {
  if b.kind == "paragraph" {
    if "runs" in b {
      block(below: 4pt, render-runs(b.runs))
    }
  } else if b.kind == "bullet" {
    if "runs" in b {
      list.item(render-runs(b.runs))
    }
  } else if b.kind == "entry" {
    block(below: sp-entry, render-entry(b, bold-title))
  }
}

// ── Section renderer ──────────────────────────────────────────────────────────

// Render a section heading. `tracking:` is only passed when heading-tracking is
// non-zero, so templates that leave it at the 0 default (every pre-PR3 template)
// produce byte-identical markup to before the knob was added.
#let heading-run(content, size) = {
  if heading-tracking != 0.0 {
    text(
      size: size,
      weight: "bold",
      fill: c-section,
      font: (font-heading, "Carlito", "Inter"),
      tracking: heading-tracking * 1em,
      content,
    )
  } else {
    text(
      size: size,
      weight: "bold",
      fill: c-section,
      font: (font-heading, "Carlito", "Inter"),
      content,
    )
  }
}

#let render-section(section) = {
  let heading-text = {
    let base = if all-caps { upper(section.heading) } else { section.heading }
    // smallcaps(…) applies the small-caps glyph variant while keeping the
    // underlying text-layer characters in their original case (extraction-safe).
    if small-caps-flag { smallcaps(base) } else { base }
  }
  let heading-size = if small-caps-flag { section-pt * 0.85 } else { section-pt }
  let bold-title = entry-bold-for-section(section)

  if section-style == "ruled-bottom" {
    block(above: sp-section-above, below: sp-rule-below, {
      heading-run(heading-text, heading-size)
    })
    line(length: 100%, stroke: (rule-thickness * 1pt) + c-rule)
    block(above: sp-after-rule, {
      for b in section.blocks { render-block(b, bold-title) }
    })
  } else if section-style == "underline" {
    block(above: sp-section-above, below: sp-after-rule, {
      heading-run(underline(heading-text), heading-size)
    })
    for b in section.blocks { render-block(b, bold-title) }
  } else {
    // bold-only
    block(above: sp-section-above, below: sp-after-rule, {
      heading-run(heading-text, heading-size)
    })
    for b in section.blocks { render-block(b, bold-title) }
  }
}

// ── Body ──────────────────────────────────────────────────────────────────────

#for section in data.sections {
  render-section(section)
}
