// Parametric cover-letter template for the Typst rendering engine.
//
// Themed by the chosen resume template's accent + heading/body fonts
// (data.style) so the letter visually matches the resume family.
// House spacing constants come from _scale.typ (prepended by engine.rs).
//
// Data contract — all keys guarded with `"k" in d` before access:
//   data.opts.page_width_mm / page_height_mm  — page geometry
//   data.opts.lang                             — BCP-47 language tag
//   data.opts.date_position                   — "top-right"|"below-header"|"above-salutation"
//   data.opts.sender_position                 — "top"|"bottom"
//   data.opts.recipient_position              — "before-date"|"after-date"
//   data.opts.subject_line_used               — bool
//   data.opts.subject_line_label              — e.g. "Betreff"
//   data.style.c_accent / c_body / c_name / c_date / c_rule — #RRGGBB colours
//   data.style.font_name / font_body           — Typst font family names
//   data.style.name_pt / body_pt               — font sizes in pt
//   data.letterhead.name                       — candidate full name
//   data.letterhead.contact[]                  — rich-text runs (links first-class)
//   data.date                                  — optional date string
//   data.recipient_lines[]                     — optional recipient block lines
//   data.subject                               — optional subject line text
//   data.salutation                            — optional salutation string
//   data.body[][]                              — paragraphs of rich-text runs
//   data.signoff                               — optional sign-off string
//   data.signature_name                        — name under the sign-off
//   data.signature_title                       — optional title under the name

// ── Style resolution ──────────────────────────────────────────────────────────

#let st = if "style" in data { data.style } else { (:) }

#let c-accent  = rgb(if "c_accent"  in st { st.c_accent  } else { "#2563EB" })
#let c-body    = rgb(if "c_body"    in st { st.c_body    } else { "#222222" })
#let c-name    = rgb(if "c_name"    in st { st.c_name    } else { "#111111" })
#let c-date    = rgb(if "c_date"    in st { st.c_date    } else { "#555555" })
#let c-rule    = rgb(if "c_rule"    in st { st.c_rule    } else { "#aaaaaa" })

#let font-name = if "font_name" in st { st.font_name } else { "Carlito" }
#let font-body = if "font_body" in st { st.font_body } else { "Carlito" }

#let name-pt = if "name_pt" in st { st.name_pt * 1pt } else { 20pt }
#let body-pt = if "body_pt" in st { st.body_pt * 1pt } else { 10.5pt }

// ── Opts resolution ───────────────────────────────────────────────────────────

#let pg-w  = if "page_width_mm"  in data.opts { data.opts.page_width_mm  * 1mm } else { 210mm }
#let pg-h  = if "page_height_mm" in data.opts { data.opts.page_height_mm * 1mm } else { 297mm }
#let lang  = if "lang"           in data.opts { data.opts.lang            } else { "en" }
#let date-pos      = if "date_position"    in data.opts { data.opts.date_position    } else { "below-header" }
#let recip-pos     = if "recipient_position" in data.opts { data.opts.recipient_position } else { "after-date" }
#let subj-used     = if "subject_line_used"  in data.opts { data.opts.subject_line_used  } else { false }
#let subj-label    = if "subject_line_label" in data.opts { data.opts.subject_line_label } else { "Subject" }

// ── Page & typography ─────────────────────────────────────────────────────────

#set page(
  width:  pg-w,
  height: pg-h,
  margin: (x: 25.4mm, y: 25.4mm),
)

#set text(
  font: (font-body, "Carlito", "Inter", "Noto Sans"),
  size: body-pt,
  fill: c-body,
  lang: lang,
)

#set par(leading: lead, spacing: sp-para, justify: true)

// ── Rich-text renderer ────────────────────────────────────────────────────────
// Mirrors the single_column.typ render-runs helper exactly so font rendering
// is identical between resumes and letters.

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

// ── Helper: emit the date line ────────────────────────────────────────────────
// Called from two positions (below-header and above-salutation); top-right is
// handled inline in the letterhead grid.

#let emit-date-block(date-str) = {
  block(above: 14pt, below: 14pt,
    text(size: body-pt, fill: c-date, date-str)
  )
}

