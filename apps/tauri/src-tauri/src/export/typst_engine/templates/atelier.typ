// Atelier — premium two-column sidebar template for the Typst rendering engine.
//
// Design contract (mirrors Template::atelier() in templates/mod.rs):
//   Accent:     Slate-indigo (#4A4580) — deep, sophisticated, distinct from any
//               commercial product. Overridable via data.opts.accent.
//   Fonts:      Source Serif 4 (name + main column headings/body),
//               Inter (sidebar headings + content).
//   Sidebar:    Full-height tinted band (warm light grey #F0EFF8) rendered via
//               page(background: ...) so it repeats on EVERY page.
//               The sidebar content (skills/education/languages) is ALSO placed
//               in the page background so it repeats on every page alongside
//               the band.
//               Width: 30 % of page width (sidebar_w).
//   Main col:   Main content flows in the normal document flow, inset left by
//               sidebar_w + gutter so it never overlaps the sidebar band.
//   Header:     Name + contact spans FULL PAGE WIDTH at the top of page 1
//               (the header is padded so it starts at the sidebar edge, giving
//               visual balance while technically spanning the content area).
//   Entries:    Each entry wrapped in block(breakable: false).
//   ATS mode:   When data.opts.ats == true, render a plain single column —
//               no background band, all sections in linear reading order
//               (main sections first, then sidebar sections).
//   Dense-sidebar fallback: when sidebar content is taller than the page's
//               available height (measured via context + measure()), the
//               document is rendered in single-column layout to prevent silent
//               data loss from place()-based clipping.
//   Empty-sidebar fallback: when no sections are assigned to the sidebar,
//               render single-column with no band.
//
// Data contract (from data.json via `#let data = json("data.json")`):
//   data.opts.page_width_mm / page_height_mm  — page geometry
//   data.opts.accent                          — optional override (#RRGGBB or "")
//   data.opts.lang                            — BCP-47 language tag
//   data.opts.ats                             — ATS linearise flag
//   data.header.name                          — candidate name
//   data.header.title                         — optional professional title
//   data.header.contact[]                     — rich-text runs for contact line
//   data.sections[].heading                   — section heading string
//   data.sections[].placement                 — "main" or "sidebar"
//   data.sections[].blocks[]                  — typed blocks
//
// Guard: every optional dict key is checked with `"key" in dict` before access.

// ── Accent resolution ─────────────────────────────────────────────────────────

#let resolved-accent = if "accent" in data.opts and data.opts.accent != "" {
  rgb(data.opts.accent)
} else {
  rgb("#4A4580")
}

// ── Layout constants ──────────────────────────────────────────────────────────

#let page_w       = data.opts.page_width_mm * 1mm
#let page_h       = data.opts.page_height_mm * 1mm
#let sidebar_frac = 0.30
#let sidebar_w    = sidebar_frac * page_w
#let gutter       = 9mm
#let margin_v     = 14mm
#let margin_r     = 14mm
// Main content left margin = sidebar band width + a gutter from the band edge.
#let main_left    = sidebar_w + gutter
// ATS-mode left margin — decoupled from main_left for a comfortable page edge.
#let ats_left     = 20mm

// Sidebar background tint — warm light grey.
#let c-sidebar-bg  = rgb("#F0EFF8")

// ── House spacing scale (centralized) ─────────────────────────────────────────
// All spacing constants are defined in _scale.typ and prepended by engine.rs.
// Do NOT redeclare them here.

// ── Color palette ─────────────────────────────────────────────────────────────

#let c-name    = rgb("#16143A")
#let c-section = resolved-accent
#let c-body    = rgb("#1E1C32")
#let c-date    = rgb("#6E69AB")
#let c-rule    = resolved-accent
#let c-contact = rgb("#3C3860")

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
      link(r.link, text(fill: resolved-accent, t))
    } else {
      t
    }
  }
}

// ── Entry renderer ────────────────────────────────────────────────────────────
// block(breakable: false) prevents an entry from splitting across a page break.
//
// bold-title: when true the title + date are bold; when false normal weight.
// Atelier never emphasizes education, so callers derive this from section.kind.

