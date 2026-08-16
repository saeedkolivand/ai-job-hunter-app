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

#let keyline-pt     = 1.5pt
#let body-margin-h  = 20mm
#let body-margin-top = 7mm

// Does the header carry a role/title line? Real résumés almost always do
// (`model/adapter.rs` fills `header.title`), which costs the band one text
// line — so the band height has to account for it.
#let has-title = (
  "title" in data.header and data.header.title != none and data.header.title != ""
)

// A THIN band (vs. Meridian's 38mm) — name, optional title, contact line.
//
// Header content is placed in `page.background`, which lays out at UNBOUNDED
// width: without the `band-box-w` bound below, a long contact line does not
// wrap, it runs off the right edge of the sheet (measured: a 125-char contact
// reached x=630pt on a 595pt-wide page — the tail was simply gone). Bounding it
// makes it wrap instead, so the band has to be tall enough to hold whatever it
// wraps to, or the remainder renders white-on-white below the band.
//
// The band height is therefore MEASURED from the header content itself (see
// `#context` at the bottom of this file), never budgeted by line count: nothing
// caps how much a header can carry — `ContactProfile.extra_links` is an
// unbounded `Vec` and a text-derived contact line is arbitrary text — so any
// fixed budget is reachable. A twelve-extra-link profile wrapped to three lines
// and put a whole white baseline 6.7pt below a fixed 28mm band
// (`awesome_band_grows_to_contain_any_contact_line_count`).
//
// `band-min-h` keeps the THIN design brief for the common 1–2-line case: the
// band never shrinks below it, it only grows when the content demands it. A
// title costs one text line, hence the two values. The has-title value was
// bumped 28mm → 29mm when the name→contact gap below was routed through
// `sp-name-below` (#28, 3pt → 9pt): the taller gap alone pushed a common
// title+long-contact header's measured content past the old 28mm floor
// (measured 81.79pt against a 79.37pt floor), which would have made every
// such header grow the band instead of sitting at the intended thin minimum.
#let band-min-h = if has-title { 29mm } else { 24mm }

// Inset above the header content, and the room kept below its last baseline.
// Typst's default text `bottom-edge` IS the baseline, so `measure(band-box)`
// stops there and the last line's descenders hang outside it — `band-pad-bottom`
// is what covers them (7.1pt for a 9.5pt contact line, whose descender is ~2.2pt)
// plus a little breathing room. It is also small enough that the common 1–2-line
// header still measures under `band-min-h`, so the band stays exactly 24/28mm
// there and only grows for genuine overflow — pinned by
// `awesome_band_contains_its_white_header_text`'s band-height assertion, which
// fails if this constant is raised enough to inflate the thin case.
#let band-pad-top = 6mm
#let band-pad-bottom = 2.5mm

// Printable width for the placed band content — the same left/right margins the
// body flow uses.
#let band-box-w = page-w - 2 * body-margin-h

// ── Rich-text helpers ──────────────────────────────────────────────────────────

// The single bold/italic/link ladder. `link-fill` is the ONLY axis that varies:
// body text draws links in the accent, the header band draws them in the band's
// white (the band supplies its own contrast). Parametrised rather than forked so
// the two can never drift — a run flag added to one is added to both.
#let render-runs-in(runs, link-fill) = {
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
      link(r.link, text(fill: link-fill, t))
    } else {
      t
    }
  }
}

#let render-runs(runs) = render-runs-in(runs, c-accent)
#let render-runs-white(runs) = render-runs-in(runs, c-band-text)

// ── Band content ──────────────────────────────────────────────────────────────
//
// Defined once and used twice: measured (to size the band) and rendered (into
// `page.background`). Both go through `band-box`, so the height the band is
// given is the height of the very thing it has to hold — a budget derived from
// the content, not restated next to it.

#let band-header-body = {
  text(
    size: name-pt,
    weight: "bold",
    fill: c-band-text,
    font: (font-name, "Inter", "Carlito"),
    if "name" in data.header { data.header.name } else { "" },
  )
  if has-title {
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
    block(above: sp-name-below,
      text(
        size: body-pt - 1pt,
        fill: c-band-text,
        font: (font-body, "Inter", "Carlito"),
        render-runs-white(data.header.contact),
      )
    )
  }
}

#let band-box = box(width: band-box-w, pad(top: band-pad-top, band-header-body))

// ── Text defaults (set before the measuring context so `measure` sees them) ────

#set text(
  font: (font-body, "Inter", "Carlito", "Noto Sans"),
  size: body-pt,
  fill: c-body,
  lang: data.opts.lang,
)

#set par(leading: lead, spacing: sp-para)

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

// ── Page setup + body ─────────────────────────────────────────────────────────
//
// The page set rule lives inside a `#context` because the band height (and the
// top margin derived from it) is MEASURED from `band-box`, and `measure` is only
// available in a context. A set rule applies to the content of the block it sits
// in, so the whole document body is produced here — that is what makes the
// measured margin reach page 1.

#context {
  let band-h = if is-ats {
    0pt
  } else {
    calc.max(band-min-h, measure(band-box).height + band-pad-bottom)
  }

  set page(
    width:  page-w,
    height: page-h,
    margin: (
      top:    if is-ats { 20mm } else { band-h + keyline-pt + body-margin-top },
      bottom: 18mm,
      left:   body-margin-h,
      right:  body-margin-h,
    ),
    background: if is-ats { none } else {
      place(top + left, rect(width: 100%, height: band-h, fill: c-accent))
      place(top + left, dy: band-h, line(length: 100%, stroke: keyline-pt + c-accent))
      place(top + left, dx: body-margin-h, dy: 0pt, band-box)
    },
  )

  // ── ATS-mode plain header (normal document flow; only when is-ats) ──────────

  if is-ats {
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

  for section in data.sections {
    render-section(section)
  }
}