// ── Helper: emit the recipient block ─────────────────────────────────────────

#let emit-recipient-block() = {
  if "recipient_lines" in data and data.recipient_lines.len() > 0 {
    block(above: 12pt, below: 12pt, {
      for line in data.recipient_lines {
        text(fill: c-body, line)
        linebreak()
      }
    })
  }
}

// ── Letterhead ────────────────────────────────────────────────────────────────
// When date_position == "top-right" we place name+date on the same row
// (name flush-left, date flush-right) to match DIN 5008 / DACH convention.

#if date-pos == "top-right" {
  // Name (left) + date (right) on one row, then contact below.
  grid(
    columns: (1fr, auto),
    gutter: 8pt,
    align(left,
      text(
        size: name-pt,
        weight: "bold",
        fill: c-name,
        font: (font-name, "Carlito", "Inter"),
        data.letterhead.name,
      )
    ),
    align(right + horizon,
      if "date" in data and data.date != none {
        text(size: body-pt, fill: c-date, data.date)
      } else { "" }
    ),
  )
} else {
  // Name only.
  block(below: sp-name-below,
    text(
      size: name-pt,
      weight: "bold",
      fill: c-name,
      font: (font-name, "Carlito", "Inter"),
      data.letterhead.name,
    )
  )
}

// Contact line (always below the name).
#if "contact" in data.letterhead and data.letterhead.contact.len() > 0 {
  block(below: 4pt,
    text(size: body-pt - 0.5pt, fill: c-body, render-runs(data.letterhead.contact))
  )
}

// Thin accent rule under the letterhead.
#line(length: 100%, stroke: 0.5pt + c-rule)

// ── Date (below-header position) ─────────────────────────────────────────────

#if date-pos == "below-header" and "date" in data and data.date != none {
  emit-date-block(data.date)
}

// ── Recipient block (before-date goes here, after top-right / below-header) ──
// "before-date" means the recipient precedes the date in non-top-right layouts.
// "after-date" means the recipient follows the date.

#if recip-pos == "before-date" and date-pos != "top-right" {
  emit-recipient-block()
  if date-pos == "below-header" {
    // Date already emitted above; skip.
  }
} else if date-pos != "top-right" and date-pos != "below-header" {
  // "above-salutation" or unrecognised — date will be emitted just before
  // the salutation (see below).
}

// After-date recipient block (most markets).
#if recip-pos == "after-date" or recip-pos == "" {
  emit-recipient-block()
}

// ── Subject line ──────────────────────────────────────────────────────────────
// Rendered bold between the recipient block and the salutation, as formal
// markets (DE/AT/CH/FR) expect.

#if subj-used and "subject" in data and data.subject != none {
  block(above: 8pt, below: 8pt,
    text(weight: "bold", fill: c-body, data.subject)
  )
}

// ── Date above-salutation position ───────────────────────────────────────────

#if date-pos == "above-salutation" and "date" in data and data.date != none {
  emit-date-block(data.date)
}

// ── Salutation ────────────────────────────────────────────────────────────────

#if "salutation" in data and data.salutation != none {
  block(above: 12pt, below: 8pt,
    text(fill: c-body, data.salutation)
  )
}

// ── Body paragraphs ───────────────────────────────────────────────────────────

#if "body" in data {
  for para in data.body {
    block(above: 0pt, below: sp-para, breakable: true,
      render-runs(para)
    )
  }
}

// ── Sign-off ──────────────────────────────────────────────────────────────────

#if "signoff" in data and data.signoff != none {
  block(above: 20pt, below: 4pt,
    text(fill: c-body, data.signoff)
  )
}

// ── Signature block (name + optional title) ───────────────────────────────────
// 3 blank lines of gap mimic the printpdf renderer's "room for a real signature"
// spacing (line_height + 14mm ≈ 20pt gap used below).

#v(20pt)

#text(
  weight: "bold",
  fill: c-name,
  font: (font-name, "Carlito", "Inter"),
  data.signature_name,
)

#if "signature_title" in data and data.signature_title != none {
  block(above: 2pt,
    text(size: body-pt - 0.5pt, fill: c-body, data.signature_title)
  )
}
