// Cologne Navy — centred-header single-column template.
//
// Design contract:
//   A centred four-line header (name in tracked navy caps, tagline, contact
//   line, links line), navy rule-underlined uppercase section headings, bold
//   job titles with the company in a lighter blue, and right-aligned italic
//   date ranges sharing a baseline with the title via a two-column grid.
//   Bullets use a navy marker with zero indent. Experience blocks are
//   BREAKABLE on purpose: forcing them whole leaves a dead zone at the foot of
//   a page. Entries elsewhere stay unbreakable.
//
// Design: supplied reference implementation, ported to this repo's IR.
// ORIGINALITY: independent design based on generic layout conventions only.
//
// Ported from a standalone reference that consumed its own rich schema
// (`basics`/`skills` groups/`projects` with `stack`+`links`/`footnotes`). This
// repo has ONE generic IR shared by every template, so what is ported is the
// LOOK, not the schema — the same way `meridian.typ` and `throughline.typ`
// style the identical `data.sections[].blocks[]` differently. Fields with no IR
// equivalent (project tech-stack lines, per-project link lists, `footnotes`
// label/value rows) simply arrive as ordinary runs/bullets and are styled as
// such; nothing is invented and no section is dropped.
//
// Data contract (same as single_column.typ):
//   data.style.c_name / c_section / c_accent / c_body / c_date / c_rule
//   data.style.font_name / font_heading / font_body
//   data.style.section_all_caps — bool
//   data.style.name_pt / section_pt / body_pt
//   data.opts.page_width_mm / page_height_mm
//   data.opts.accent — optional override (#RRGGBB or "")
//   data.opts.lang
//   data.header.name / title / contact[]
//   data.sections[].heading / blocks[] / kind
//
// Guard: every optional dict key is checked before access.
// Spacing scale constants come from _scale.typ (prepended by engine.rs).
//
// DEVIATION FROM THE SUPPLIED DESIGN — letter tracking.
// The brief specifies 0.14em on the name and 0.18em on section headings. At
// those values the rendered PDF does not extract as words: the name comes back
// as "À LVA R O   È S P O S I T O" and headings as "S U M M A R Y", because
// tracking wide enough relative to the glyph advance is read as inter-word
// spacing by PDF text extraction — and therefore by an ATS. This app is a
// job-application tool, so a name no parser can read is not a cosmetic issue.
// Reduced to 0.06em (name) and 0.10em (headings), the widest values that still
// extract cleanly, and inside the 0.03–0.10em band every other template here
// already uses. Pinned by `every_template_extracts_accented_latin_content`,
// which fails at the brief's values.
//
// The reference expressed its whole type scale as multiples of a single `base`.
// That property is preserved HERE as ratios off `body-pt` (below), but `base`
// is deliberately NOT a per-template option: this repo has no per-template size
// knob, and `_scale.typ` is a locked shared house scale. Adding one would be a
// pipeline change across all templates, not a template addition.

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#1F3864" })
#let c-section = rgb(if "c_section" in st { st.c_section } else { "#1F3864" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#1A1A1A" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#4A4A4A" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#1F3864" })

// Per-render accent override wins, then the registry accent, then navy.
#let c-accent = {
  let o = if "opts" in data and "accent" in data.opts { data.opts.accent } else { "" }
  if o != "" { rgb(o) } else if "c_accent" in st { rgb(st.c_accent) } else { rgb("#1F5C99") }
}

// Muted tones for the tagline/contact lines and job sub-lines. Derived, not
// configurable — they are part of this template's identity.
#let c-muted   = rgb("#3D4D63")
#let c-subtle  = rgb("#5A5A5A")

#let font-name = if "font_name" in st { st.font_name } else { "Carlito" }
#let font-body = if "font_body" in st { st.font_body } else { "Carlito" }
#let font-head = if "font_heading" in st { st.font_heading } else { font-name }

#let all-caps = if "section_all_caps" in st { st.section_all_caps } else { true }

// Read from the registry rather than hardcoded, so `Template::cologne_navy`'s
// `heading_tracking` is the single source of truth — a hardcode here made that
// field dead AND wrong (it said 0.18 while this rendered 0.10).
#let heading-tracking = if "heading_tracking" in st { st.heading_tracking } else { 0.10 }

#let name-pt    = if "name_pt"    in st { st.name_pt * 1pt } else { 20.8pt }
#let section-pt = if "section_pt" in st { st.section_pt * 1pt } else { 9.5pt }
#let body-pt    = if "body_pt"    in st { st.body_pt * 1pt } else { 10pt }

// Type scale as multiples of the body size — the reference's defining property.
#let sz-tagline    = body-pt * 1.08
#let sz-contact    = body-pt * 0.94
#let sz-job-title  = body-pt * 1.05
#let sz-date       = body-pt * 0.91
#let sz-sub        = body-pt * 0.88

// ── Page ──────────────────────────────────────────────────────────────────────