#let render-entry(blk, bold-title) = {
  let title-content = if "title" in blk { render-runs(blk.title) } else { "" }
  let date-str = if "date" in blk and blk.date != none { blk.date } else { "" }
  let title-weight = if bold-title { "bold" } else { "regular" }

  block(breakable: false, width: 100%, {
    grid(
      columns: (1fr, auto),
      gutter: 4pt,
      text(weight: title-weight, fill: c-body, title-content),
      text(weight: title-weight, fill: c-date, size: 9.5pt, date-str),
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
// Atelier never emphasizes education: education titles render at normal weight.
#let entry-bold-for-section(section) = {
  let kind = if "kind" in section { section.kind } else { "" }
  kind != "education"
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

// ── Section renderer — main column ────────────────────────────────────────────

#let render-main-section(section) = {
  block(above: sp-section-above, below: sp-rule-below, {
    text(
      size: 11pt,
      weight: "bold",
      fill: c-section,
      font: ("Source Serif 4", "Carlito", "Inter"),
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

// ── Section renderer — sidebar ────────────────────────────────────────────────
// Inter (sans) for a visual contrast against the main column's serif.

#let render-sidebar-section(section) = {
  block(above: sp-sb-section-above, below: sp-sb-rule-below, {
    text(
      size: 9.5pt,
      weight: "bold",
      fill: resolved-accent,
      font: ("Inter", "Carlito", "Noto Sans"),
      upper(section.heading),
    )
  })
  line(length: 100%, stroke: 0.4pt + c-rule)
  let bold-title = entry-bold-for-section(section)
  block(above: sp-sb-after-rule, {
    set text(
      font: ("Inter", "Carlito", "Noto Sans"),
      size: 9pt,
      fill: c-body,
    )
    set par(leading: sb-lead)
    for b in section.blocks {
      if b.kind == "paragraph" {
        if "runs" in b {
          block(below: sp-sb-item, render-runs(b.runs))
        }
      } else if b.kind == "bullet" {
        if "runs" in b {
          block(below: sp-sb-item, list.item(render-runs(b.runs)))
        }
      } else if b.kind == "entry" {
        block(below: sp-sb-item, render-entry(b, bold-title))
      }
    }
  })
}

// ── Partition sections ────────────────────────────────────────────────────────

#let main-sections    = data.sections.filter(s => (if "placement" in s { s.placement } else { "main" }) == "main")
#let sidebar-sections = data.sections.filter(s => (if "placement" in s { s.placement } else { "main" }) == "sidebar")

// ── Empty-sidebar detection ───────────────────────────────────────────────────
// When no sections are assigned to the sidebar there is nothing to put in the
// tinted band; render single-column with no band instead.

#let has-sidebar = sidebar-sections.len() > 0

// ── Sidebar content block ─────────────────────────────────────────────────────
// Built as a box so it can be measured (for dense-sidebar detection) and placed
// into page(background: ...) for the two-column layout.

#let sidebar-content-inner = box(
  width: sidebar_w - 2 * gutter,
  {
    set text(font: ("Inter", "Carlito", "Noto Sans"), size: 9.5pt)
    set par(leading: sb-lead)
    for section in sidebar-sections {
      render-sidebar-section(section)
    }
  }
)

// ── ATS / no-sidebar path ─────────────────────────────────────────────────────
// Used when: (a) ATS mode requested, or (b) no sidebar sections.
// The dense-sidebar overflow path is handled below inside a `context` block.

#if data.opts.ats or (not has-sidebar) {

  set page(
    width:  page_w,
    height: page_h,
    margin: (top: margin_v, bottom: margin_v, left: ats_left, right: margin_r),
  )

  set text(
    font: ("Source Serif 4", "Carlito", "Inter", "Noto Sans"),
    size: 10.5pt,
    fill: c-body,
    lang: data.opts.lang,
  )
  set par(leading: lead, spacing: sp-para)

  // Header
  block(below: sp-name-below, {
    text(size: 22pt, weight: "bold", fill: c-name,
      font: ("Source Serif 4", "Carlito", "Inter"),
      data.header.name,
    )
  })
  if "title" in data.header and data.header.title != none {
    block(below: sp-header-title-below, text(size: 11pt, style: "italic", fill: c-contact, data.header.title))
  }
  block(below: sp-header-contact, text(size: 9.5pt, fill: c-contact, render-runs(data.header.contact)))
  line(length: 100%, stroke: 0.5pt + c-rule)
  v(3mm)

  // Main sections first, then sidebar sections — fully linear for ATS parsers.
  for section in main-sections {
    render-main-section(section)
  }
  for section in sidebar-sections {
    render-main-section(section)
  }

} else {

  // ── Two-column or dense-sidebar-fallback path ──────────────────────────────
  // We need context to measure the sidebar height before deciding the layout.
  // The `context` block gives us access to layout information; we compute the
  // sidebar height and branch into two-column or single-column accordingly.
  //
  // Both branches share the same page settings and text settings, but the
  // two-column branch adds page(background: ...) with the sidebar band.

  context {
    let avail_h      = page_h - 2 * margin_v
    let sidebar_size = measure(sidebar-content-inner)
    let sidebar-fits = sidebar_size.height <= avail_h

    if sidebar-fits {
      // ── Two-column layout ──────────────────────────────────────────────────
      // page(background: ...) draws the sidebar band AND sidebar content on
      // every page so it repeats automatically as main content overflows.

      set page(
        width:  page_w,
        height: page_h,
        margin: (top: margin_v, bottom: margin_v, left: 0pt, right: margin_r),
        background: {
          // (a) Full-height tinted band — drawn on EVERY page for visual continuity.
          place(left + top,
            rect(width: sidebar_w, height: 100%, fill: c-sidebar-bg)
          )
          // (b) Sidebar content — rendered ONCE, on page 1 only. Drawing it in the
          //     page background would otherwise repeat the whole sidebar on every
          //     page of a multi-page résumé; gating on the page counter keeps it on
          //     page 1 while the band continues, so the main column still clears it.
          context {
            if counter(page).get().first() == 1 {
              place(left + top, dx: gutter, dy: margin_v, sidebar-content-inner)
            }
          }
        },
      )

      set text(
        font: ("Source Serif 4", "Carlito", "Inter", "Noto Sans"),
        size: 10.5pt,
        fill: c-body,
        lang: data.opts.lang,
      )
      set par(leading: lead, spacing: sp-para)

      // ── Header ───────────────────────────────────────────────────────────────
      // Left-pad by main_left (= sidebar_w + gutter) so the name, contact, and
      // header rules left-align with the main-column body below — the header sits
      // to the right of the sidebar band with the same gutter as the body, instead
      // of hugging the band edge.
      pad(left: main_left, {
        block(below: sp-name-below, {
          line(length: 100%, stroke: 1.5pt + resolved-accent)
          v(2mm)
          text(size: 22pt, weight: "bold", fill: c-name,
            font: ("Source Serif 4", "Carlito", "Inter"),
            data.header.name,
          )
        })
        if "title" in data.header and data.header.title != none {
          block(below: sp-header-title-below, text(size: 11pt, style: "italic", fill: c-contact, data.header.title))
        }
        block(below: sp-header-contact, text(size: 9.5pt, fill: c-contact, render-runs(data.header.contact)))
        line(length: 100%, stroke: 0.5pt + c-rule)
        v(3mm)
      })

      // Main column content — inset by main_left so it never overlaps the band.
      pad(left: main_left, {
        for section in main-sections {
          render-main-section(section)
        }
      })

    } else {
      // ── Dense-sidebar fallback: single-column ─────────────────────────────
      // The sidebar content is taller than one page; using place() would clip
      // it silently.  Render all sections in a single linear column instead,
      // preserving all data.  The accent colour is kept for visual identity.

      set page(
        width:  page_w,
        height: page_h,
        margin: (top: margin_v, bottom: margin_v, left: ats_left, right: margin_r),
      )

      set text(
        font: ("Source Serif 4", "Carlito", "Inter", "Noto Sans"),
        size: 10.5pt,
        fill: c-body,
        lang: data.opts.lang,
      )
      set par(leading: lead, spacing: sp-para)

      block(below: sp-name-below, {
        text(size: 22pt, weight: "bold", fill: c-name,
          font: ("Source Serif 4", "Carlito", "Inter"),
          data.header.name,
        )
      })
      if "title" in data.header and data.header.title != none {
        block(below: sp-header-title-below, text(size: 11pt, style: "italic", fill: c-contact, data.header.title))
      }
      block(below: sp-header-contact, text(size: 9.5pt, fill: c-contact, render-runs(data.header.contact)))
      line(length: 100%, stroke: 0.5pt + c-rule)
      v(3mm)

      for section in main-sections {
        render-main-section(section)
      }
      for section in sidebar-sections {
        render-main-section(section)
      }
    }
  }
}
