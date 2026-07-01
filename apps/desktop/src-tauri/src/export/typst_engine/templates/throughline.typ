// Throughline — timeline spine premium single-column template.
//
// Design contract:
//   EXPERIENCE and PROJECTS sections render as a vertical timeline: a thin
//   vertical accent line (spine segment per entry) runs down the left of the
//   section with a small filled dot (node) at each entry start. The entry title
//   and date appear to the right of the node; bullets are indented under it.
//   Other sections (summary, skills, education, …) render as normal single-column
//   blocks using the shared render-entry pattern.
//
//   Pagination safety: each entry is wrapped in block(breakable: false) so the
//   node + title + bullets stay together. The spine is drawn per-entry (as a
//   segment from node top to content bottom) rather than as one absolute line,
//   so it is never clipped at a page break.
//
// Design: original. Accent is deep forest teal (#1A5C52).
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

#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#0F322D" })
#let c-section = rgb(if "c_section" in st { st.c_section } else { "#1A5C52" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#192320" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#55786E" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#A0C8BE" })

#let c-accent = {
  if "accent" in data.opts and data.opts.accent != "" {
    rgb(data.opts.accent)
  } else if "c_accent" in st {
    rgb(st.c_accent)
  } else {
    rgb("#1A5C52")
  }
}

#let font-name    = if "font_name"    in st { st.font_name    } else { "Manrope" }
#let font-heading = if "font_heading" in st { st.font_heading } else { "Manrope" }
#let font-body    = if "font_body"    in st { st.font_body    } else { "Carlito" }

#let all-caps     = if "section_all_caps"  in st { st.section_all_caps  } else { true }
#let title-italic = if "job_title_italic"  in st { st.job_title_italic  } else { true }

#let name-pt    = if "name_pt"    in st { st.name_pt    * 1pt } else { 22pt }
#let section-pt = if "section_pt" in st { st.section_pt * 1pt } else { 11pt }
#let body-pt    = if "body_pt"    in st { st.body_pt    * 1pt } else { 10.5pt }

// Education is always non-bold in Throughline (non-academic template).
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

// ── Timeline geometry ─────────────────────────────────────────────────────────
// The spine and node sit in a left gutter; entry content flows to the right.
// node-r: radius of the filled dot.
// gutter-w: total width reserved for spine + node + gap to content.
// spine-x: x offset of the spine centre from the gutter left edge.
// content-indent: how far the entry content is pushed right from the gutter.

#let node-r       = 3.5pt
#let spine-x      = node-r          // centre of spine = centre of node
#let gutter-w     = node-r * 2 + 7pt // dot diameter + gap
#let content-indent = gutter-w

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

#block(below: sp-name-below, {
  text(
    size: name-pt,
    weight: "bold",
    fill: c-name,
    font: (font-name, "Manrope", "Inter", "Carlito"),
    if "name" in data.header { data.header.name } else { "" },
  )
})

#if "title" in data.header and data.header.title != none {
  block(below: sp-header-title-below,
    text(
      size: section-pt,
      style: if title-italic { "italic" } else { "normal" },
      fill: c-body,
      data.header.title,
    )
  )
}

#if "contact" in data.header {
  block(below: sp-header-contact,
    text(size: body-pt, fill: c-body, render-runs(data.header.contact))
  )
}

// ── Standard entry renderer (non-timeline sections) ───────────────────────────

#let render-entry-standard(blk, bold-title) = {
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

// ── Timeline entry renderer (for experience + project sections) ───────────────
// Each entry is rendered as a row:
//   left column (gutter-w): spine segment + node dot
//   right column (1fr):     title/date grid + subtitle + bullets
//
// The spine is a per-entry segment (from node centre to the bottom of the
// content) rather than one continuous absolute line, so page breaks do not
// clip it. block(breakable: false) keeps each node+content pair together.

#let render-entry-timeline(blk, bold-title) = {
  let title-content = if "title" in blk { render-runs(blk.title) } else { "" }
  let date-str = if "date" in blk and blk.date != none { blk.date } else { "" }
  let title-weight = if bold-title { "bold" } else { "regular" }

  block(breakable: false, width: 100%, above: 0pt, below: sp-entry, {
    // Use a grid: narrow gutter for the spine/node, wide column for content.
    grid(
      columns: (gutter-w, 1fr),
      gutter: 0pt,
      // Left cell: node dot centred horizontally in the gutter.
      align(center + top,
        circle(
          radius: node-r,
          fill: c-accent,
          stroke: none,
        )
      ),
      // Right cell: entry content.
      {
        // Title + date row.
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
      },
    )
  })
}

#let entry-bold-for-section(section) = {
  let kind = if "kind" in section { section.kind } else { "" }
  if kind == "education" { emphasize-edu } else { true }
}

// ── Block renderer ────────────────────────────────────────────────────────────

#let render-block-standard(b, bold-title) = {
  if b.kind == "paragraph" {
    if "runs" in b {
      block(below: 4pt, render-runs(b.runs))
    }
  } else if b.kind == "bullet" {
    if "runs" in b {
      list.item(render-runs(b.runs))
    }
  } else if b.kind == "entry" {
    block(below: sp-entry, render-entry-standard(b, bold-title))
  }
}

#let render-block-timeline(b, bold-title) = {
  if b.kind == "paragraph" {
    if "runs" in b {
      block(below: 4pt, render-runs(b.runs))
    }
  } else if b.kind == "bullet" {
    if "runs" in b {
      list.item(render-runs(b.runs))
    }
  } else if b.kind == "entry" {
    // Spine+node rendered inside render-entry-timeline.
    render-entry-timeline(b, bold-title)
  }
}

// ── Section renderer ──────────────────────────────────────────────────────────
// Experience and Projects sections use the timeline renderer;
// all other sections use the standard renderer.

#let render-section(section) = {
  let heading-text = if all-caps { upper(section.heading) } else { section.heading }
  let kind = if "kind" in section { section.kind } else { "" }
  let is-timeline = kind == "experience" or kind == "projects"
  let bold-title = entry-bold-for-section(section)

  // Section heading with ruled-bottom divider.
  block(above: sp-section-above, below: sp-rule-below, {
    text(
      size: section-pt,
      weight: "bold",
      fill: c-section,
      font: (font-heading, "Manrope", "Inter", "Carlito"),
      heading-text,
    )
  })
  line(length: 100%, stroke: 0.5pt + c-rule)

  block(above: sp-after-rule, {
    if is-timeline {
      for b in section.blocks {
        render-block-timeline(b, bold-title)
      }
    } else {
      for b in section.blocks {
        render-block-standard(b, bold-title)
      }
    }
  })
}

// ── Body ──────────────────────────────────────────────────────────────────────

#for section in data.sections {
  render-section(section)
}