#let page-w = if "opts" in data and "page_width_mm" in data.opts { data.opts.page_width_mm } else { 210 }
#let page-h = if "opts" in data and "page_height_mm" in data.opts { data.opts.page_height_mm } else { 297 }
#let doc-lang = if "opts" in data and "lang" in data.opts { data.opts.lang } else { "en" }

#set page(
  width: page-w * 1mm,
  height: page-h * 1mm,
  margin: (top: 13mm, bottom: 12mm, x: 14mm),
)
#set text(
  font: (font-body, "Carlito", "Inter", "Noto Sans"),
  size: body-pt,
  fill: c-body,
  lang: doc-lang,
)
#set par(leading: lead, justify: false)

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

// Separator used between inline items in the header. `0.45em` of air either
// side, per the design.
#let sep = [#h(0.45em)·#h(0.45em)]

// ── Header ────────────────────────────────────────────────────────────────────

#let header = if "header" in data { data.header } else { (:) }

#align(center)[
  #text(
    font: (font-name, "Carlito", "Inter"),
    size: name-pt,
    weight: "bold",
    fill: c-name,
    tracking: 0.06em,
    upper(if "name" in header { header.name } else { "" }),
  )

  #if "title" in header and header.title != none and header.title != "" {
    v(sp-header-title-below, weak: true)
    text(size: sz-tagline, fill: c-muted, header.title)
  }

  #if "contact" in header and header.contact.len() > 0 {
    v(sp-name-below, weak: true)
    // The reference had separate `contact` and `links` lines. This IR carries
    // ONE contact list whose runs may each hold a `link`, so they render as a
    // single line — splitting it would mean guessing which items are "links".
    text(size: sz-contact, fill: c-muted, header.contact.map(r => render-runs((r,))).join(sep))
  }
]

#v(sp-header-contact)

// ── Section heading ───────────────────────────────────────────────────────────

#let render-heading(title) = {
  block(above: sp-section-above, below: sp-after-rule, {
    text(
      font: (font-head, "Carlito", "Inter"),
      size: section-pt,
      weight: "bold",
      fill: c-section,
      tracking: heading-tracking * 1em,
      if all-caps { upper(title) } else { title },
    )
    v(sp-rule-below, weak: true)
    line(length: 100%, stroke: 0.9pt + c-rule)
  })
}

// ── Entry ─────────────────────────────────────────────────────────────────────

// `breakable` is a parameter, not a constant: experience entries must reflow
// (a forced-whole job block left a three-inch dead zone at the foot of page 1
// during the original design), while projects and education stay whole.
#let render-entry(blk, breakable-entry) = {
  let title-content = if "title" in blk { render-runs(blk.title) } else { "" }
  let date-str = if "date" in blk and blk.date != none { blk.date } else { "" }

  block(breakable: breakable-entry, width: 100%, {
    // `align: (left + bottom, right + bottom)` so the smaller italic date sits
    // on the same baseline as the larger bold title.
    grid(
      columns: (1fr, auto),
      column-gutter: 1em,
      align: (left + bottom, right + bottom),
      text(size: sz-job-title, weight: "bold", fill: c-body, title-content),
      text(size: sz-date, style: "italic", fill: c-date, date-str),
    )

    if "subtitle" in blk and blk.subtitle != none and blk.subtitle.len() > 0 {
      block(above: sp-subtitle-gap, below: sp-subtitle-below,
        text(size: sz-sub, style: "italic", fill: c-subtle, render-runs(blk.subtitle)),
      )
    }

    if "bullets" in blk and blk.bullets.len() > 0 {
      block(above: sp-bullet-above, below: 0pt, {
        set list(
          marker: text(fill: c-section, [•]),
          indent: 0pt,
          body-indent: 0.55em,
          spacing: sp-bullet-gap,
        )
        for bullet in blk.bullets {
          list.item(render-runs(bullet))
        }
      })
    }
  })
}

// ── Blocks ────────────────────────────────────────────────────────────────────

#let render-block(b, breakable-entry) = {
  if b.kind == "paragraph" {
    if "runs" in b { block(below: sp-para, render-runs(b.runs)) }
  } else if b.kind == "bullet" {
    if "runs" in b {
      set list(
        marker: text(fill: c-section, [•]),
        indent: 0pt,
        body-indent: 0.55em,
        spacing: sp-bullet-gap,
      )
      list.item(render-runs(b.runs))
    }
  } else if b.kind == "entry" {
    block(below: sp-entry, render-entry(b, breakable-entry))
  }
}

// ── Sections ──────────────────────────────────────────────────────────────────

#let sections = if "sections" in data { data.sections } else { () }

#for section in sections {
  // Every section is individually optional: an empty block list renders NOTHING
  // — no orphan heading, no stray rule. The reference guarded this per known
  // section name; here it is one check that covers current and future kinds.
  if "blocks" in section and section.blocks.len() > 0 {
    let kind = if "kind" in section { section.kind } else { "" }
    // Only experience reflows; see `render-entry`.
    let breakable-entry = kind == "experience"
    render-heading(if "heading" in section { section.heading } else { "" })
    for b in section.blocks { render-block(b, breakable-entry) }
  }
}
