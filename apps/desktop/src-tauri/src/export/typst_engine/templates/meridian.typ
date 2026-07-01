// Meridian — header-forward band premium single-column template.
//
// Design contract:
//   A full-width tinted header band spans the top of the page, holding the
//   candidate name (large, bold, white), optional professional title (italic,
//   white), and the contact line (smaller, white). The band is filled with the
//   accent color so it prints on paper. A thin accent keyline separates the band
//   from the body. Below the band: an airy single-column body (summary,
//   experience, …) using the shared _scale.typ rhythm and the render-entry
//   pattern (bold job/project titles, education non-bold).
//   Section headings appear in the accent color with a ruled-bottom divider.
//
// Design: original. Accent is warm copper-sienna (#A0522D).
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
//   data.opts.lang
//   data.header.name / title / contact[]
//   data.sections[].heading / blocks[] / kind
//
// Guard: every optional dict key is checked before access.
// Spacing scale constants come from _scale.typ (prepended by engine.rs).

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-section = rgb(if "c_section" in st { st.c_section } else { "#A0522D" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#1E1914" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#786450" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#D2AA8C" })

// Accept a per-render accent override; else use data.style.c_accent; else copper.
#let c-accent = {
  if "accent" in data.opts and data.opts.accent != "" {
    rgb(data.opts.accent)
  } else if "c_accent" in st {
    rgb(st.c_accent)
  } else {
    rgb("#A0522D")
  }
}

// Header band uses accent color as fill; text is white for contrast.
#let c-band-fill = c-accent
#let c-band-text = rgb("#FFFFFF")

#let font-name    = if "font_name"    in st { st.font_name    } else { "Inter" }
#let font-heading = if "font_heading" in st { st.font_heading } else { "Inter" }
#let font-body    = if "font_body"    in st { st.font_body    } else { "Inter" }

#let all-caps    = if "section_all_caps"  in st { st.section_all_caps  } else { true }
#let title-italic = if "job_title_italic" in st { st.job_title_italic  } else { true }

#let name-pt    = if "name_pt"    in st { st.name_pt    * 1pt } else { 26pt }
#let section-pt = if "section_pt" in st { st.section_pt * 1pt } else { 11pt }
#let body-pt    = if "body_pt"    in st { st.body_pt    * 1pt } else { 10.5pt }

// Education is always non-bold in Meridian (non-academic template).
#let emphasize-edu = if "emphasize_education" in st { st.emphasize_education } else { false }

// ── Layout constants ──────────────────────────────────────────────────────────

#let page-w = data.opts.page_width_mm  * 1mm
#let page-h = data.opts.page_height_mm * 1mm

// Band height: generous to accommodate name + title + contact.
#let band-h     = 38mm
// Keyline thickness below the band.
#let keyline-pt = 2.5pt

// Horizontal padding inside the band (aligns with body margin).
#let body-margin-h = 22mm
#let body-margin-top = 8mm     // space from band bottom to first section

// ── Page setup ────────────────────────────────────────────────────────────────
// Zero top margin — we draw the band manually in the page background.
// The body top margin is added as a pad block after the band.

#set page(
  width:  page-w,
  height: page-h,
  margin: (
    top:    band-h + keyline-pt + body-margin-top,
    bottom: 18mm,
    left:   body-margin-h,
    right:  body-margin-h,
  ),
  background: {
    // Full-width accent band at the top.
    place(top + left,
      rect(width: 100%, height: band-h, fill: c-band-fill)
    )
    // Thin keyline immediately below the band.
    place(top + left, dy: band-h,
      line(length: 100%, stroke: keyline-pt + c-accent)
    )
    // Header text placed inside the band.
    place(top + left, dx: body-margin-h, dy: 0pt, {
      // Vertical padding inside the band: center content.
      let name-text = text(
        size: name-pt,
        weight: "bold",
        fill: c-band-text,
        font: (font-name, "Inter", "Carlito"),
        if "name" in data.header { data.header.name } else { "" },
      )

      // Render runs in white for the contact line.
      let render-runs-white(runs) = {
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
            // Links appear white (band bg) — same fill as surrounding text.
            link(r.link, text(fill: c-band-text, t))
          } else {
            t
          }
        }
      }

      // Calculate vertical offset to center name+contact stack inside the band.
      // Rough vertical centering: pad 7mm from top of band.
      pad(top: 7mm, {
        name-text
        if "title" in data.header and data.header.title != none {
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
        if "contact" in data.header {
          block(above: 3pt,
            text(
              size: body-pt - 1pt,
              fill: c-band-text,
              font: (font-body, "Inter", "Carlito"),
              render-runs-white(data.header.contact),
            )
          )
        }
      })
    })
  },
)

#set text(
  font: (font-body, "Inter", "Carlito", "Noto Sans"),
  size: body-pt,
  fill: c-body,
  lang: data.opts.lang,
)

#set par(leading: lead, spacing: sp-para)

// ── Rich-text helper (body) ───────────────────────────────────────────────────

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

#let render-section(section) = {
  let heading-text = if all-caps { upper(section.heading) } else { section.heading }
  let bold-title = entry-bold-for-section(section)

  block(above: sp-section-above, below: sp-rule-below, {
    text(
      size: section-pt,
      weight: "bold",
      fill: c-accent,
      font: (font-heading, "Inter", "Carlito"),
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
