// ATS Classic template for the Typst rendering engine.
//
// Design contract (mirrors Template::classic() in templates/mod.rs):
//   - Font:      Carlito (Calibri-metric-compatible, OFL-1.1), body 10.5 pt
//   - Colors:    near-black only — no accent color for maximum ATS compatibility
//   - Name:      left-aligned, 20 pt, bold
//   - Sections:  ALL-CAPS heading, 11 pt, full-width rule below (RuledBottom)
//   - Entries:   title bold | subtitle italic | date right-aligned | bullets
//   - Margins:   1 in (25.4 mm) on all sides
//   - Single column → reading order is always linear (ATS-safe by construction)
//   - opts.ats flag is a no-op here (single column already is ATS-safe)
//
// Note on section_style: Template::classic() declares SectionStyle::RuledBottom
// (full-width rule) which matches the rendered output here (line below heading).
//
// Data contract (consumed from data.json via `#let data = json("data.json")`):
//   data.opts.page_width_mm / page_height_mm  — page geometry
//   data.opts.lang                             — language tag (unused visually)
//   data.opts.ats                              — ATS flag (no-op for single col)
//   data.header.name                           — candidate name string
//   data.header.title                          — optional title string
//   data.header.contact[]                      — rich-text runs for contact line
//   data.sections[].heading                    — section heading string
//   data.sections[].blocks[]                   — typed blocks (paragraph/bullet/entry)
//
// Guard: every optional dict key is checked with `"key" in dict` before access.

// ── House spacing scale (centralized) ─────────────────────────────────────────
// All spacing constants are defined in _scale.typ and prepended by engine.rs.
// Do NOT redeclare them here.

// ── Page setup ────────────────────────────────────────────────────────────────

#set page(
  width:  (data.opts.page_width_mm  * 1mm),
  height: (data.opts.page_height_mm * 1mm),
  margin: (x: 25.4mm, y: 25.4mm),
)

// ── Typography ─────────────────────────────────────────────────────────────────

#set text(
  font:   ("Carlito", "Inter", "Noto Sans"),
  size:   10.5pt,
  fill:   rgb("#222222"),
  lang:   data.opts.lang,
)

#set par(leading: lead, spacing: sp-para)

// ── Color palette — near-black only ───────────────────────────────────────────

#let c-name    = rgb("#111111")
#let c-section = rgb("#111111")
#let c-body    = rgb("#222222")
#let c-date    = rgb("#555555")
#let c-rule    = rgb("#aaaaaa")

// ── Rich-text helper ──────────────────────────────────────────────────────────
// Converts a runs array (from data.json) into Typst inline content.
// Each run has: text, bold, italic, link (optional).

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
      link(r.link, t)
    } else {
      t
    }
  }
}

// ── Header ────────────────────────────────────────────────────────────────────

#block(below: sp-name-below, {
  text(size: 20pt, weight: "bold", fill: c-name, data.header.name)
})

#if "title" in data.header and data.header.title != none {
  block(below: 2pt, {
    text(size: 11pt, style: "italic", fill: c-body, data.header.title)
  })
}

// Contact line — inline rich runs (links are real hyperlinks)
#if "contact" in data.header {
  block(below: sp-header-contact, {
    text(size: 10pt, fill: c-body, render-runs(data.header.contact))
  })
}

// ── Section renderer ──────────────────────────────────────────────────────────

// render-entry: renders an entry block.
// bold-title: when true the title + date are bold; when false they are normal weight.
// Classic and Atelier never emphasize education, so callers pass
// (section.kind != "education") as the bold-title argument.
#let render-entry(blk, bold-title) = {
  // Title row: title + right-aligned date on the same line.
  // Weight follows bold-title: bold for most sections, normal for education.
  let title-content = if "title" in blk { render-runs(blk.title) } else { "" }
  let date-str = if "date" in blk and blk.date != none { blk.date } else { "" }
  let title-weight = if bold-title { "bold" } else { "regular" }

  grid(
    columns: (1fr, auto),
    gutter: 4pt,
    {
      text(weight: title-weight, fill: c-body, title-content)
    },
    {
      text(weight: title-weight, fill: c-date, date-str)
    },
  )

  // Subtitle (italic) — weight unchanged regardless of section kind.
  if "subtitle" in blk and blk.subtitle != none and blk.subtitle.len() > 0 {
    block(above: sp-subtitle-gap, below: sp-subtitle-below, {
      text(style: "italic", fill: c-body, render-runs(blk.subtitle))
    })
  }

  // Bullets — unchanged.
  if "bullets" in blk and blk.bullets.len() > 0 {
    block(above: sp-bullet-above, below: 0pt, {
      set list(spacing: sp-bullet-gap)
      for bullet in blk.bullets {
        list.item(render-runs(bullet))
      }
    })
  }
}

// Decide whether entry titles should be bold for a given section.
// Classic never emphasizes education: education titles render at normal weight.
#let entry-bold-for-section(section) = {
  let kind = if "kind" in section { section.kind } else { "" }
  kind != "education"
}

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

#let render-section(section) = {
  // Section heading: ALL-CAPS, 11 pt, full-width rule underneath (RuledBottom).
  block(above: sp-section-above, below: sp-rule-below, {
    text(
      size: 11pt,
      weight: "bold",
      fill: c-section,
      upper(section.heading),
    )
  })
  line(length: 100%, stroke: 0.5pt + c-rule)
  let bold-title = entry-bold-for-section(section)
  block(above: sp-after-rule, {
    for b in section.blocks {
      render-block(b, bold-title)
    }
  })
}

// ── Body ──────────────────────────────────────────────────────────────────────

#for section in data.sections {
  render-section(section)
}
